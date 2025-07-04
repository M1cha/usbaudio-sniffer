use anyhow::Context as _;
use pipewire::spa;

struct UserData {
    unused_buffers_sender: crossbeam::channel::Sender<Box<crate::AudioFrame>>,
    ready_buffers_receiver: crossbeam::channel::Receiver<Box<crate::AudioFrame>>,
    stride: usize,
}

fn get_channel_size(format: spa::param::audio::AudioFormat) -> anyhow::Result<usize> {
    Ok(match format {
        spa::param::audio::AudioFormat::S8 => 1,
        spa::param::audio::AudioFormat::U8 => 1,
        spa::param::audio::AudioFormat::S16LE => 2,
        spa::param::audio::AudioFormat::S16BE => 2,
        spa::param::audio::AudioFormat::U16LE => 2,
        spa::param::audio::AudioFormat::U16BE => 2,
        spa::param::audio::AudioFormat::S24_32LE => 4,
        spa::param::audio::AudioFormat::S24_32BE => 4,
        spa::param::audio::AudioFormat::U24_32LE => 4,
        spa::param::audio::AudioFormat::U24_32BE => 4,
        spa::param::audio::AudioFormat::S32LE => 4,
        spa::param::audio::AudioFormat::S32BE => 4,
        spa::param::audio::AudioFormat::U32LE => 4,
        spa::param::audio::AudioFormat::U32BE => 4,
        spa::param::audio::AudioFormat::S24LE => 3,
        spa::param::audio::AudioFormat::S24BE => 3,
        spa::param::audio::AudioFormat::U24LE => 3,
        spa::param::audio::AudioFormat::U24BE => 3,
        spa::param::audio::AudioFormat::S20LE => 3,
        spa::param::audio::AudioFormat::S20BE => 3,
        spa::param::audio::AudioFormat::U20LE => 3,
        spa::param::audio::AudioFormat::U20BE => 3,
        spa::param::audio::AudioFormat::S18LE => 3,
        spa::param::audio::AudioFormat::S18BE => 3,
        spa::param::audio::AudioFormat::U18LE => 3,
        spa::param::audio::AudioFormat::U18BE => 3,
        spa::param::audio::AudioFormat::F32LE => 4,
        spa::param::audio::AudioFormat::F32BE => 4,
        spa::param::audio::AudioFormat::F64LE => 8,
        spa::param::audio::AudioFormat::F64BE => 8,
        _ => anyhow::bail!("invalid audio format"),
    })
}

pub fn run(
    cli: &crate::Cli,
    unused_buffers_sender: crossbeam::channel::Sender<Box<crate::AudioFrame>>,
    ready_buffers_receiver: crossbeam::channel::Receiver<Box<crate::AudioFrame>>,
) -> anyhow::Result<()> {
    let channel_size = get_channel_size(cli.format)?;

    let mainloop = pipewire::main_loop::MainLoop::new(None)?;
    let context = pipewire::context::Context::new(&mainloop)?;
    let core = context.connect(None)?;
    let properties = pipewire::properties::properties! {
        *pipewire::keys::NODE_VIRTUAL => "true",
        *pipewire::keys::MEDIA_CLASS => "Audio/Source",
        *pipewire::keys::NODE_NAME => "USB Audio Sniffer",
    };
    let stream = pipewire::stream::Stream::new(&core, "usb-sniffer", properties)?;

    let data = UserData {
        unused_buffers_sender,
        ready_buffers_receiver,
        stride: channel_size * cli.channels.len(),
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .process(|stream, userdata| match stream.dequeue_buffer() {
            None => println!("out of buffers"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                let data = &mut datas[0];
                let n_frames = if let Some(mut slice) = data.data() {
                    let mut total_frames = 0;

                    while slice.len() > crate::sniffer::MAX_DATA_SIZE {
                        if let Ok(frame) = userdata.ready_buffers_receiver.try_recv() {
                            let num_frames_pipewire = slice.len() / userdata.stride;
                            let num_frames_buffer = frame.slice().len() / userdata.stride;
                            let num_frames_common = num_frames_buffer.min(num_frames_pipewire);

                            if num_frames_common < num_frames_buffer {
                                log::warn!("BUG: pipewire buffer is to small, partial drop");
                            }

                            let slice_len = num_frames_common * userdata.stride;
                            slice[0..slice_len].copy_from_slice(&frame.slice()[0..slice_len]);

                            userdata.unused_buffers_sender.send(frame).unwrap();
                            total_frames += num_frames_common;
                            slice = &mut slice[slice_len..];
                        } else {
                            break;
                        }
                    }

                    total_frames
                } else {
                    0
                };
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = userdata.stride as _;
                *chunk.size_mut() = (userdata.stride * n_frames) as _;
            }
        })
        .register()?;

    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(cli.format);
    audio_info.set_rate(cli.rate);
    audio_info.set_channels(cli.channels.len().try_into().unwrap());

    let mut position = [0; spa::param::audio::MAX_CHANNELS];
    for (index, channel) in cli.channels.iter().enumerate() {
        *(position.get_mut(index).context("too many channels")?) = *channel;
    }
    audio_info.set_position(position);

    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::sys::SPA_TYPE_OBJECT_Format,
            id: spa::sys::SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )
    .unwrap()
    .0
    .into_inner();

    let mut params = [spa::pod::Pod::from_bytes(&values).unwrap()];

    stream.connect(
        spa::utils::Direction::Output,
        None,
        pipewire::stream::StreamFlags::AUTOCONNECT
            | pipewire::stream::StreamFlags::MAP_BUFFERS
            | pipewire::stream::StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    mainloop.run();
    Ok(())
}
