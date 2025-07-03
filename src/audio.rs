use pipewire::spa;

pub const DEFAULT_RATE: u32 = 48000;
pub const DEFAULT_CHANNELS: u32 = 2;
pub const CHAN_SIZE: usize = std::mem::size_of::<i16>();

struct UserData {
    unused_buffers_sender: crossbeam::channel::Sender<Box<crate::AudioFrame>>,
    ready_buffers_receiver: crossbeam::channel::Receiver<Box<crate::AudioFrame>>,
}

pub fn run(
    unused_buffers_sender: crossbeam::channel::Sender<Box<crate::AudioFrame>>,
    ready_buffers_receiver: crossbeam::channel::Receiver<Box<crate::AudioFrame>>,
) -> anyhow::Result<()> {
    let mainloop = pipewire::main_loop::MainLoop::new(None)?;
    let context = pipewire::context::Context::new(&mainloop)?;
    let core = context.connect(None)?;
    let properties = pipewire::properties::properties! {
        *pipewire::keys::NODE_VIRTUAL => "true",
        *pipewire::keys::MEDIA_CLASS => "Audio/Source",
        *pipewire::keys::NODE_NAME => "audio source",
    };
    let stream = pipewire::stream::Stream::new(&core, "usb-sniffer", properties)?;

    let data = UserData {
        unused_buffers_sender,
        ready_buffers_receiver,
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .process(|stream, userdata| match stream.dequeue_buffer() {
            None => println!("out of buffers"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                let stride = CHAN_SIZE * DEFAULT_CHANNELS as usize;
                let data = &mut datas[0];
                let n_frames = if let Some(slice) = data.data() {
                    if let Ok(frame) = userdata.ready_buffers_receiver.try_recv() {
                        let num_frames_pipewire = slice.len() / stride;
                        let num_frames_queue = frame.slice().len() / stride;
                        let num_frames_common = num_frames_queue.min(num_frames_pipewire);

                        slice[0..num_frames_common * stride]
                            .copy_from_slice(&frame.slice()[0..num_frames_common * stride]);

                        userdata.unused_buffers_sender.send(frame).unwrap();
                        num_frames_common
                    } else {
                        0
                    }
                } else {
                    0
                };
                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = stride as _;
                *chunk.size_mut() = (stride * n_frames) as _;
            }
        })
        .register()?;

    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::S16LE);
    audio_info.set_rate(DEFAULT_RATE);
    audio_info.set_channels(DEFAULT_CHANNELS);

    let mut position = [0; spa::param::audio::MAX_CHANNELS];
    position[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
    position[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
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
