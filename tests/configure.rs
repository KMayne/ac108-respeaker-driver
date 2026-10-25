use ac108::fake::{parse_regs, FakeAc108, Op};
use ac108::regs::{self, bit};
use ac108::{configure, Config, Error};

const RESET: &str = include_str!("fixtures/ac108-reset.regs");
const CAPTURE_16K_4CH: &str = include_str!("fixtures/ac108-capture-16k-4ch.regs");

fn chip() -> FakeAc108 {
    FakeAc108::new(parse_regs(RESET))
}

/// Run `configure` on a fake chip, with its sleeps recorded in the chip's log.
fn run(chip: &mut FakeAc108, cfg: &Config) -> Result<(), Error> {
    let mut delay = chip.delay();
    configure(chip, &mut delay, cfg)
}

fn index_of(log: &[Op], pred: impl Fn(&Op) -> bool) -> Option<usize> {
    log.iter().position(pred)
}

fn last_index_of(log: &[Op], pred: impl Fn(&Op) -> bool) -> Option<usize> {
    log.iter().rposition(pred)
}

#[test]
fn default_config_reproduces_the_reference_capture_from_reset() {
    let mut c = chip();
    run(&mut c, &Config::default()).expect("configure");

    let want = parse_regs(CAPTURE_16K_4CH);
    let got = c.registers();
    let diffs: Vec<String> = (0..=0xfe)
        .filter(|&r| got[r] != want[r])
        .map(|r| format!("0x{r:02x}: got 0x{:02x}, want 0x{:02x}", got[r], want[r]))
        .collect();
    assert!(diffs.is_empty(), "registers differ from the reference capture:\n{}", diffs.join("\n"));
}

#[test]
fn also_works_from_a_dirty_state_because_it_resets_first() {
    let mut c = chip();
    run(&mut c, &Config::default()).unwrap();
    run(&mut c, &Config { pga_gain_db: 20, ..Config::default() }).unwrap();
    run(&mut c, &Config::default()).unwrap();
    assert_eq!(c.registers(), &parse_regs(CAPTURE_16K_4CH));
}

#[test]
fn checks_the_chip_id_before_writing_anything() {
    let mut not_ac108 = parse_regs(RESET);
    not_ac108[regs::CHIP_RST as usize] = 0x00;
    let mut c = FakeAc108::new(not_ac108);

    let err = run(&mut c, &Config::default()).unwrap_err();
    assert!(matches!(err, Error::WrongChipId { found: 0x00 }), "{err:?}");
    assert_eq!(c.writes().len(), 0, "wrote to a chip that isn't an AC108");
}

#[test]
fn resets_the_chip_with_its_first_write_then_waits() {
    let mut c = chip();
    run(&mut c, &Config::default()).unwrap();

    assert_eq!(c.writes().first().copied(), Some((regs::CHIP_RST, regs::CHIP_RST_VAL)));
    let reset = index_of(&c.log(), |op| *op == Op::Write(regs::CHIP_RST, regs::CHIP_RST_VAL)).unwrap();
    assert!(matches!(c.log().get(reset + 1), Some(Op::DelayMs(ms)) if *ms >= 1), "no delay after reset");
}

#[test]
fn expects_the_reset_command_to_go_unacknowledged_and_rechecks_the_chip_id() {
    // The real AC108 resets before ACKing the reset command (seen on hardware).
    let mut c = chip();
    assert!(c.nacks_reset);
    run(&mut c, &Config::default()).expect("a NACKed reset is normal");

    let log = c.log();
    let reset = index_of(&log, |op| *op == Op::Write(regs::CHIP_RST, regs::CHIP_RST_VAL)).unwrap();
    let next_write = log[reset + 1..].iter().position(|op| matches!(op, Op::Write(..))).unwrap() + reset + 1;
    let id_check = log[reset + 1..next_write].contains(&Op::Read(regs::CHIP_RST, regs::CHIP_ID));
    assert!(id_check, "didn't confirm the chip answers again after reset");
}

#[test]
fn also_works_with_a_chip_that_acknowledges_its_reset() {
    let mut c = chip();
    c.nacks_reset = false;
    run(&mut c, &Config::default()).unwrap();
    assert_eq!(c.registers(), &parse_regs(CAPTURE_16K_4CH));
}

