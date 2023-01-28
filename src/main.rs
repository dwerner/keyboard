#![no_std]
#![no_main]

use core::{convert::Infallible, panic::PanicInfo};

use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_time::{rate::Fraction, Clock, Instant};
use hal::{gpio::PinState, timer::CounterUs};
use rtt_target::{rprintln, rtt_init_print};
use stm32f4xx_hal::{
    self as hal,
    otg_fs::{UsbBus as SynopsysBus, UsbBusType, USB},
};
use usb_device::bus::UsbBusAllocator;
use usb_device::class_prelude::*;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use usbd_human_interface_device::device::keyboard::NKROBootKeyboardInterface;
use usbd_human_interface_device::page::Keyboard;
use usbd_human_interface_device::prelude::*;

use crate::hal::{pac, prelude::*};

struct DeviceClock {
    timer: CounterUs<pac::TIM1>,
}

impl Clock for DeviceClock {
    type T = u32;

    const SCALING_FACTOR: Fraction = Fraction::new(1, 1);

    fn try_now(&self) -> Result<Instant<Self>, embedded_time::clock::Error> {
        let now = self.timer.now();
        Ok(Instant::<Self>::new(now.ticks()))
    }
}

static mut EP_MEMORY: [u32; 1024] = [0; 1024];

mod keys {
    use super::*;
    use Keyboard::*;

    pub enum KeyMapping {
        Left([[Keyboard; 6]; 6]),
        Right([[Keyboard; 6]; 6]),
        Other,
    }

    impl KeyMapping {
        pub fn mapping(&self, line: usize, column: usize) -> Keyboard {
            match self {
                // Left side of the keyboard was wired from index to pinkie,
                // which reverses the order.
                KeyMapping::Left(mapping) => mapping[line][5 - column],
                KeyMapping::Right(mapping) => mapping[line][column],
                KeyMapping::Other => NoEventIndicated,
            }
        }
    }

    pub const RIGHT_KEYS: KeyMapping = KeyMapping::Right([
        [Keyboard6, Keyboard7, Keyboard8, Keyboard9, Keyboard0, Minus],
        [Y, U, I, O, P, Backslash],
        [H, J, K, L, Semicolon, Apostrophe],
        [N, M, Comma, Dot, ForwardSlash, RightShift],
        [
            ReturnEnter,
            Space,
            UpArrow,
            DownArrow,
            NoEventIndicated,
            NoEventIndicated,
        ],
        [
            F2,
            RightControl,
            LeftBrace,
            RightBrace,
            NoEventIndicated,
            NoEventIndicated,
        ],
    ]);

    pub const LEFT_KEYS: KeyMapping = KeyMapping::Left([
        [Equal, Keyboard1, Keyboard2, Keyboard3, Keyboard4, Keyboard5],
        [Tab, Q, W, E, R, T],
        [Grave, A, S, D, F, G],
        [LeftShift, Z, X, C, V, B],
        [
            NoEventIndicated,
            NoEventIndicated,
            LeftArrow,
            RightArrow,
            DeleteBackspace,
            LeftControl,
        ],
        [
            NoEventIndicated,
            NoEventIndicated,
            DeleteForward,
            End,
            Escape,
            LeftAlt,
        ],
    ]);
}

