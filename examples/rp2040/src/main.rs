//! Simple USB Audio example for the rp2040 (e.g. Raspberry Pi Pico)
//!
//! Simulates a microphone that emits a 1 kHz tone and a dummy audio output and
//! prints the payload length of each thousand received audio frame an reports
//! changes of the alternate settings.
//!
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

use embedded_hal::digital::v2::OutputPin;
use hal::{
    clocks::init_clocks_and_plls,
    gpio::FunctionUart,
    pac,
    sio::Sio,
    uart::{self, UartPeripheral},
    usb::UsbBus,
    watchdog::Watchdog,
};
use rp2040_hal::{self as hal, Clock};

use usb_device::class_prelude::UsbBusAllocator;

use usb_device::prelude::*;
use usbd_audio::{AudioClassBuilder, Format, StreamConfig, TerminalType};

use core::fmt::Write;
use core::writeln;

#[link_section = ".boot2"]
#[used]
pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let tx_pin = pins.gpio4.into_function::<FunctionUart>();
    let rx_pin = pins.gpio5.into_function::<FunctionUart>();
    let mut uart = UartPeripheral::<_, _, _>::new(pac.UART1, (tx_pin, rx_pin), &mut pac.RESETS)
        .enable(
            uart::common_configs::_115200_8_N_1,
            clocks.peripheral_clock.freq(),
        )
        .unwrap();

    writeln!(uart, "USB audio test").unwrap();

    let mut led_pin = pins.gpio11.into_push_pull_output();
    led_pin.set_high().unwrap();
    //let button_pin = pins.gpio12.into_pull_up_input();

    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    let mut usb_audio = AudioClassBuilder::new()
        .input(
            StreamConfig::new_discrete(Format::S16le, 1, &[48000], TerminalType::InMicrophone)
                .unwrap(),
        )
        .output(
            StreamConfig::new_discrete(
                Format::S24le,
                2,
                &[44100, 48000, 96000],
                TerminalType::OutSpeaker,
            )
            .unwrap(),
        )
        .build(&usb_bus)
        .unwrap();

    let string_descriptor = StringDescriptors::new(LangID::EN_US)
        .manufacturer("Kiffie Labs")
        .product("Audio port")
        .serial_number("42");

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .max_packet_size_0(64)
        .unwrap()
        .strings(&[string_descriptor])
        .unwrap()
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
                    writeln!(uart, "RX len = {}", len).unwrap();
                }
            }
        }
        if input_alt_setting != usb_audio.input_alt_setting().unwrap()
            || output_alt_setting != usb_audio.output_alt_setting().unwrap()
        {
            input_alt_setting = usb_audio.input_alt_setting().unwrap();
            output_alt_setting = usb_audio.output_alt_setting().unwrap();
            writeln!(
                uart,
                "Alt. set. {} {}",
                input_alt_setting, output_alt_setting
            )
            .unwrap();
        }
        usb_audio.write(sinetab_le).ok();
    }
}
