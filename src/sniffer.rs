use anyhow::Context as _;

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
enum CaptureControl {
    Reset = 0,
    Enable = 1,
    Speed0 = 2,
    Speed1 = 3,
    Test = 4,
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum CaptureSpeed {
    LowSpeed = 0,
    FullSpeed = 1,
    HighSpeed = 2,
    Reset = 3,
}

bitfield::bitfield! {
    pub struct CommonHeader(MSB0 [u8]);
    impl Debug;

    pub is_data, _: 0;
    pub toggle, _: 1;
    pub non_zero, _: 2;
    pub timestamp_overflow, _: 3;
    pub u32, ts, _: 23, 4;
}

bitfield::bitfield! {
    pub struct StatusHeader(MSB0 [u8]);
    impl Debug;

    pub u8, speed, _: 1, 0;
    pub trigger, _: 2;
    pub vbus, _: 3;
    pub u8, ls, _: 7, 4;
}

bitfield::bitfield! {
    pub struct DataHeader(MSB0 [u8]);
    impl Debug;

    pub u8, reserved0, _: 1, 0;
    pub data_error, _: 2;
    pub crc_error, _: 3;
    pub overflow, _: 4;
    pub u16, size, _: 15, 5;
    pub u16, duration, _: 31, 16;
}

const DATA_ENDPOINT_SIZE: usize = 512;
//const TRANSFER_SIZE: usize = DATA_ENDPOINT_SIZE * 2000;
const TRANSFER_SIZE: usize = DATA_ENDPOINT_SIZE;
#[cfg(target_os = "linux")]
const TRANSFER_COUNT: usize = 16;
#[cfg(target_os = "windows")]
const TRANSFER_COUNT: usize = 128;

pub const MAX_DATA_SIZE: usize = 1280;

pub struct Sniffer {
    interface: nusb::Interface,
    ep_in: nusb::Endpoint<nusb::transfer::Bulk, nusb::transfer::In>,
}

impl Sniffer {
    pub async fn new() -> anyhow::Result<Self> {
        let di = nusb::list_devices()
            .await
            .unwrap()
            .find(|d| d.vendor_id() == 0x6666 && d.product_id() == 0x6620)
            .context("device should be connected")?;
        log::debug!("Device info: {di:?}");

        let device = di.open().await.context("failed to open device")?;
        let interface = device
            .claim_interface(0)
            .await
            .context("failed to claim interface")?;
        let ep_in = interface
            .endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(0x82)
            .context("failed to get endpoint")?;

        let mut sniffer = Self { interface, ep_in };
        sniffer
            .init()
            .await
            .context("failed to initialize sniffer")?;

        Ok(sniffer)
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        self.ctrl(CaptureControl::Enable, false).await?;
        self.ctrl(CaptureControl::Reset, true).await?;
        self.flush_data().context("failed to flush data")?;

        let speed_u8 = CaptureSpeed::FullSpeed as u8;
        self.ctrl(CaptureControl::Speed0, (speed_u8 & 1) != 0)
            .await?;
        self.ctrl(CaptureControl::Speed1, (speed_u8 & 2) != 0)
            .await?;

        self.ctrl(CaptureControl::Reset, false).await?;
        self.ctrl(CaptureControl::Enable, true).await?;

        Ok(())
    }

    async fn ctrl(&mut self, index: CaptureControl, value: bool) -> anyhow::Result<()> {
        self.interface
            .control_out(
                nusb::transfer::ControlOut {
                    control_type: nusb::transfer::ControlType::Vendor,
                    recipient: nusb::transfer::Recipient::Device,
                    request: 0xd0,
                    value: index as u16 | (if value { 1 } else { 0 } << 4),
                    index: 0,
                    data: &[],
                },
                core::time::Duration::from_millis(1),
            )
            .await
            .with_context(|| format!("failed to send {index:?} request"))
    }

    async fn init(&mut self) -> anyhow::Result<()> {
        self.ctrl(CaptureControl::Reset, true).await?;
        self.ctrl(CaptureControl::Enable, false).await?;
        self.ctrl(CaptureControl::Test, false).await?;
        self.ctrl(CaptureControl::Speed0, true).await?;
        self.ctrl(CaptureControl::Speed0, false).await?;
        self.ctrl(CaptureControl::Speed1, true).await?;
        self.ctrl(CaptureControl::Speed1, false).await?;
        Ok(())
    }

    fn flush_data(&mut self) -> anyhow::Result<()> {
        let buffer = self.ep_in.allocate(DATA_ENDPOINT_SIZE);
        self.ep_in.submit(buffer);

        for _ in 0..100 {
            let completion = match self
                .ep_in
                .wait_next_complete(core::time::Duration::from_millis(20))
            {
                None => break,
                Some(v) => v,
            };

            completion.status.context("bulk transfer failed")?;

            self.ep_in.submit(completion.buffer);
        }

        Ok(())
    }

    pub fn reader(self) -> nusb::io::EndpointRead<impl nusb::transfer::BulkOrInterrupt> {
        self.ep_in
            .reader(TRANSFER_SIZE)
            .with_num_transfers(TRANSFER_COUNT)
    }
}
