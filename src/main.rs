mod audio;
mod sniffer;

use anyhow::Context as _;
use clap::Parser as _;
use pipewire::spa;
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

fn parse_format(format: &str) -> Result<spa::param::audio::AudioFormat, std::io::Error> {
    Ok(match format {
        "S8" => spa::param::audio::AudioFormat::S8,
        "U8" => spa::param::audio::AudioFormat::U8,
        "S16LE" => spa::param::audio::AudioFormat::S16LE,
        "S16BE" => spa::param::audio::AudioFormat::S16BE,
        "U16LE" => spa::param::audio::AudioFormat::U16LE,
        "U16BE" => spa::param::audio::AudioFormat::U16BE,
        "S24_32LE" => spa::param::audio::AudioFormat::S24_32LE,
        "S24_32BE" => spa::param::audio::AudioFormat::S24_32BE,
        "U24_32LE" => spa::param::audio::AudioFormat::U24_32LE,
        "U24_32BE" => spa::param::audio::AudioFormat::U24_32BE,
        "S32LE" => spa::param::audio::AudioFormat::S32LE,
        "S32BE" => spa::param::audio::AudioFormat::S32BE,
        "U32LE" => spa::param::audio::AudioFormat::U32LE,
        "U32BE" => spa::param::audio::AudioFormat::U32BE,
        "S24LE" => spa::param::audio::AudioFormat::S24LE,
        "S24BE" => spa::param::audio::AudioFormat::S24BE,
        "U24LE" => spa::param::audio::AudioFormat::U24LE,
        "U24BE" => spa::param::audio::AudioFormat::U24BE,
        "S20LE" => spa::param::audio::AudioFormat::S20LE,
        "S20BE" => spa::param::audio::AudioFormat::S20BE,
        "U20LE" => spa::param::audio::AudioFormat::U20LE,
        "U20BE" => spa::param::audio::AudioFormat::U20BE,
        "S18LE" => spa::param::audio::AudioFormat::S18LE,
        "S18BE" => spa::param::audio::AudioFormat::S18BE,
        "U18LE" => spa::param::audio::AudioFormat::U18LE,
        "U18BE" => spa::param::audio::AudioFormat::U18BE,
        "F32LE" => spa::param::audio::AudioFormat::F32LE,
        "F32BE" => spa::param::audio::AudioFormat::F32BE,
        "F64LE" => spa::param::audio::AudioFormat::F64LE,
        "F64BE" => spa::param::audio::AudioFormat::F64BE,
        _ => {
            return Err(std::io::Error::other("invalid audio format"));
        }
    })
}

fn parse_channel(channel: &str) -> Result<spa::sys::spa_audio_channel, std::io::Error> {
    Ok(match channel {
        "FL" => spa::sys::SPA_AUDIO_CHANNEL_FL,
        "FR" => spa::sys::SPA_AUDIO_CHANNEL_FR,
        "FC" => spa::sys::SPA_AUDIO_CHANNEL_FC,
        "LFE" => spa::sys::SPA_AUDIO_CHANNEL_LFE,
        "SL" => spa::sys::SPA_AUDIO_CHANNEL_SL,
        "SR" => spa::sys::SPA_AUDIO_CHANNEL_SR,
        "FLC" => spa::sys::SPA_AUDIO_CHANNEL_FLC,
        "FRC" => spa::sys::SPA_AUDIO_CHANNEL_FRC,
        "RC" => spa::sys::SPA_AUDIO_CHANNEL_RC,
        "RL" => spa::sys::SPA_AUDIO_CHANNEL_RL,
        "RR" => spa::sys::SPA_AUDIO_CHANNEL_RR,
        "TC" => spa::sys::SPA_AUDIO_CHANNEL_TC,
        "TFL" => spa::sys::SPA_AUDIO_CHANNEL_TFL,
        "TFC" => spa::sys::SPA_AUDIO_CHANNEL_TFC,
        "TFR" => spa::sys::SPA_AUDIO_CHANNEL_TFR,
        "TRL" => spa::sys::SPA_AUDIO_CHANNEL_TRL,
        "TRC" => spa::sys::SPA_AUDIO_CHANNEL_TRC,
        "TRR" => spa::sys::SPA_AUDIO_CHANNEL_TRR,
        "RLC" => spa::sys::SPA_AUDIO_CHANNEL_RLC,
        "RRC" => spa::sys::SPA_AUDIO_CHANNEL_RRC,
        "FLW" => spa::sys::SPA_AUDIO_CHANNEL_FLW,
        "FRW" => spa::sys::SPA_AUDIO_CHANNEL_FRW,
        "LFE2" => spa::sys::SPA_AUDIO_CHANNEL_LFE2,
        "FLH" => spa::sys::SPA_AUDIO_CHANNEL_FLH,
        "FCH" => spa::sys::SPA_AUDIO_CHANNEL_FCH,
        "FRH" => spa::sys::SPA_AUDIO_CHANNEL_FRH,
        "TFLC" => spa::sys::SPA_AUDIO_CHANNEL_TFLC,
        "TFRC" => spa::sys::SPA_AUDIO_CHANNEL_TFRC,
        "TSL" => spa::sys::SPA_AUDIO_CHANNEL_TSL,
        "TSR" => spa::sys::SPA_AUDIO_CHANNEL_TSR,
        "LLFE" => spa::sys::SPA_AUDIO_CHANNEL_LLFE,
        "RLFE" => spa::sys::SPA_AUDIO_CHANNEL_RLFE,
        "BC" => spa::sys::SPA_AUDIO_CHANNEL_BC,
        "BLC" => spa::sys::SPA_AUDIO_CHANNEL_BLC,
        "BRC" => spa::sys::SPA_AUDIO_CHANNEL_BRC,
        _ => {
            return Err(std::io::Error::other("invalid audio channel"));
        }
    })
}

#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    rate: u32,
    #[arg(short, long, value_parser = parse_format)]
    format: spa::param::audio::AudioFormat,
    #[arg(short, long, value_delimiter = ',', value_parser = parse_channel)]
    channels: Vec<spa::sys::spa_audio_channel>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_millis().init();
    let cli = Cli::parse();
    log::debug!("{cli:#?}");

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
            &cli,
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
