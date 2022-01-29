//! Simple USB Audio example for PIC32MX270 (28 pins)
//!
//! Simulates a microphone that emits a 1 kHz tone and a dummy audio output and
//! prints the payload length of each thousand received audio frame an reports
//! changes of the alternate settings.
//!
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use alloc_pic32::Pic32Heap;
use core::fmt::Write;
use embedded_hal::{blocking::delay::DelayMs, digital::v2::*};
use mips_rt::entry;
use panic_halt as _;
use pic32_config_sector::pic32mx2xx::*;
use pic32_hal::{
    clock::Osc,
    coretimer::Delay,
    gpio::GpioExt,
    pac,
    pps::{MapPin, NoPin, PpsExt},
    pps_no_pin,
    time::U32Ext,
    uart::Uart,
    usb::UsbBus,
};
use usb_device::prelude::*;
use usbd_audio::{AudioClassBuilder, Format, StreamConfig, TerminalType};

// PIC32 configuration registers for PIC32MX2xx
#[link_section = ".configsfrs"]
#[used]
pub static CONFIGSFRS: ConfigSector = ConfigSector::default()
    // DEVCFG3
    .FVBUSONIO(FVBUSONIO::OFF)
    .FUSBIDIO(FUSBIDIO::OFF)
    // DEVCFG2
    .FPLLODIV(FPLLODIV::DIV_2)
    .UPLLEN(UPLLEN::ON)
    .UPLLIDIV(UPLLIDIV::DIV_2)
    .FPLLMUL(FPLLMUL::MUL_20)
    .FPLLIDIV(FPLLIDIV::DIV_2)
    // DEVCFG 1
    .FWDTEN(FWDTEN::OFF)
    .WDTPS(WDTPS::PS1048576)
    .FPBDIV(FPBDIV::DIV_1)
    .POSCMOD(POSCMOD::XT)
    .FSOSCEN(FSOSCEN::OFF)
    .FNOSC(FNOSC::PRIPLL)
    // DEVCFG 0
    .JTAGEN(JTAGEN::OFF)
    .build();

#[global_allocator]
static ALLOCATOR: Pic32Heap = Pic32Heap::empty();

#[entry]
fn main() -> ! {
    // Initialize the allocator BEFORE you use it
    let start = mips_rt::heap_start() as usize;
    let size = 8192; // in bytes
    unsafe { ALLOCATOR.init(start, size) }

    let p = pac::Peripherals::take().unwrap();
    let parts = p.PORTB.split();
    let vpins = p.PPS.split();

    let sysclock = 40_000_000_u32.hz();
    let clock = Osc::new(p.OSC, sysclock);
    let mut timer = Delay::new(sysclock);

    let txd = parts
        .rb0
        .into_push_pull_output()
        .map_pin(vpins.outputs.u2tx);
    //let rxd = NoPin::new().map_pin(vpins.inputs.u2rx);
    let rxd = pps_no_pin!(vpins.inputs.u2rx);
    let uart = Uart::uart2(p.UART2, &clock, 115200, rxd, txd);

    timer.delay_ms(10u32);
    let (mut tx, _) = uart.split();
    writeln!(tx, "USB audio test").unwrap();

    let mut led = parts.rb5.into_push_pull_output();
    led.set_high().unwrap();

    let usb_bus = UsbBus::new(p.USB);

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

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .max_packet_size_0(64)
        .manufacturer("Kiffie Labs")
        .product("Audio port")
        .serial_number("42")
        .build();

    let sinetab = [
        0i16, 4276, 8480, 12539, 16383, 19947, 23169, 25995, 28377, 30272, 31650, 32486, 32767,
        32486, 31650, 30272, 28377, 25995, 23169, 19947, 16383, 12539, 8480, 4276, 0, -4276, -8480,
        -12539, -16383, -19947, -23169, -25995, -28377, -30272, -31650, -32486, -32767, -32486,
        -31650, -30272, -28377, -25995, -23169, -19947, -16383, -12539, -8480, -4276,
    ];
    let sinetab_le = unsafe { &*(&sinetab as *const _ as *const [u8; 96]) };

    let mut ctr = 0;
    let mut input_alt_setting = 0;
    let mut output_alt_setting = 0;
    loop {
        if usb_dev.poll(&mut [&mut usb_audio]) {
            let mut buf = [0u8; 1024];
            if let Ok(len) = usb_audio.read(&mut buf) {
                ctr += 1;
                if ctr >= 1000 {
                    ctr = 0;
                    writeln!(tx, "RX len = {}", len).unwrap();
                }
            }
        }
        if input_alt_setting  != usb_audio.input_alt_setting().unwrap() ||
           output_alt_setting != usb_audio.output_alt_setting().unwrap()
        {
            input_alt_setting = usb_audio.input_alt_setting().unwrap();
            output_alt_setting = usb_audio.output_alt_setting().unwrap();
            writeln!(tx, "Alt. set. {} {}", input_alt_setting, output_alt_setting).unwrap();
        }
        usb_audio.write(sinetab_le).ok();
    }
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Cannot allocate heap memory: {:?}", layout);
}
