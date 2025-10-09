#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
extern crate alloc;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use alloc::rc::Rc;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use alloc_cortex_m::CortexMHeap;
use core::cell::RefCell;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m::{asm, interrupt};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m_rt::entry;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embedded_hal::digital::v2::OutputPin;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use nexus_actor_core_rs::{ActorSystem, Behaviors, MailboxOptions, Props};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use nexus_actor_embedded_rs::LocalMailboxFactory;
use nexus_utils_embedded_rs::Element;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use panic_halt as _;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use rp2040_boot2;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use rp2040_hal::{
  self as hal,
  clocks::Clock,
  gpio::{pin::bank0::Gpio25, FunctionSioOutput},
  pac,
  sio::Sio,
  watchdog::Watchdog,
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
const HEAP_SIZE: usize = 16 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[link_section = ".boot2"]
#[used]
static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

#[derive(Clone, Copy, Debug)]
enum Command {
  Greet(&'static str),
  Report,
  Stop,
}

impl Element for Command {}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[entry]
fn main() -> ! {
  let heap_start = core::ptr::addr_of_mut!(HEAP) as usize;
  interrupt::free(|_| unsafe {
    ALLOCATOR.init(heap_start, HEAP_SIZE);
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

  let led_pin: LedHandle = Rc::new(RefCell::new(
    pins.gpio25.into_function::<FunctionSioOutput>().into_push_pull_output(),
  ));
  let system_clock_hz = clocks.system_clock.freq().to_Hz();

  let mut system: ActorSystem<Command, LocalMailboxFactory> = ActorSystem::new(LocalMailboxFactory::default());
  let mut root = system.root_context();

  let behavior_led = led_pin.clone();
  let behavior_clock_hz = system_clock_hz;

  let greeter_props = Props::with_behavior(MailboxOptions::default(), move || {
    let setup_led = behavior_led.clone();
    let clock_hz = behavior_clock_hz;
    Behaviors::setup(move |ctx| {
      let ready_blinks = ((ctx.actor_id().0 as u8) % 3 + 1) as usize;
      blink_repeated(&setup_led, ready_blinks, 60, 60, clock_hz);

      let message_led = setup_led.clone();
      let mut greeted = 0usize;
      Behaviors::receive_message(move |msg: Command| match msg {
        Command::Greet(name) => {
          blink_message(&message_led, &BlinkPattern::short_pulse(), clock_hz);
          greeted = greeted.wrapping_add(1);
          // 名前の長さで追加の点滅パターンを作成し、誰に挨拶したかを伝える。
          let name_len = name.len().min(5) as u8;
          if name_len > 0 {
            blink_repeated(&message_led, name_len as usize, 80, 80, clock_hz);
          }
          Behaviors::same()
        }
        Command::Report => {
          let flashes = core::cmp::max(greeted, 1);
          blink_repeated(&message_led, flashes, 200, 120, clock_hz);
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
  greeter.tell(Command::Greet("Alice")).expect("greet Alice");
  greeter.tell(Command::Greet("Bob")).expect("greet Bob");
  greeter.tell(Command::Report).expect("report greetings");
  greeter.tell(Command::Stop).expect("stop greeter");

  system.run_until_idle().expect("drain mailbox");

  set_led_state(&led_pin, false);

  // ループに入って省電力待機。LEDは消灯状態を維持する。
  loop {
    asm::wfi();
  }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct BlinkPattern {
  on_ms: u32,
  off_ms: u32,
  repeat: usize,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl BlinkPattern {
  const fn new(on_ms: u32, off_ms: u32, repeat: usize) -> Self {
    Self { on_ms, off_ms, repeat }
  }

  const fn short_pulse() -> Self {
    Self::new(120, 80, 1)
  }

  const fn shutdown() -> Self {
    Self::new(300, 200, 3)
  }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn blink_message(led: &LedHandle, pattern: &BlinkPattern, sys_hz: u32) {
  blink_repeated(led, pattern.repeat, pattern.on_ms, pattern.off_ms, sys_hz);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn blink_repeated(led: &LedHandle, times: usize, on_ms: u32, off_ms: u32, sys_hz: u32) {
  for _ in 0..times {
    set_led_state(led, true);
    delay_ms(on_ms, sys_hz);
    set_led_state(led, false);
    delay_ms(off_ms, sys_hz);
  }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn set_led_state(led: &LedHandle, high: bool) {
  let mut led_guard = led.borrow_mut();
  if high {
    led_guard.set_high().ok();
  } else {
    led_guard.set_low().ok();
  }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn delay_ms(ms: u32, sys_hz: u32) {
  if ms == 0 {
    return;
  }
  let cycles = (sys_hz / 1_000).saturating_mul(ms.max(1));
  asm::delay(cycles);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
type LedPin = hal::gpio::Pin<Gpio25, FunctionSioOutput, hal::gpio::PullDownDisabled>;

#[cfg(all(target_arch = "arm", target_os = "none"))]
type LedHandle = Rc<RefCell<LedPin>>;

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
  println!(
    "RP2040 Behaviors Greeter exampleはthumbv6m-none-eabiターゲット向けです。\n\
     `cargo run --release --example rp2040_behaviors_greeter --target thumbv6m-none-eabi` でビルドしてください。"
  );
}
