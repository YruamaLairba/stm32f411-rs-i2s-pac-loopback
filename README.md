# stm32f411-rs-i2s-pac-loopback

This rust project is an example of i2s usage on a stm32f411 chip. It use to i2s
peripherals do to a communication between a master transmitter and a slave
receiver. I mainly did this because i was in trouble to do communication with
an external chip and i wanted to understand what was wrong. Noticeably, the
code resynchronize the slave when a desynchronization occurred.

At time of this project, there is no Hardware Abstraction Layer for i2s.
Instead, the main source directly access to the hardware using `pac` module,
this is why it contains many unsafe sections.

This project is for a nucleo-f411re but should be easily adaptable for any
stm32f4xx using a 8MHz external oscillator. To Adapt it, you need:
 - Change ram an flash in `memory.x` according to your device.
 - In `Cargo.toml`, under `dependencies.stm32f4xx-hal` replace the `stm32f411`
   feature according to your chip (See
[here](https://crates.io/crates/stm32f4xx-hal) for available chips).
 - In `Embed.toml`, under `default.general`, replace `chip` value by your chip
   (`cargo embed --list-chip` to get the list of possible value).

## Connecting devices

| i2s master | i2s slave  | _function_     |
|------------|------------|----------------|
| PB13       | PB0        | _Serial Clock_ |
| PB15       | PB8        | _Serial Data_  |
| PB12       | PB1        | _Word Select_  |
| PC6        | -          | _Master Clock_ |


## Build, load, and run

This require `cargo embed`, you can install it with `cargo install cargo-embed`.
Just run `cargo embed` to build, load, and run the firmware.

## License

This project is licensed under terms of both MIT and Apache licenses. See
[LICENSE-APACHE.txt](LICENSE-APACHE.txt) and
[LICENSE-MIT.txt](LICENSE-MIT.txt).
