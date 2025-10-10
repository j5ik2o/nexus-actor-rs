#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use defmt_test::tests;
use rp235x_hal as hal;

#[tests]
mod hardware {
  use super::*;

  #[test]
  fn smoke_placeholder() {
    info!("RP2350 hardware smoke test placeholder");
    assert!(true);
  }
}
