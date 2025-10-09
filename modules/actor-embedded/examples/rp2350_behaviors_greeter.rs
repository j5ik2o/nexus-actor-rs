#![cfg_attr(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board", not(feature = "embedded_arc")))]
compile_error!("rp2350_behaviors_greeter 例をビルドするには `embedded_arc` および `rp2350-board` フィーチャが必要です。");

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
extern crate alloc;

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use alloc_cortex_m::CortexMHeap;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use cortex_m::{asm, interrupt};
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use embedded_hal::digital::v2::OutputPin;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use panic_halt as _;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use rp235x_hal::{self as hal, clocks::init_clocks_and_plls, gpio::PinState, pac, sio::Sio, watchdog::Watchdog, Clock};

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use hal::entry;

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use nexus_actor_core_rs::{ActorSystem, Behaviors, MailboxOptions, Props};
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use nexus_actor_embedded_rs::ArcMailboxFactory;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
use nexus_utils_embedded_rs::{ArcCsStateCell, Element, StateCell};

#[derive(Clone, Copy, Debug)]
enum Command {
  Greet(&'static str),
  Report,
  Stop,
}

impl Element for Command {}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
const HEAP_SIZE: usize = 32 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
#[entry]
fn main() -> ! {
  let heap_start = core::ptr::addr_of_mut!(HEAP) as usize;
  interrupt::free(|_| unsafe {
    ALLOCATOR.init(heap_start, HEAP_SIZE);
  });

  let mut pac = pac::Peripherals::take().unwrap();
  let mut watchdog = Watchdog::new(pac.WATCHDOG);

  let clocks = init_clocks_and_plls(
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

  let led_pin = ArcCsStateCell::new(
    pins
      .gpio6
      // Raspberry Pi Pico 2 (RP2350) のオンボード LED は GPIO6.
      .into_push_pull_output_in_state(PinState::Low),
  );
  let system_clock_hz = clocks.system_clock.freq().to_Hz();

  let mut system: ActorSystem<Command, ArcMailboxFactory<CriticalSectionRawMutex>> =
    ActorSystem::new(ArcMailboxFactory::default());
  let mut root = system.root_context();

  let behavior_led = led_pin.clone();
  let behavior_clock_hz = system_clock_hz;

  let greeter_props = Props::with_behavior(MailboxOptions::default(), move || {
    let setup_led = behavior_led.clone();
    let clock_hz = behavior_clock_hz;
    Behaviors::setup(move |ctx| {
      let ready_blinks = ((ctx.actor_id().0 as u8) % 4 + 1) as usize;
      blink_repeated(&setup_led, ready_blinks, 50, 50, clock_hz);

      let message_led = setup_led.clone();
      let mut greeted = 0usize;
      Behaviors::receive_message(move |msg: Command| match msg {
        Command::Greet(name) => {
          blink_message(&message_led, &BlinkPattern::short_pulse(), clock_hz);
          greeted = greeted.wrapping_add(1);
          let name_len = name.len().min(6) as u8;
          if name_len > 0 {
            blink_repeated(&message_led, name_len as usize, 80, 80, clock_hz);
          }
          Behaviors::same()
        }
        Command::Report => {
          let flashes = core::cmp::max(greeted, 1);
          blink_repeated(&message_led, flashes, 180, 120, clock_hz);
          Behaviors::same()
        }
        Command::Stop => {
          blink_message(&message_led, &BlinkPattern::shutdown(), clock_hz);
          Behaviors::stopped()
        }
      })
    })
  });

  let greeter = root.spawn(greeter_props).expect("spawn greeter");
  greeter.tell(Command::Greet("Ares")).expect("greet Ares");
  greeter.tell(Command::Greet("Boreas")).expect("greet Boreas");
  greeter.tell(Command::Report).expect("report greetings");
  greeter.tell(Command::Stop).expect("stop greeter");

  system.run_until_idle().expect("drain mailbox");
  set_led_state(&led_pin, false);

  loop {
    asm::wfi();
  }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
struct BlinkPattern {
  on_ms: u32,
  off_ms: u32,
  repeat: usize,
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
impl BlinkPattern {
  const fn new(on_ms: u32, off_ms: u32, repeat: usize) -> Self {
    Self { on_ms, off_ms, repeat }
  }

  const fn short_pulse() -> Self {
    Self::new(100, 60, 1)
  }

  const fn shutdown() -> Self {
    Self::new(250, 200, 4)
  }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
fn blink_message<T>(led: &ArcCsStateCell<T>, pattern: &BlinkPattern, sys_hz: u32)
where
  T: OutputPin, {
  blink_repeated(led, pattern.repeat, pattern.on_ms, pattern.off_ms, sys_hz);
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
fn blink_repeated<T>(led: &ArcCsStateCell<T>, times: usize, on_ms: u32, off_ms: u32, sys_hz: u32)
where
  T: OutputPin, {
  for _ in 0..times {
    set_led_state(led, true);
    delay_ms(on_ms, sys_hz);
    set_led_state(led, false);
    delay_ms(off_ms, sys_hz);
  }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
fn set_led_state<T>(led: &ArcCsStateCell<T>, high: bool)
where
  T: OutputPin, {
  let mut guard = led.borrow_mut();
  if high {
    guard.set_high().ok();
  } else {
    guard.set_low().ok();
  }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "rp2350-board"))]
fn delay_ms(ms: u32, sys_hz: u32) {
  if ms == 0 {
    return;
  }
  let cycles = (sys_hz / 1_000).saturating_mul(ms.max(1));
  asm::delay(cycles);
}

#[cfg(not(all(target_arch = "arm", target_os = "none", feature = "rp2350-board")))]
fn main() {
  println!(
    "RP2350 Behaviors Greeter example は thumbv8m.main-none-eabihf ターゲット向けです。\n\
     `cargo run --release --example rp2350_behaviors_greeter --target thumbv8m.main-none-eabihf \\\n\
      \t--no-default-features --features alloc,embedded_arc,rp2350-board` を実行してください。"
  );
}
