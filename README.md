# usbd-audio

[![Crates.io](https://img.shields.io/crates/v/usbd-audio.svg)](https://crates.io/crates/usbd-audio)
[![docs.rs](https://img.shields.io/docsrs/usbd-audio.svg)](https://docs.rs/usbd-audio)

USB Audio 1.0 class for [usb-device](https://crates.io/crates/usb-device)

This crate provides a USB audio device class based on "Universal Serial Bus
Device Class Definition for Audio Devices", Release 1.0 (experimental
implementation without the aim of standard compliance).

Since the USB descriptor can be quite large, it may be required to activate the
feature `control-buffer-256` of the `usb-device` crate.

Example

```rust
let mut usb_bus = ... // create a UsbBusAllocator in a platform specific way
let mut usb_audio = AudioClassBuilder::new()
    .input(
        StreamConfig::new_discrete(
            Format::S16le,
            1,
            &[48000],
            TerminalType::InMicrophone).unwrap())
    .output(
        StreamConfig::new_discrete(
            Format::S24le,
            2,
            &[44100, 48000, 96000],
            TerminalType::OutSpeaker).unwrap())
    .build(&usb_bus)
    .unwrap();
```

This example creates an audio device having a one channel (Mono) microphone with
a fixed sampling frequency of 48 KHz and a two channel (Stereo) speaker output
that supports three different sampling rates.