#[test]
fn pll_is_enabled_only_after_its_dividers_are_set() {
    let mut c = chip();
    run(&mut c, &Config::default()).unwrap();

    let enable = index_of(&c.log(), |op| {
        matches!(*op, Op::Write(regs::PLL_CTRL1, v) if v & bit(regs::PLL_EN) != 0)
    })
    .expect("PLL never enabled");
    for div in [regs::PLL_CTRL2, regs::PLL_CTRL3, regs::PLL_CTRL4, regs::PLL_CTRL5] {
        let set = last_index_of(&c.log(), |op| matches!(*op, Op::Write(r, _) if r == div))
            .unwrap_or_else(|| panic!("PLL divider 0x{div:02x} never written"));
        assert!(set < enable, "PLL divider 0x{div:02x} written after the PLL was enabled");
    }
}

#[test]
fn starts_the_i2s_clocks_last_and_only_after_the_pll_reports_lock() {
    let mut c = chip();
    run(&mut c, &Config::default()).unwrap();

    let clocks_out = bit(regs::BCLK_IOEN) | bit(regs::LRCK_IOEN);
    let start = index_of(&c.log(), |op| {
        matches!(*op, Op::Write(regs::I2S_CTRL, v) if v & clocks_out == clocks_out)
    })
    .expect("I2S clocks never started");
    let locked = index_of(&c.log(), |op| {
        matches!(*op, Op::Read(regs::PLL_CTRL1, v) if v & bit(regs::PLL_LOCKED_STATUS) != 0)
    })
    .expect("never saw the PLL lock");

    assert!(locked < start, "started I2S clocks before the PLL locked");
    assert_eq!(c.writes().last().map(|&(r, _)| r), Some(regs::I2S_CTRL), "I2S start wasn't the last write");
}

#[test]
fn gives_up_without_starting_clocks_if_the_pll_never_locks() {
    let mut c = chip();
    c.pll_locks = false;

    let err = run(&mut c, &Config::default()).unwrap_err();
    assert!(matches!(err, Error::PllNotLocked), "{err:?}");
    let i2s = c.registers()[regs::I2S_CTRL as usize];
    assert_eq!(i2s & (bit(regs::BCLK_IOEN) | bit(regs::LRCK_IOEN)), 0, "clock outputs enabled anyway");
    let waited: u32 = c.log().iter().map(|op| if let Op::DelayMs(ms) = op { *ms } else { 0 }).sum();
    assert!((50..=1000).contains(&waited), "waited {waited} ms for lock, expected a bounded wait");
}

#[test]
fn pga_gain_applies_to_all_four_mics() {
    let mut c = chip();
    run(&mut c, &Config { pga_gain_db: 20, ..Config::default() }).unwrap();
    for r in regs::ANA_PGA_CTRL {
        assert_eq!(c.registers()[r as usize], 20 << regs::ADC_ANALOG_PGA, "PGA reg 0x{r:02x}");
    }
}

#[test]
fn digital_volume_applies_to_all_four_mics() {
    let mut c = chip();
    run(&mut c, &Config { digital_volume: 0xa0, ..Config::default() }).unwrap();
    for r in regs::ADC_DVOL_CTRL {
        assert_eq!(c.registers()[r as usize], 0xa0, "volume reg 0x{r:02x}");
    }
}

#[test]
fn rejects_pga_gain_above_31_db_without_touching_the_chip() {
    let mut c = chip();
    let err = run(&mut c, &Config { pga_gain_db: 32, ..Config::default() }).unwrap_err();
    assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
    assert!(c.log().is_empty(), "touched the chip: {:?}", c.log());
}

#[test]
fn rejects_rates_the_24_576_mhz_clock_cannot_make() {
    for hz in [44_100, 22_050, 11_025, 15_000, 0] {
        let mut c = chip();
        let err = run(&mut c, &Config { sample_rate_hz: hz, ..Config::default() }).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{hz} Hz: {err:?}");
        assert!(c.log().is_empty(), "{hz} Hz: touched the chip");
    }
}

#[test]
fn rate_sets_the_adc_rate_code_and_bit_clock_divider() {
    // BCLK = rate * 4 channels * 32 bits; divider = 24.576 MHz / BCLK.
    // (rate, ADC_SPRC code, BCLKDIV code for that divider)
    for (hz, rate_code, div_code) in [
        (8_000, 0, 8),   // /24
        (12_000, 2, 7),  // /16 -- rate code 1 is 11.025 kHz
        (16_000, 3, 6),  // /12
        (24_000, 5, 5),  // /8
        (32_000, 6, 4),  // /6
        (48_000, 8, 3),  // /4
    ] {
        let mut c = chip();
        run(&mut c, &Config { sample_rate_hz: hz, ..Config::default() }).unwrap();
        let r = c.registers();
        assert_eq!(r[regs::ADC_SPRC as usize] & 0x0f, rate_code, "{hz} Hz rate code");
        assert_eq!(r[regs::I2S_BCLK_CTRL as usize] & 0x0f, div_code, "{hz} Hz BCLK divider");
    }
}
