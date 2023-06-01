//! USB Audio class
//!
//! This crate provides a USB device class based on "Universal Serial Bus Device
//! Class Definition for Audio Devices", Release 1.0 (experimental
//! implementation without the aim of standard compliance).
//!
//! Since the USB descriptor can be quite large, it may be required to activate the feature
//! `control-buffer-256` of the `usb-device` crate.
//!
//! Example
//!
//! ```ignore
//! let mut usb_bus = ... // create a UsbBusAllocator in a platform specific way
//!
//! let mut usb_audio = AudioClassBuilder::new()
//!     .input(
//!         StreamConfig::new_discrete(
//!             Format::S16le,
//!             1,
//!             &[48000],
//!             TerminalType::InMicrophone).unwrap())
//!     .output(
//!         StreamConfig::new_discrete(
//!             Format::S24le,
//!             2,
//!             &[44100, 48000, 96000],
//!             TerminalType::OutSpeaker).unwrap())
//!     .build(&usb_bus)
//!     .unwrap();
//! ```
//!
//! This example creates an audio device having a one channel (Mono) microphone
//! with a fixed sampling frequency of 48 KHz and a two channel (Stereo) speaker
//! output that supports three different sampling rates.
#![no_std]

use class_codes::*;
use core::convert::From;
use usb_device::control::{Recipient, Request, RequestType};
use usb_device::device::DEFAULT_ALTERNATE_SETTING;
use usb_device::endpoint::{Endpoint, EndpointDirection, In, Out};
use usb_device::{class_prelude::*, UsbDirection};

mod terminal_type;
pub use terminal_type::TerminalType;
mod class_codes;

const ID_INPUT_TERMINAL: u8 = 0x01;
const ID_OUTPUT_TERMINAL: u8 = 0x02;

const MAX_ISO_EP_SIZE: u32 = 1023;

#[derive(Clone, Copy, Debug)]
pub enum Format {
    /// Signed, 16 bits per subframe, little endian
    S16le,
    /// Signed, 24 bits per subframe, little endian
    S24le,
}

/// Sampling rates that shall be supported by an steaming endpoint
#[derive(Debug)]
pub enum Rates<'a> {
    /// A continuous range of sampling rates in samples/second defined by a
    /// tuple including a minimum value and a maximum value. The maximum value
    /// must be greater than the minimum value.
    Continuous(u32, u32),
    /// A set of discrete sampling rates in samples/second
    Discrete(&'a [u32]),
}

#[derive(Debug)]
pub struct StreamConfig<'a> {
    format: Format,
    channels: u8,
    rates: Rates<'a>,
    terminal_type: TerminalType,
    /// ISO endpoint size calculated from format, channels and rates (may be
    /// removed in future)
    ep_size: u16,
}

impl StreamConfig<'_> {
    /// Create a stream configuration with one or more discrete sampling rates
    /// indicated in samples/second. An input stream or an output stream will
    /// have an Input Terminal or Output Terminal of Terminal Type
    /// `terminal_type`, respectively.
    pub fn new_discrete<'a>(
        format: Format,
        channels: u8,
        rates: &'a [u32],
        terminal_type: TerminalType,
    ) -> Result<StreamConfig<'a>> {
        let max_rate = rates.iter().max().unwrap();
        let ep_size = Self::ep_size(format, channels, *max_rate)?;
        let rates = Rates::Discrete(rates);
        Ok(StreamConfig {
            format,
            channels,
            rates,
            terminal_type,
            ep_size,
        })
    }

    /// Create a stream configuration with a continuous range of supported
    /// sampling rates indicated in samples/second. An input stream or an output
    /// stream will have an Input Terminal or Output Terminal of Terminal Type
    /// `terminal_type`, respectively.
    pub fn new_continuous(
        format: Format,
        channels: u8,
        min_rate: u32,
        max_rate: u32,
        terminal_type: TerminalType,
    ) -> Result<StreamConfig<'static>> {
        if min_rate >= max_rate {
            return Err(Error::InvalidValue);
        }
        let ep_size = Self::ep_size(format, channels, max_rate)?;
        let rates = Rates::Continuous(min_rate, max_rate);
        Ok(StreamConfig {
            format,
            channels,
            rates,
            terminal_type,
            ep_size,
        })
    }

    /// calculate ISO endpoint size from format, channels and rates
    fn ep_size(format: Format, channels: u8, max_rate: u32) -> Result<u16> {
        let octets_per_frame = channels as u32
            * match format {
                Format::S16le => 2,
                Format::S24le => 3,
            };
        let ep_size = octets_per_frame * max_rate / 1000;
        if ep_size > MAX_ISO_EP_SIZE {
            return Err(Error::BandwidthExceeded);
        }
        Ok(ep_size as u16)
    }
}

