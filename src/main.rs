#![no_std]
#![no_main]

use cyw43_pio::PioSpi;
use embassy_executor as executor;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0, PIO1, USB};
use embassy_rp::pio;
use embassy_rp::usb;
use embassy_time::Timer;
use esc::ESC;
use static_cell::StaticCell;

mod esc;
mod math;
mod on_drop;
mod usb_serial;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
  USBCTRL_IRQ => usb::InterruptHandler<USB>;
  PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
  PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
});

#[executor::task]
async fn logger_task(driver: usb::Driver<'static, USB>) {
  embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn wifi_task(
  runner: cyw43::Runner<
    'static,
    Output<'static, PIN_23>,
    PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>,
  >,
) -> ! {
  runner.run().await
}

#[executor::task]
async fn blink_task(control: &'static mut cyw43::Control<'_>) -> ! {
  loop {
    control.gpio_set(0, Level::High.into()).await;
    Timer::after_millis(500).await;
    control.gpio_set(0, Level::Low.into()).await;
    Timer::after_millis(500).await;
  }
}

#[executor::main]
async fn main(spawner: executor::Spawner) {
  let p = embassy_rp::init(Default::default());

  let usb_driver = usb::Driver::new(p.USB, Irqs);
  spawner.spawn(logger_task(usb_driver)).unwrap();

  let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
  let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

  let pwr = Output::new(p.PIN_23, Level::Low);
  let cs = Output::new(p.PIN_25, Level::High);

  let mut pio = pio::Pio::new(p.PIO0, Irqs);
  let spi = PioSpi::new(
    &mut pio.common,
    pio.sm0,
    pio.irq0,
    cs,
    p.PIN_24,
    p.PIN_29,
    p.DMA_CH0,
  );

  static STATE: StaticCell<cyw43::State> = StaticCell::new();
  let state = STATE.init(cyw43::State::new());
  let (_net_device, control, runner) = cyw43::new(state, pwr, spi, fw).await;
  spawner.spawn(wifi_task(runner)).unwrap();

  static CONTROL: StaticCell<cyw43::Control<'_>> = StaticCell::new();
  let control = CONTROL.init(control);
  control.init(clm).await;
  control
    .set_power_management(cyw43::PowerManagementMode::PowerSave)
    .await;

  spawner.spawn(blink_task(control)).unwrap();

  let mut pio1 = pio::Pio::new(p.PIO1, Irqs);
  let mut motor = ESC::new(&mut pio1.common, pio1.sm0, pio1.irq0, p.PIN_2);

  motor.attach().await;

  let mut increase = true;

  loop {
    let power = motor.get_power();

    if power == 0.0 {
      increase = true;
    } else if power == 1.0 {
      increase = false;
    }

    println!("Power: {:.2}", power);

    motor
      .set_power(if increase { power + 0.01 } else { power - 0.01 })
      .await;

    // println!("Power: {}", power);

    Timer::after_millis(250).await;
  }
}
