# USB Audio Sniffer

This software captures audio data from a USB device like USB headphones and
makes it available through a pipewire audio source. This allows e.g. streaming
the audio of your USB headphones in OBS to work around game consoles not
allowing to output to HDMI and USB at the same time. It only works on Linux.

It uses the open source [usb-sniffer](https://github.com/ataradov/usb-sniffer)
hardware from Alex Taradov.

## Compile / Run

```bash
cargo run --release
```

If you want to see logs, I recommend running it the following way:

```bash
RUST_LOG=debug,nusb=info cargo run --release
```

## Audio format

Currently, the audio format is hardcoded in [audio.rs](src/audio.rs). Take a
look at `DEFAULT_RATE`, `DEFAULT_CHANNELS`, `DEFAULT_FORMAT`, `CHAN_SIZE` and
`audio_info.set_position`.
