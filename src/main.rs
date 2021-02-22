//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

//#![deny(unsafe_code)]
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

//extern crate panic_semihosting;
extern crate panic_halt;

use nb::block;

#[macro_use]
extern crate alloc;
use alloc_cortex_m::CortexMHeap;
use core::alloc::Layout;

use accelerometer::Accelerometer;
use lis3dh::{Lis3dh, SlaveAddr};
use micromath::F32Ext;

use cortex_m_rt::entry;
use embedded_graphics::{
    fonts::Text, pixelcolor::BinaryColor, prelude::*, style::TextStyleBuilder,
};
use profont::ProFont18Point;
use shared_bus;
use ssd1306::{mode::GraphicsMode, Builder, I2CDIBuilder};
use stm32f1xx_hal::{
    i2c::{BlockingI2c, DutyCycle, Mode},
    pac,
    prelude::*,
    timer::Timer,
};

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

const MAX_LENGTH: usize = 5;
const DEC_FIGS: usize = 2;

const LOOPS_PER_SEC: u32 = 100;

#[entry]
fn main() -> ! {
    // Initialize the allocator BEFORE you use it
    let start = cortex_m_rt::heap_start() as usize;
    let size = 1024; // in bytes
    unsafe { ALLOCATOR.init(start, size) }

    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    // Configure the syst timer to trigger an update every second
    let mut timer = Timer::syst(cp.SYST, &clocks).start_count_down(LOOPS_PER_SEC.hz());

    // I2C config
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);

    let scl1 = gpiob.pb8.into_alternate_open_drain(&mut gpiob.crh);
    let sda1 = gpiob.pb9.into_alternate_open_drain(&mut gpiob.crh);

    let i2c1 = BlockingI2c::i2c1(
        dp.I2C1,
        (scl1, sda1),
        &mut afio.mapr,
        Mode::Fast {
            frequency: 400_000.hz(),
            duty_cycle: DutyCycle::Ratio2to1,
        },
        clocks,
        &mut rcc.apb1,
        1000,
        10,
        1000,
        1000,
    );

    // Share the I2C bus across all attached devices
    // Otherwise the first device would take ownership of the bus
    // and no other devices could be attached
    let bus = shared_bus::BusManagerSimple::new(i2c1);

    let interface = I2CDIBuilder::new().init(bus.acquire_i2c());

    let mut disp: GraphicsMode<_, _> = Builder::new().connect(interface).into();
    disp.init().unwrap();

    disp.clear();
    disp.flush().unwrap();

    // 18 Point font is 12 x 22 pixels
    let text_style = TextStyleBuilder::new(ProFont18Point)
        .text_color(BinaryColor::On)
        .build();

    let mut lis3dh = Lis3dh::new(bus.acquire_i2c(), SlaveAddr::Default).unwrap();
    lis3dh.set_range(lis3dh::Range::G8).unwrap();

    let mut peak = 0.0;

    loop {
        let accel = lis3dh.accel_norm().unwrap();
        let (x, y, z) = (accel.x, accel.y, accel.z);
        let abs_mag = ((x * x + y * y + z * z).sqrt() - 1.0).abs();
        if abs_mag > peak {
            peak = abs_mag;
        }

        // let x_text = format!("X: {:>width$.dec$}", x, width = MAX_LENGTH, dec = DEC_FIGS);
        // let y_text = format!("Y: {:>width$.dec$}", y, width = MAX_LENGTH, dec = DEC_FIGS);
        // let z_text = format!("Z: {:>width$.dec$}", z, width = MAX_LENGTH, dec = DEC_FIGS);

        let cur_text = format!(
            "Cur: {:>width$.dec$}",
            abs_mag,
            width = MAX_LENGTH,
            dec = DEC_FIGS
        );

        let peak_text = format!(
            "Max: {:>width$.dec$}",
            peak,
            width = MAX_LENGTH,
            dec = DEC_FIGS
        );

        disp.clear();

        // Text::new(&x_text, Point::zero())
        //     .into_styled(text_style)
        //     .draw(&mut disp)
        //     .unwrap();

        // Text::new(&y_text, Point::new(0, 23))
        //     .into_styled(text_style)
        //     .draw(&mut disp)
        //     .unwrap();

        // Text::new(&z_text, Point::new(0, 46))
        //     .into_styled(text_style)
        //     .draw(&mut disp)
        //     .unwrap();

        Text::new(&cur_text, Point::zero())
            .into_styled(text_style)
            .draw(&mut disp)
            .unwrap();

        Text::new(&peak_text, Point::new(0, 23))
            .into_styled(text_style)
            .draw(&mut disp)
            .unwrap();

        disp.flush().unwrap();

        block!(timer.wait()).unwrap();
    }
}

#[alloc_error_handler]
fn oom(_: Layout) -> ! {
    loop {}
}
