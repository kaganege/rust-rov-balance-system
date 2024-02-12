#![allow(dead_code)]

use crate::math;
use crate::on_drop::OnDrop;
use embassy_rp::pio::{Common, Config, Direction, Instance, Irq, PioPin, StateMachine};
use fixed::{traits::ToFixed, types::extra::U8, FixedU32};
use pio_proc::pio_file;

pub const MIN_PULSE_WIDTH: u32 = 1000;
pub const MAX_PULSE_WIDTH: u32 = 2000;
const PWM_SIGNAL_FREQ: u32 = 50;
const CLOCK_DIVIDER: u32 = 125_000_000 / (PWM_SIGNAL_FREQ * 136);
const REFRESH_INTERVAL: u32 = 20000;

fn percent_to_pulse_width(percent: u8) -> u32 {
  math::map_range(percent as u32, (0, 100), (MIN_PULSE_WIDTH, MAX_PULSE_WIDTH))
}

fn pulse_width_to_percent(pulse_width: u32) -> u8 {
  math::map_range(pulse_width, (MIN_PULSE_WIDTH, MAX_PULSE_WIDTH), (0, 100)) as u8
}

pub struct ESC<'d, T: Instance, const SM: usize> {
  pulse_width: u32,
  sm: StateMachine<'d, T, SM>,
  irq: Irq<'d, T, SM>,
}

impl<'d, T: Instance, const SM: usize> ESC<'d, T, SM> {
  pub fn new(
    pio: &mut Common<'d, T>,
    mut sm: StateMachine<'d, T, SM>,
    irq: Irq<'d, T, SM>,
    pin: impl PioPin,
  ) -> Self {
    let clock_divider: FixedU32<U8> = CLOCK_DIVIDER.to_fixed();
    let prg = pio_file!("src/esc/esc.pio", select_program("esc"));
    let pin = pio.make_pio_pin(pin);
    let pins = [&pin];
    sm.set_pin_dirs(Direction::Out, &pins);

    let mut config = Config::default();
    config.set_out_pins(&pins);
    config.clock_divider = (125_000_000 / (100 * 136)).to_fixed();
    config.use_program(&pio.load_program(&prg.program), &pins);
    sm.set_config(&config);
    sm.set_enable(true);

    // Set frequency to 50 Hz
    T::PIO
      .sm(SM)
      .clkdiv()
      .write(|w| w.0 = clock_divider.to_bits() << 8);
    sm.clkdiv_restart();

    Self {
      irq,
      sm,
      pulse_width: MIN_PULSE_WIDTH,
    }
  }

  pub async fn set_pulse_width(&mut self, pulse_width: u32) {
    assert!(
      pulse_width < MIN_PULSE_WIDTH,
      "Pulse width must be in {MIN_PULSE_WIDTH}-{MAX_PULSE_WIDTH}"
    );
    assert!(
      pulse_width > MAX_PULSE_WIDTH,
      "Pulse width must be in {MIN_PULSE_WIDTH}-{MAX_PULSE_WIDTH}"
    );

    self.sm.tx().wait_push(pulse_width).await;
    let drop = OnDrop::new(|| {
      self.sm.clear_fifos();
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
    });
    self.irq.wait().await;
    self.pulse_width = pulse_width;
    drop.defuse();
  }

  pub fn get_pulse_width(&self) -> u32 {
    self.pulse_width
  }

  pub async fn set_power(&mut self, percent: u8) {
    assert!(percent > 100, "Power must be in 0-100");

    let pulse_width = percent_to_pulse_width(percent);
    self.set_pulse_width(pulse_width).await
  }

  pub fn get_power(&self) -> u8 {
    pulse_width_to_percent(self.pulse_width)
  }
}