/// USB audio errors, including possible USB Stack errors
#[derive(Debug)]
pub enum Error {
    InvalidValue,
    BandwidthExceeded,
    StreamNotInitialized,
    UsbError(usb_device::UsbError),
}

impl From<UsbError> for Error {
    fn from(err: UsbError) -> Self {
        Error::UsbError(err)
    }
}

/// Result type alias for the USB Audio Class
type Result<T> = core::result::Result<T, Error>;

/// Internal state related to audio streaming in a certain direction
struct AudioStream<'a, B: UsbBus, D: EndpointDirection> {
    stream_config: StreamConfig<'a>,
    interface: InterfaceNumber,
    endpoint: Endpoint<'a, B, D>,
    alt_setting: u8,
}

macro_rules! append {
    ($iter:ident, $value:expr) => {
        *($iter.next().ok_or(UsbError::BufferOverflow)?.1) = $value;
    };
}

macro_rules! append_u24le {
    ($iter:ident, $value:expr) => {
        append!($iter, $value as u8);
        append!($iter, ($value >> 8) as u8);
        append!($iter, ($value >> 16) as u8);
    };
}

impl<'a, B: UsbBus, D: EndpointDirection> AudioStream<'a, B, D> {
    fn write_ac_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {
        let is_input = self.endpoint.address().direction() == UsbDirection::In;
        let terminal_type: u16 = self.stream_config.terminal_type.into();
        let id_offset = if is_input { 0 } else { 4 };

        // write Input Terminal Descriptor (12 bytes)
        let tt = if is_input {
            terminal_type
        } else {
            TerminalType::UsbStreaming.into()
        }
        .to_le_bytes();

        writer.write(
            CS_INTERFACE,
            &[
                INPUT_TERMINAL,                // bDescriptorSubtype
                ID_INPUT_TERMINAL + id_offset, // bTerminalID
                tt[0],                         // wTerminalType
                tt[1],
                0x00,                        // bAssocTerminal
                self.stream_config.channels, // bNrChannels
                0x03,
                0x00, // wChannelConfig: Left Front and Right Front
                0x00, // iChannelNames
                0x00, // iTerminal
            ],
        )?;

        // write Output Terminal Descriptor (9 bytes)
        let tt = if is_input {
            TerminalType::UsbStreaming.into()
        } else {
            terminal_type
        }
        .to_le_bytes();
        writer.write(
            CS_INTERFACE,
            &[
                OUTPUT_TERMINAL,                // bDescriptorSubtype
                ID_OUTPUT_TERMINAL + id_offset, // bTerminalID
                tt[0],                          // wTerminalType
                tt[1],
                0x00,                          // bAssocTerminal
                ID_INPUT_TERMINAL + id_offset, // bSourceID
                0x00,                          // iTerminal
            ],
        )
    }

