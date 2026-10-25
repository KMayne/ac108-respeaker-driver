# ac108-respeaker-driver

User-space setup for the AC108 ADC on the ReSpeaker 4-Mic Array HAT for Raspberry Pi.

The usual way to use this HAT is Seeed's out-of-tree kernel driver. That driver
starts the AC108's clocks over I2C from inside the PCM trigger, which runs in
atomic context. On current kernels that gives `BUG: scheduling while atomic`,
`I2S SYNC error` and misaligned, unusable audio, and it has to be rebuilt with
DKMS for every kernel.

This project splits the job in two:

- **`ac108ctl`** (this crate, Rust, user space) configures the AC108 once over
  `/dev/i2c-1`: power, PLL (24 MHz to 24.576 MHz), 4-slot TDM, ADCs, gains. The
  chip then runs as I2S clock master and streams all 4 mics continuously.
- **A device tree overlay** (`overlay/`, planned) connects the Pi's stock
  `bcm2835-i2s` driver, as clock slave, to the stock `dmic-codec` placeholder.
  No custom kernel code at all.

Because the clocks are already running before any capture starts, the
start-up timing problem that breaks the kernel driver can't happen.

## Status

Work in progress. Fixed to what the HAT needs: one AC108, 4 channels, 32-bit
slots, 24 MHz MCLK. Sample rates from the 24.576 MHz family (8, 12, 16, 24, 32,
48 kHz).

## Building

Tests run on any machine against a fake AC108:

```sh
cargo test
```

Cross-compile a static binary for the Pi (64-bit Raspberry Pi OS), using the
`rust-lld` linker that ships with Rust, so no cross C toolchain is needed:

```sh
rustup target add aarch64-unknown-linux-musl
cargo pi        # alias in .cargo/config.toml
scp target/aarch64-unknown-linux-musl/release/ac108ctl pi:
```

## How the tests work

`tests/fixtures/` holds two register dumps taken from a real HAT on a
Raspberry Pi 4 through the original driver's debug interface:

- `ac108-reset.regs`: every register right after a software reset.
- `ac108-capture-16k-4ch.regs`: every register while capturing 16 kHz, 4
  channels, 32-bit slots.

`src/fake.rs` is a fake AC108 that starts from the reset dump and imitates the
behaviour the setup code relies on (reset, read-only chip ID, PLL lock status).
The main test checks that running the setup on it ends in exactly the
capture-state dump; other tests check the order of operations and error paths.

## Sources

This is a clean user-space rewrite, but it relies on other people's work:

- **Seeed Studio's seeed-voicecard driver**, the original kernel driver for the
  ReSpeaker HATs: <https://github.com/respeaker/seeed-voicecard>. `ac108.c` by
  Baozhu Zuo, `ac108.h` by panjunwen, `seeed-voicecard.c` by Peter Yang, all
  GPL-2.0. The register names and bit positions in `src/regs.rs`, the PLL
  divider values and the BCLK divider table come from these files, and the
  setup sequence follows what the driver does across `set_fmt`, `hw_params`,
  DAPM and `set_clock`.
- **Hin-Tak Leung's maintained fork**, which carries the driver forward to new
  kernels: <https://github.com/HinTak/seeed-voicecard>. The register reference
  was read from branch `v6.12`, commit `f04dab2`, and the fixtures were captured
  with that build.
- **The fix in PR #45 by avisual**,
  <https://github.com/HinTak/seeed-voicecard/pull/45>, and the report in issue
  #22, <https://github.com/HinTak/seeed-voicecard/issues/22>. These pinned down
  the sleeping-I2C-in-atomic-trigger bug, which is the reason for this design.
- **The Raspberry Pi kernel's stock audio drivers** (`bcm2835-i2s`,
  `simple-audio-card`, `dmic-codec`), which do the actual audio transport.

## License

GPL-2.0-only, matching the driver the register definitions come from. See
`LICENSE`.
