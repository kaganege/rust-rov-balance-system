#![no_std]
#![no_main]

use embassy_executor as executor;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::pio;
use embassy_rp::usb;
use embassy_time::Timer;
use esc::ESC;

mod esc;
mod math;
mod on_drop;

macro_rules! println {
  ( $( $x:expr ),+ ) => {
    log::info!($($x),+)
  };
}

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
  USBCTRL_IRQ => usb::InterruptHandler<USB>;
  PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[executor::task]
async fn logger_task(driver: usb::Driver<'static, USB>) {
  embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[executor::main]
async fn main(spawner: executor::Spawner) {
  let p = embassy_rp::init(Default::default());

  let usb_driver = usb::Driver::new(p.USB, Irqs);
  spawner.spawn(logger_task(usb_driver)).unwrap();

  let pio::Pio {
    mut common,
    irq0: irq,
    sm0: sm,
    ..
  } = pio::Pio::new(p.PIO0, Irqs);
  let esc = ESC::new(&mut common, sm, irq, p.PIN_2);

  loop {
    println!("Power: {}", esc.get_power());
    // I don't know, but it doesn't work if we don't wait.
    Timer::after(Default::default()).await;
  }
}
