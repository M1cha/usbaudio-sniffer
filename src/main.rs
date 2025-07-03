mod audio;
mod sniffer;

use anyhow::Context as _;
use sniffer::Sniffer;
use tokio::io::AsyncReadExt as _;

struct AudioFrame {
    data: [u8; sniffer::MAX_DATA_SIZE],
    start: usize,
    end: usize,
}

impl AudioFrame {
    pub const fn new() -> Self {
        Self {
            data: [0u8; sniffer::MAX_DATA_SIZE],
            start: 0,
            end: 0,
        }
    }

    pub fn slice(&self) -> &[u8] {
        &self.data[self.start..self.end]
    }

    pub fn remove_start(&mut self, num: usize) {
        if num > self.end - self.start {
            panic!("tried to remove more data than available");
        }

        self.start += num;
    }

    pub fn remove_end(&mut self, num: usize) {
        if num > self.end - self.start {
            panic!("tried to remove more data than available");
        }

        self.end -= num;
    }
}

struct AudioReceiver {
    out_frame_received: bool,
}

impl AudioReceiver {
    /// returns true, if there is audio data in `frame`.
    fn usb_frame_received(&mut self, frame: &mut AudioFrame) -> bool {
        let data = frame.slice();

        if data.len() < 3 {
            return false;
        }

        if data[0] == 0xe1 {
            self.out_frame_received = true;
            return false;
        }

        if self.out_frame_received && data[0] == 0xc3 {
            frame.remove_start(1);
            frame.remove_end(2);

            if frame.slice().is_empty() {
                log::warn!("empty audio data");
            }

            return true;
        }

        self.out_frame_received = false;
        false
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_millis().init();
    let (unused_buffers_sender, unused_buffers_receiver) = crossbeam::channel::unbounded();
    let (ready_buffers_sender, ready_buffers_receiver) = crossbeam::channel::unbounded();

    for _ in 0..16 {
        unused_buffers_sender
            .send(Box::new(AudioFrame::new()))
            .unwrap();
    }

    let unused_buffers_sender2 = unused_buffers_sender.clone();
    let ready_buffers_receiver2 = ready_buffers_receiver.clone();
    std::thread::spawn(move || {
        audio::run(
            unused_buffers_sender2.clone(),
            ready_buffers_receiver2.clone(),
        )
        .unwrap();
    });

    let mut sniffer = Sniffer::new().await.context("failed to create sniffer")?;
    sniffer.start().await?;

    let mut reader = sniffer.reader();
    let mut audio_receiver = AudioReceiver {
        out_frame_received: false,
    };

    let mut toggle = false;
    let mut scratch = [0u8; sniffer::MAX_DATA_SIZE];
    loop {
        let mut common_data = [0u8; 3];
        reader.read_exact(&mut common_data).await?;
        let common = sniffer::CommonHeader(&common_data);
        //log::debug!("common: {common:#?}");

        if common.non_zero() {
            anyhow::bail!("zero flag in header is not zero");
        }
        if common.toggle() != toggle {
            anyhow::bail!(
                "toggle flag in header is {}, expected {}",
                common.toggle(),
                toggle
            );
        }
        toggle = !toggle;

        if common.is_data() {
            let mut header_data = [0u8; 4];
            reader.read_exact(&mut header_data).await?;
            let header = sniffer::DataHeader(&header_data);
            //log::debug!("data: {header:#?}");

            let frame_size: usize = header.size().into();
            if !(common_data.len() + header_data.len()..=sniffer::MAX_DATA_SIZE)
                .contains(&frame_size)
            {
                anyhow::bail!("bad frame size: {}", frame_size);
            }

            let data_size: usize = frame_size - common_data.len() - header_data.len();
            if data_size > 0 {
                match unused_buffers_receiver.try_recv() {
                    Ok(mut frame) => {
                        let usb_data = &mut frame.data[0..data_size];
                        reader.read_exact(usb_data).await?;
                        frame.start = 0;
                        frame.end = data_size;

                        if audio_receiver.usb_frame_received(&mut frame) {
                            match ready_buffers_sender.try_send(frame) {
                                Ok(_) => (),
                                Err(crossbeam::channel::TrySendError::Full(frame)) => {
                                    log::warn!("failed to send read buffer");
                                    unused_buffers_sender.send(frame).unwrap();
                                }
                                Err(crossbeam::channel::TrySendError::Disconnected(_)) => {
                                    unimplemented!();
                                }
                            }
                        } else {
                            unused_buffers_sender.send(frame).unwrap();
                        }
                    }
                    Err(_) => {
                        log::warn!("failed to allocate, drop");
                        let usb_data = &mut scratch[0..data_size];
                        reader.read_exact(usb_data).await?;
                    }
                }
            }
        } else {
            let mut header_data = [0u8; 1];
            reader.read_exact(&mut header_data).await?;
            let _header = sniffer::StatusHeader(&header_data);
            //log::debug!("status: {header:#?}");
        }
    }
}
