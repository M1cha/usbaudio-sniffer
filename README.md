# USB Audio Sniffer

This software captures audio data from a USB device like USB headphones and
makes it available through a pipewire audio source. This allows e.g. streaming
the audio of your USB headphones in OBS to work around game consoles not
allowing to output to HDMI and USB at the same time. It only works on Linux.

It uses the open source [usb-sniffer](https://github.com/ataradov/usb-sniffer)
hardware from Alex Taradov.

## Compile / Run

```bash
cargo run --release -- --rate 48000 --format S16LE --channels FL,FR
```

If you want to see logs, I recommend running it the following way:

```bash
RUST_LOG=debug,nusb=info cargo run --release -- --rate 48000 --format S16LE --channels FL,FR
```

## Audio format

In the CLI above you have to specify rate, format and channel config. That's
because this tool doesn't try to detect it from the sniffed data so it can be
started even after the USB host has finished the configuration already. The
format should be static an can be obtained using `lsusb -v -d VENDOR:PRODUCT`.
There are usually multiple formats, so you either have to guess or look at the
sniffer output using Wireshark to find out which one is being used.

For me, the one in question looks like this:

```
    Interface Descriptor:
      bLength                 9
      bDescriptorType         4
      bInterfaceNumber        0
      bAlternateSetting       0
      bNumEndpoints           0
      bInterfaceClass         1 Audio
      bInterfaceSubClass      1 Control Device
      bInterfaceProtocol      0
      iInterface              0
...
      AudioControl Interface Descriptor:
        bLength                12
        bDescriptorType        36
        bDescriptorSubtype      2 (INPUT_TERMINAL)
        bTerminalID            38
        wTerminalType      0x0101 USB Streaming
        bAssocTerminal          0
        bNrChannels             2
        wChannelConfig     0x0003
          Left Front (L)
          Right Front (R)
        iChannelNames           0
        iTerminal               0
...
    Interface Descriptor:
      bLength                 9
      bDescriptorType         4
      bInterfaceNumber        1
      bAlternateSetting       1
      bNumEndpoints           1
      bInterfaceClass         1 Audio
      bInterfaceSubClass      2 Streaming
      bInterfaceProtocol      0
      iInterface              0
      AudioStreaming Interface Descriptor:
        bLength                 7
        bDescriptorType        36
        bDescriptorSubtype      1 (AS_GENERAL)
        bTerminalLink          38
        bDelay                  1 frames
        wFormatTag         0x0001 PCM
      AudioStreaming Interface Descriptor:
        bLength                11
        bDescriptorType        36
        bDescriptorSubtype      2 (FORMAT_TYPE)
        bFormatType             1 (FORMAT_TYPE_I)
        bNrChannels             2
        bSubframeSize           2
        bBitResolution         16
        bSamFreqType            1 Discrete
        tSamFreq[ 0]        48000
...
```
