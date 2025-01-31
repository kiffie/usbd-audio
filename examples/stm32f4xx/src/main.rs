#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

use stm32f4xx_hal::otg_fs::{UsbBus, USB};
use stm32f4xx_hal::gpio::alt::otg_fs::{Dm, Dp};
use stm32f4xx_hal::{pac, prelude::*};

use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
use usb_device::LangID;
use usbd_audio::{AudioClassBuilder, Format, StreamConfig, TerminalType};

use rtt_target::{rprintln, rtt_init_print};

static mut EP_MEMORY: [u32; 1024] = [0; 1024];

#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("Init");

    let dp = pac::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.MHz()) // Change this to the speed of the external crystal you're using
        .sysclk(48.MHz())
        .require_pll48clk()
        .freeze();

    let gpioa = dp.GPIOA.split();

    let usb = USB {
        usb_global: dp.OTG_FS_GLOBAL,
        usb_device: dp.OTG_FS_DEVICE,
        usb_pwrclk: dp.OTG_FS_PWRCLK,
        pin_dm: Dm::PA11(gpioa.pa11.into_alternate()),
        pin_dp: Dp::PA12(gpioa.pa12.into_alternate()),
        hclk: clocks.hclk(),
    };

    let usb_bus = UsbBus::new(usb, unsafe { &mut EP_MEMORY });

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
        .manufacturer("Josef Labs")
        .product("USB audio test")
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
                    rprintln!("RX len = {}", len)
                }
            }
        }
        if input_alt_setting != usb_audio.input_alt_setting().unwrap()
            || output_alt_setting != usb_audio.output_alt_setting().unwrap()
        {
            input_alt_setting = usb_audio.input_alt_setting().unwrap();
            output_alt_setting = usb_audio.output_alt_setting().unwrap();
            rprintln!("Alt. set. {} {}", input_alt_setting, output_alt_setting);
        }
        usb_audio.write(sinetab_le).ok();
    }
}

