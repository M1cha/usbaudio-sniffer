mod audio;
mod sniffer;

use sniffer::Sniffer;

use anyhow::Context as _;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt as _;

struct AudioReceiver {
    out_frame_received: bool,
    queue: Arc<Mutex<VecDeque<u8>>>,
}

impl AudioReceiver {
    fn usb_frame_received(&mut self, data: &[u8]) {
        if data.len() < 3 {
            return;
        }

        if data[0] == 0xe1 {
            self.out_frame_received = true;
            return;
        }

        if self.out_frame_received && data[0] == 0xc3 {
            self.audio_frame_received(&data[1..data.len() - 2]);
        }

        self.out_frame_received = false;
    }

    fn audio_frame_received(&self, data: &[u8]) {
        //log::debug!("audio");
        let mut queue = self.queue.lock().unwrap();
        for byte in data {
            queue.push_back(*byte);
        }
        //hexdump::hexdump(&data);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let queue = Arc::new(Mutex::new(VecDeque::new()));
    let queue2 = queue.clone();

    std::thread::spawn(move || {
        audio::run(queue2).unwrap();
    });

    let mut sniffer = Sniffer::new().await.context("failed to create sniffer")?;
    sniffer.start().await?;

    let mut reader = sniffer.reader();
    let mut audio_receiver = AudioReceiver {
        out_frame_received: false,
        queue,
    };

    let mut toggle = false;
    let mut usb_data = vec![0u8; sniffer::MAX_DATA_SIZE];
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
                let usb_data = &mut usb_data[0..data_size];
                reader.read_exact(usb_data).await?;
                audio_receiver.usb_frame_received(usb_data);
            }
        } else {
            let mut header_data = [0u8; 1];
            reader.read_exact(&mut header_data).await?;
            let _header = sniffer::StatusHeader(&header_data);
            //log::debug!("status: {header:#?}");
        }
    }
}
