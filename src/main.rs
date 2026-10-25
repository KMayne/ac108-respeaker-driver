use std::process::ExitCode;

use ac108::bus::StdDelay;
use ac108::i2c::LinuxI2c;
use ac108::regs::{self, bit};
use ac108::{configure, Config, Error, RegisterBus};

const USAGE: &str = "\
Usage: ac108ctl <command> [options]

Commands:
  init     Reset the AC108 and start streaming 4 mics over TDM
  status   Show chip ID, PLL lock and whether I2S is running
  dump     Print every register as \"0xNN 0xVV\" lines

Options:
  --bus N                I2C bus number (default 1, i.e. /dev/i2c-1)
  --addr 0xNN            AC108 I2C address (default 0x3b)
  --rate HZ              init: sample rate (default 16000)
  --pga-gain DB          init: analog mic gain, 0-31 dB (default 0)
  --digital-volume 0xNN  init: raw digital volume, 0xa0 = 0 dB (default 0xde)
  --slot-sources A,B,C,D init: ADC (0-3) carried by TDM slots 1-4 (default 2,3,0,1)
";

struct Args {
    command: String,
    bus: u8,
    addr: u16,
    cfg: Config,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().ok_or("missing command")?;
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        return Err(String::new());
    }
    if !matches!(command.as_str(), "init" | "status" | "dump") {
        return Err(format!("unknown command {command}"));
    }
    let mut args = Args { command, bus: 1, addr: 0x3b, cfg: Config::default() };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--bus" => args.bus = parse_num(&value()?)?,
            "--addr" => args.addr = parse_num(&value()?)?,
            "--rate" => args.cfg.sample_rate_hz = parse_num(&value()?)?,
            "--pga-gain" => args.cfg.pga_gain_db = parse_num(&value()?)?,
            "--digital-volume" => args.cfg.digital_volume = parse_num(&value()?)?,
            "--slot-sources" => {
                let v = value()?;
                let parts: Vec<u8> = v.split(',').map(parse_num).collect::<Result<_, _>>()?;
                args.cfg.slot_sources =
                    parts.try_into().map_err(|_| format!("--slot-sources needs 4 values, got {v}"))?;
            }
            "-h" | "--help" => return Err(String::new()),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(args)
}

/// Decimal, or hex with a 0x prefix.
fn parse_num<T: TryFrom<u64>>(s: &str) -> Result<T, String> {
    let n = match s.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => s.parse(),
    }
    .map_err(|_| format!("not a number: {s}"))?;
    T::try_from(n).map_err(|_| format!("out of range: {s}"))
}

fn run(args: &Args) -> Result<(), Error> {
    let mut chip = LinuxI2c::open(args.bus, args.addr)?;
    match args.command.as_str() {
        "init" => {
            configure(&mut chip, &mut StdDelay, &args.cfg)?;
            println!(
                "AC108 streaming: {} Hz, 4 x 32-bit TDM slots, PGA {} dB, digital volume 0x{:02x}",
                args.cfg.sample_rate_hz, args.cfg.pga_gain_db, args.cfg.digital_volume
            );
        }
        "status" => {
            let id = chip.read(regs::CHIP_RST)?;
            let pll = chip.read(regs::PLL_CTRL1)?;
            let i2s = chip.read(regs::I2S_CTRL)?;
            let clocks = bit(regs::BCLK_IOEN) | bit(regs::LRCK_IOEN);
            println!("chip id:    0x{id:02x}{}", if id == regs::CHIP_ID { " (AC108)" } else { " (not an AC108!)" });
            println!("pll locked: {}", pll & bit(regs::PLL_LOCKED_STATUS) != 0);
            println!("clocks out: {}", i2s & clocks == clocks);
            println!("tx enabled: {}", i2s & (bit(regs::TXEN) | bit(regs::GEN)) == bit(regs::TXEN) | bit(regs::GEN));
        }
        "dump" => {
            for reg in 0x00..=0xfe {
                println!("0x{reg:02x} 0x{:02x}", chip.read(reg)?);
            }
        }
        _ => unreachable!("commands are checked in parse_args"),
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("{msg}\n");
            }
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ac108ctl: {e}");
            ExitCode::FAILURE
        }
    }
}