    fn write_as_and_ep_descriptors(&self, writer: &mut DescriptorWriter) -> usb_device::Result<()> {
        let is_input = self.endpoint.address().direction() == UsbDirection::In;
        let id_offset = if is_input { 0 } else { 4 };
        // Standard AS Interface Descriptor (Alt. Set. 0)
        writer.interface(self.interface, AUDIO, AUDIOSTREAMING, 0x00)?;

        // Standard AS Interface Descriptor (Alt. Set. 1)
        writer.interface_alt(self.interface, 0x01, AUDIO, AUDIOSTREAMING, 0x00, None)?;

        // Class-specific AS General Interface Descriptor
        let terminal_link = id_offset
            + if is_input {
                ID_OUTPUT_TERMINAL
            } else {
                ID_INPUT_TERMINAL
            };
        writer.write(
            CS_INTERFACE,
            &[
                AS_GENERAL,    // bDescriptorSubtype:
                terminal_link, // bTerminalLink
                0x01,          // bDelay
                PCM as u8,
                (PCM >> 8) as u8, // wFormatTag
            ],
        )?;

        // Type 1 Format Type Descriptor
        let mut format_desc = [0x00u8; 128];
        let mut iter = format_desc.iter_mut().enumerate();
        append!(iter, FORMAT_TYPE); // bDescriptorSubtype;
        append!(iter, FORMAT_TYPE_I); // bFormatType
        append!(iter, self.stream_config.channels); // bNrChannels
        append!(
            iter,
            match self.stream_config.format {
                // bSubFrameSize
                Format::S16le => 2,
                Format::S24le => 3,
            }
        );
        append!(
            iter,
            match self.stream_config.format {
                // bBitResolution
                Format::S16le => 16,
                Format::S24le => 24,
            }
        );
        match self.stream_config.rates {
            Rates::Continuous(min, max) => {
                append!(iter, 0x00); // bSamFreqType
                append_u24le!(iter, min);
                append_u24le!(iter, max);
            }
            Rates::Discrete(rates) => {
                append!(iter, rates.len() as u8); // bSamFreqType
                for rate in rates {
                    append_u24le!(iter, *rate);
                }
            }
        }
        let length = iter.next().unwrap().0;
        writer.write(CS_INTERFACE, &format_desc[..length])?;

        // Standard Endpoint Descriptor
        writer.endpoint(&self.endpoint)?;

        // Class-specific Isoc. Audio Data Endpoint Descriptor
        writer.write(
            0x25,
            &[
                // bDescriptorType: CS_ENDPOINT
                0x01, // bDescriptorSubtype: GENERAL
                0x00, // bmAttributes
                0x00, // bLockDelayUnits
                0x00, 0x00, // wLockDelay
            ],
        )
    }
}

/// Builder class to create an `AudioClass` structure.
pub struct AudioClassBuilder<'a> {
    input: Option<StreamConfig<'a>>,
    output: Option<StreamConfig<'a>>,
}

impl<'a> AudioClassBuilder<'a> {
    /// Create a new AudioClassBuilder
    pub fn new() -> AudioClassBuilder<'static> {
        AudioClassBuilder {
            input: None,
            output: None,
        }
    }

    /// Configure the input audio stream according to a `StreamConfig`.
    /// At most one input stream can be configured. When calling this method
    /// multiple times, the last call matters.
    pub fn input(self, input: StreamConfig<'a>) -> AudioClassBuilder<'a> {
        AudioClassBuilder {
            input: Some(input),
            output: self.output,
        }
    }

    /// Configure the output audio stream according to a `StreamConfig`.
    /// At most one output stream can be configured. When calling this method
    /// multiple times, the last call matters.
    pub fn output(self, output: StreamConfig<'a>) -> AudioClassBuilder<'a> {
        AudioClassBuilder {
            input: self.input,
            output: Some(output),
        }
    }

    /// Create the `AudioClass` structure
    pub fn build<B: UsbBus>(self, alloc: &'a UsbBusAllocator<B>) -> Result<AudioClass<'a, B>> {
        let control_iface = alloc.interface();
        let mut ac = AudioClass {
            control_iface,
            input: None,
            output: None,
        };
        if let Some(stream_config) = self.input {
            let interface = alloc.interface();
            let endpoint =
                alloc.alloc(None, EndpointType::Isochronous, stream_config.ep_size, 1)?;
            let alt_setting = DEFAULT_ALTERNATE_SETTING;
            ac.input = Some(AudioStream {
                stream_config,
                interface,
                endpoint,
                alt_setting,
            })
        }

        if let Some(stream_config) = self.output {
            let interface = alloc.interface();
            let endpoint =
                alloc.alloc(None, EndpointType::Isochronous, stream_config.ep_size, 1)?;
            let alt_setting = DEFAULT_ALTERNATE_SETTING;
            ac.output = Some(AudioStream {
                stream_config,
                interface,
                endpoint,
                alt_setting,
            })
        }

        Ok(ac)
    }
}

/// USB device class for audio devices.
///
/// This device class based on the "Universal Serial Bus Device Class Definition
/// for Audio Devices", Release 1.0. It supports one input stream and/or one
/// output stream.
pub struct AudioClass<'a, B: UsbBus> {
    control_iface: InterfaceNumber,
    input: Option<AudioStream<'a, B, In>>,
    output: Option<AudioStream<'a, B, Out>>,
}

impl<B: UsbBus> AudioClass<'_, B> {
    /// Read audio frames as output by the host. Returns an Error if no output
    /// stream has been configured.
    pub fn read(&self, data: &mut [u8]) -> Result<usize> {
        if let Some(ref info) = self.output {
            info.endpoint.read(data).map_err(Error::UsbError)
        } else {
            Err(Error::StreamNotInitialized)
        }
    }

    /// Write audio frames to be input by the host. Returns an Error when no
    /// input stream has been configured.
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        if let Some(ref info) = self.input {
            info.endpoint.write(data).map_err(Error::UsbError)
        } else {
            Err(Error::StreamNotInitialized)
        }
    }

    /// Get current Alternate Setting of the input stream. Returns an error if
    /// the stream is not configured.
    pub fn input_alt_setting(&self) -> Result<u8> {
        self.input
            .as_ref()
            .ok_or(Error::StreamNotInitialized)
            .map(|si| si.alt_setting)
    }

    /// Get current Alternate Setting of the output stream. Returns an error if
    /// the stream is not configured.
    pub fn output_alt_setting(&self) -> Result<u8> {
        self.output
            .as_ref()
            .ok_or(Error::StreamNotInitialized)
            .map(|si| si.alt_setting)
    }
}