fn iterate_lines() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(25.MHz())
        .sysclk(84.MHz())
        .require_pll48clk()
        .freeze();

    let device_clock = DeviceClock {
        timer: dp.TIM1.counter_us(&clocks),
    };

    let gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();

    let mut line_0 = gpiob.pb0.into_open_drain_output_in_state(PinState::High);
    let mut line_1 = gpioa.pa7.into_open_drain_output_in_state(PinState::High);
    let mut line_2 = gpioa.pa15.into_open_drain_output_in_state(PinState::High);
    let mut line_3 = gpiob.pb13.into_open_drain_output_in_state(PinState::High);
    let mut line_4 = gpiob.pb14.into_open_drain_output_in_state(PinState::High);
    let mut line_5 = gpioa.pa10.into_open_drain_output_in_state(PinState::High);

    let mut col_0 = gpioa.pa0.into_pull_up_input();
    let mut col_1 = gpioa.pa1.into_pull_up_input();
    let mut col_2 = gpioa.pa2.into_pull_up_input();
    let mut col_3 = gpioa.pa3.into_pull_up_input();
    let mut col_4 = gpioa.pa4.into_pull_up_input();
    let mut col_5 = gpioa.pa5.into_pull_up_input();

    let cols: &mut [&mut dyn InputPin<Error = Infallible>] = &mut [
        &mut col_0, &mut col_1, &mut col_2, &mut col_3, &mut col_4, &mut col_5,
    ];

    let lines: &mut [&mut dyn OutputPin<Error = Infallible>] = &mut [
        &mut line_0,
        &mut line_1,
        &mut line_2,
        &mut line_3,
        &mut line_4,
        &mut line_5,
    ];

    let usb = USB {
        usb_global: dp.OTG_FS_GLOBAL,
        usb_device: dp.OTG_FS_DEVICE,
        usb_pwrclk: dp.OTG_FS_PWRCLK,
        pin_dm: gpioa.pa11.into_alternate(),
        pin_dp: gpioa.pa12.into_alternate(),
        hclk: clocks.hclk(),
    };

    let usb_alloc: UsbBusAllocator<SynopsysBus<USB>> =
        UsbBusType::new(usb, unsafe { &mut EP_MEMORY });

    let mut keyboard = UsbHidClassBuilder::new()
        .add_interface(NKROBootKeyboardInterface::default_config(&device_clock))
        .build(&usb_alloc);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_alloc, UsbVidPid(0x1209, 0x0001))
        .manufacturer("custom-keyboard-dwerner")
        .product("Dan's Keyboard")
        .serial_number("42")
        .build();

    let mut input_timer = dp.TIM2.counter_us(&clocks);
    input_timer.start(10.millis()).unwrap();

    let mut tick_timer = dp.TIM3.counter_us(&clocks);
    tick_timer.start(500.micros()).unwrap();

    rprintln!("starting keyboard");
    loop {
        // All keys marked as being pressed during this pass.
        let mut keys_pressed: [Keyboard; 6 * 6] = [Keyboard::NoEventIndicated; 6 * 6];

        for (line_index, line) in lines.iter_mut().enumerate() {
            line.set_low().unwrap();

            // waiting here eliminates a ghost keypress of the next line.
            nb::block!(tick_timer.wait()).unwrap();

            for (col_index, col) in cols.iter().enumerate() {
                keys_pressed[(col_index * 6) + line_index] = if col.is_low().unwrap() {
                    // Ask the key mapping what key should be written.
                    //let key = keys::LEFT_KEYS.mapping(line_index, col_index);
                    let key = keys::RIGHT_KEYS.mapping(line_index, col_index);
                    rprintln!("line: {} col: {} key {:?}", line_index, col_index, key);
                    key
                } else {
                    Keyboard::NoEventIndicated
                };
            }
            line.set_high().unwrap();
        }

        nb::block!(input_timer.wait()).unwrap();
        match keyboard.interface().write_report(&keys_pressed) {
            Err(UsbHidError::WouldBlock) => {}
            Err(UsbHidError::Duplicate) => {}
            Ok(()) => {}
            Err(e) => {
                panic!("Failed to write keyboard report: {:?}", e)
            }
        }

        nb::block!(tick_timer.wait()).unwrap();
        match keyboard.interface().tick() {
            Ok(()) => {}
            Err(UsbHidError::WouldBlock) => {}
            Err(e) => {
                panic!("Failed to process keyboard tick: {:?}", e)
            }
        }

        if usb_dev.poll(&mut [&mut keyboard]) {
            match keyboard.interface().read_report() {
                Ok(l) => {
                    rprintln!("read report {:?}", l)
                }
                Err(UsbError::WouldBlock) => {}
                Err(err) => {
                    rprintln!("error reading report {:?}", err)
                }
            }
        }
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();
    iterate_lines();
}

#[panic_handler]
#[inline(never)]
fn panic(info: &PanicInfo) -> ! {
    rprintln!("{}", info);
    loop {}
}
