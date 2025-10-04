#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
extern crate alloc;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use alloc_cortex_m::CortexMHeap;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m::interrupt;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m_rt::entry;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embedded_hal::digital::v2::OutputPin;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use panic_halt as _;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use rp2040_boot2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use rp2040_hal::{self as hal, clocks::Clock, gpio::FunctionSioOutput, pac, sio::Sio, watchdog::Watchdog};

#[cfg(all(target_arch = "arm", target_os = "none"))]
const HEAP_SIZE: usize = 8 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[link_section = ".boot2"]
#[used]
static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[entry]
fn main() -> ! {
  let heap_start = core::ptr::addr_of_mut!(HEAP) as *mut u8;
  interrupt::free(|_| unsafe {
    ALLOCATOR.init(heap_start as usize, HEAP_SIZE);
  });

  let mut pac = pac::Peripherals::take().unwrap();
  let mut watchdog = Watchdog::new(pac.WATCHDOG);

  let clocks = hal::clocks::init_clocks_and_plls(
    12_000_000,
    pac.XOSC,
    pac.CLOCKS,
    pac.PLL_SYS,
    pac.PLL_USB,
    &mut pac.RESETS,
    &mut watchdog,
  )
  .ok()
  .unwrap();

  let sio = Sio::new(pac.SIO);
  let pins = hal::gpio::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

  let mut led = pins.gpio25.into_function::<FunctionSioOutput>().into_push_pull_output();

  loop {
    led.set_high().ok();
    delay_ms(250, clocks.system_clock.freq().to_Hz());
    led.set_low().ok();
    delay_ms(250, clocks.system_clock.freq().to_Hz());
  }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn delay_ms(ms: u32, sys_hz: u32) {
  let cycles = (sys_hz / 1000) * ms;
  cortex_m::asm::delay(cycles);
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
  println!("This example targets RP2040. Build with --target thumbv6m-none-eabi.");
}
