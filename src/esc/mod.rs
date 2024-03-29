#![allow(dead_code)]

use crate::math;
use embassy_rp::pio::{Common, Config, Direction, Instance, Irq, PioPin, StateMachine};
use fixed::{traits::ToFixed, types::extra::U8, FixedU32};
use pio_proc::pio_file;

pub const MIN_PULSE_WIDTH: u32 = 1000;
pub const MAX_PULSE_WIDTH: u32 = 2000;
const PWM_SIGNAL_FREQ: u32 = 50;
const CLOCK_DIVIDER: u32 = 125_000_000 / (PWM_SIGNAL_FREQ * 136);
const REFRESH_INTERVAL: u32 = 20000;

fn percent_to_pulse_width(percent: f64) -> u32 {
  math::map_range(
    percent,
    (0.0, 1.0),
    (MIN_PULSE_WIDTH as f64, MAX_PULSE_WIDTH as f64),
  ) as u32
}

fn pulse_width_to_percent(pulse_width: u32) -> f64 {
  math::map_range(
    pulse_width as f64,
    (MIN_PULSE_WIDTH as f64, MAX_PULSE_WIDTH as f64),
    (0.0, 1.0),
  )
}

fn us_to_pio_cycles(us: u32) -> u32 {
  us * (embassy_rp::clocks::clk_sys_freq() / 1_000_000)
}

pub struct ESC<'d, T: Instance, const SM: usize> {
  pulse_width: u32,
  sm: StateMachine<'d, T, SM>,
  irq: Irq<'d, T, SM>,
  config: Config<'d, T>,
}

impl<'d, T: Instance, const SM: usize> ESC<'d, T, SM> {
  pub fn new(
    pio: &mut Common<'d, T>,
    mut sm: StateMachine<'d, T, SM>,
    irq: Irq<'d, T, SM>,
    pin: impl PioPin,
  ) -> Self {
    let pin = pio.make_pio_pin(pin);
    let prg = pio_file!("src/esc/esc.pio", select_program("esc"));
    let pins = [&pin];

    if sm.is_enabled() {
      sm.set_enable(false);
    }

    sm.set_pin_dirs(Direction::Out, &pins);

    let mut config = Config::default();
    // config.clock_divider = (125_000_000 / (100 * 136)).to_fixed();
    config.set_set_pins(&pins);
    config.use_program(&pio.load_program(&prg.program), &pins);

    Self {
      irq,
      sm,
      config,
      pulse_width: Default::default(),
    }
  }

  pub async fn attach(&mut self) {
    if !self.sm.is_enabled() {
      self.sm.set_enable(false);
      self.sm.set_config(&self.config);

      // Convert this into Rust code.
      // pio_sm_put_blocking(_pio, _sm, RP2040::usToPIOCycles(REFRESH_INTERVAL) / 3);

      self
        .sm
        .tx()
        .wait_push(us_to_pio_cycles(REFRESH_INTERVAL) / 3)
        .await;
      // self
      //   .set_pulse_width(us_to_pio_cycles(REFRESH_INTERVAL) / 3)
      //   .await;
      // self.set_frequency(50);

      unsafe {
        self.sm.exec_instr(
          pio::InstructionOperands::PULL {
            if_empty: false,
            block: false,
          }
          .encode(),
        );

        self.sm.exec_instr(
          pio::InstructionOperands::OUT {
            destination: pio::OutDestination::ISR,
            bit_count: 32,
          }
          .encode(),
        );
      }

      self.set_power(0.0).await;

      unsafe {
        self.sm.exec_instr(
          pio::InstructionOperands::PULL {
            if_empty: false,
            block: false,
          }
          .encode(),
        );

        self.sm.exec_instr(
          pio::InstructionOperands::MOV {
            destination: pio::MovDestination::X,
            source: pio::MovSource::OSR,
            op: pio::MovOperation::None,
          }
          .encode(),
        );
      }
      self.sm.set_enable(true);
    }

    self.set_power(0.0).await;
  }

  async fn detach(&mut self) {
    self.set_pulse_width(0).await;
    embassy_time::Timer::after_micros(5).await; // Avoid race condition

    self.sm.set_enable(false);
  }

  fn set_frequency(&mut self, freq: u32) {
    let clock_divider: FixedU32<U8> = freq.to_fixed();

    T::PIO
      .sm(SM)
      .clkdiv()
      .write(|w| w.0 = clock_divider.to_bits() << 8);

    self.sm.clkdiv_restart();
  }

  async fn set_pulse_width(&mut self, pulse_width: u32) {
    if self.sm.is_enabled() {
      self.pulse_width = pulse_width;
      crate::println!(
        "Pulse Width: {pulse_width}, Cycles: {}",
        us_to_pio_cycles(pulse_width) / 3
      );

      self.sm.clear_fifos(); // Remove any old updates that haven't yet taken effect
      self
        .sm
        .tx()
        .wait_push(us_to_pio_cycles(pulse_width) / 3)
        .await;
    }
  }

  /// Specify a value between 0.0 - 1.0
  pub async fn set_power(&mut self, percent: f64) {
    let pulse_width = percent_to_pulse_width(percent.clamp(0.0, 1.0));

    self.set_pulse_width(pulse_width).await
  }

  pub fn get_power(&self) -> f64 {
    if !self.sm.is_enabled() {
      0.0
    } else {
      pulse_width_to_percent(self.pulse_width.clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH))
    }
  }
}