impl<B: UsbBus> UsbClass<B> for AudioClass<'_, B> {
    fn get_configuration_descriptors(
        &self,
        writer: &mut DescriptorWriter,
    ) -> usb_device::Result<()> {
        writer.interface(self.control_iface, AUDIO, AUDIOCONTROL, 0x00)?;

        // write Class-specific Audio Control (AC) Interface Descriptors
        let mut in_collection = 0u8;
        if self.input.is_some() {
            in_collection += 1;
        }
        if self.output.is_some() {
            in_collection += 1;
        }
        let total_length = 8u16 + (1 + 21) * in_collection as u16;

        let mut ac_header = [
            HEADER, // bDescriptorSubtype
            0x00,
            0x01, // bcdADC
            total_length as u8,
            (total_length >> 8) as u8, // wTotalLength
            in_collection,             // number of AS interfaces
            0x00,
            0x00, // placeholders for baInterfaceNr
        ];
        let mut ndx = 6;
        if let Some(ref input) = self.input {
            ac_header[ndx] = input.interface.into();
            ndx += 1;
        }
        if let Some(ref output) = self.output {
            ac_header[ndx] = output.interface.into();
            ndx += 1;
        }
        writer.write(CS_INTERFACE, &ac_header[..ndx])?;
        if let Some(ref a) = self.input {
            a.write_ac_descriptors(writer)?;
        }
        if let Some(ref a) = self.output {
            a.write_ac_descriptors(writer)?;
        }

        // write Audio Streaming (AS) and endpoint (EP) descriptors
        if let Some(ref a) = self.input {
            a.write_as_and_ep_descriptors(writer)?;
        }
        if let Some(ref a) = self.output {
            a.write_as_and_ep_descriptors(writer)?;
        }
        Ok(())
    }

    fn control_in(&mut self, xfer: ControlIn<B>) {
        let req = xfer.request();
        if req.request_type == RequestType::Standard
            && req.recipient == Recipient::Interface
            && req.request == Request::GET_INTERFACE
            && req.length == 1
        {
            let iface = req.index as u8;
            if let Some(info) = self.input.as_ref() {
                if iface == info.interface.into() {
                    xfer.accept_with(&[info.alt_setting]).ok();
                    return;
                }
            }
            if let Some(info) = self.output.as_ref() {
                if iface == info.interface.into() {
                    xfer.accept_with(&[info.alt_setting]).ok();
                    return;
                }
            }
        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();
        if req.request_type == RequestType::Standard
            && req.recipient == Recipient::Interface
            && req.request == Request::SET_INTERFACE
        {
            let iface = req.index as u8;
            let alt_setting = req.value;

            if let Some(info) = self.input.as_mut() {
                if iface == info.interface.into() {
                    info.alt_setting = alt_setting as u8;
                    xfer.accept().ok();
                    return;
                }
            }
            if let Some(info) = self.output.as_mut() {
                if iface == info.interface.into() {
                    info.alt_setting = alt_setting as u8;
                    xfer.accept().ok();
                    return;
                }
            }
        }
    }
}
