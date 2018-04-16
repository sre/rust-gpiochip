Rust gpiochip
=============

The Rust `gpiochip` seeks to provide full access to the Linux gpiochip
device in Rust without the need to wrap any C code or directly make
low-level system calls.

Examples
========

```rust
extern crate gpiochip as gpio;

/// Print information about first gpiochip
fn main() {
    let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();

    println!("GPIOChip 0:");
    println!(" Name:  {:?}", chip.name);
    println!(" Label: {:?}", chip.label);
    println!(" Lines: {:?}", chip.lines);
    println!("");

    for i in 0..chip.lines {
        let info = chip.info(i).unwrap();

        println!(" GPIO {:?}: {:?}", info.gpio, info.name);
        println!("     Consumer: {:?}", info.consumer);
        println!("     Flags: {:?}", info.flags);
    }
}
```

```rust
extern crate gpiochip as gpio;

/// Simple get/set example
fn main() {
    let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();
    let gpio_a = chip.request("gpioA", gpio::RequestFlags::INPUT, 0, 0).unwrap();
    let gpio_b = chip.request("gpioA", gpio::RequestFlags::OUTPUT | gpio::RequestFlags::ACTIVE_LOW, 1, 0).unwrap();

    let val = gpio_a.get().unwrap();
    gpio_b.set(val).unwrap();
}
```

```rust
extern crate gpiochip as gpio;

/// GPIO events
fn main() {
    let chip = gpio::GpioChip::new("/dev/gpiochip0").unwrap();

    let gpio_a = chip.request_event("gpioA", 0, gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES).unwrap();
    let gpio_b = chip.request_event("gpioB", 1, gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES).unwrap();

    let bitmap = gpio::wait_for_event(&[&gpio_a, &gpio_b], 1000).unwrap();

    if bitmap & 0b01 == 0b01 {
        let event = gpio_a.read().unwrap();
        println!("gpioA: event @ {:?} - {:?}", event.timestamp, event.id);
    }

    if bitmap & 0b10 == 0b10 {
        let event = gpio_b.read().unwrap();
        println!("gpioB: event @ {:?} - {:?}", event.timestamp, event.id);
    }
}
```

License
=======

© 2018 Sebastian Reichel

ISC License

Permission to use, copy, modify, and/or distribute this software for
any purpose with or without fee is hereby granted, provided that the
above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
