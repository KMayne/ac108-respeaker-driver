use crate::regs::BCLK_DIVIDERS;
use crate::Error;

/// The HAT's on-board oscillator feeds the AC108's MCLK pin.
pub const MCLK_HZ: u32 = 24_000_000;
/// The PLL turns MCLK into this system clock (the 48 kHz family).
pub const SYSCLK_HZ: u32 = 24_576_000;
/// One AC108, four mics.
pub const CHANNELS: u32 = 4;
/// Each mic gets a 32-bit TDM slot.
pub const SLOT_BITS: u32 = 32;

/// PLL settings for 24 MHz in, 24.576 MHz out:
/// FOUT = FIN * N / ((M1 + 1) * (M2 + 1) * (K1 + 1) * (K2 + 1)) = 24 MHz * 128 / (5 * 25).
/// From the `ac108_pll_div_list` table in the original driver.
pub struct PllDividers {
    pub m1: u8,
    pub m2: u8,
    pub n: u16,
    pub k1: u8,
    pub k2: u8,
}

pub const PLL_24M_TO_24_576M: PllDividers = PllDividers { m1: 4, m2: 0, n: 128, k1: 24, k2: 0 };

/// ADC_SPRC sample-rate codes. Only rates that divide evenly from 24.576 MHz are
/// usable; 11.025/22.05/44.1 kHz would need the 22.5792 MHz PLL setting.
const RATE_CODES: [(u32, u8); 10] = [
    (8_000, 0),
    (11_025, 1),
    (12_000, 2),
    (16_000, 3),
    (22_050, 4),
    (24_000, 5),
    (32_000, 6),
    (44_100, 7),
    (48_000, 8),
    (96_000, 9),
];

/// Settings that can vary. Everything else is fixed by the HAT's wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub sample_rate_hz: u32,
    /// Analog mic preamp gain, 0..=31 dB.
    pub pga_gain_db: u8,
    /// ADC digital volume, raw register value: 0xa0 is 0 dB, 0.75 dB per step.
    pub digital_volume: u8,
    /// Which ADC (0..=3) each of the 4 TDM slots carries.
    pub slot_sources: [u8; 4],
}

impl Default for Config {
    /// Matches what Seeed's driver and mixer settings produce. The driver rotates
    /// the slots (ADC3, ADC4, ADC1, ADC2) to undo a rotation in its CPU-side setup.
    fn default() -> Self {
        Config { sample_rate_hz: 16_000, pga_gain_db: 0, digital_volume: 0xde, slot_sources: [2, 3, 0, 1] }
    }
}

impl Config {
    /// ADC_SPRC code for the sample rate.
    pub fn rate_code(&self) -> Result<u8, Error> {
        RATE_CODES
            .iter()
            .find(|&&(hz, _)| hz == self.sample_rate_hz)
            .map(|&(_, code)| code)
            .ok_or(Error::InvalidConfig("sample rate not supported by the AC108"))
    }

    /// Index into `regs::BCLK_DIVIDERS` so that SYSCLK / divider = BCLK exactly.
    pub fn bclk_div_code(&self) -> Result<u8, Error> {
        let bclk = self.bclk_hz();
        if bclk == 0 || !SYSCLK_HZ.is_multiple_of(bclk) {
            return Err(Error::InvalidConfig("sample rate can't be made from the 24.576 MHz clock"));
        }
        let ratio = SYSCLK_HZ / bclk;
        BCLK_DIVIDERS
            .iter()
            .position(|&d| d == ratio)
            .map(|i| i as u8)
            .ok_or(Error::InvalidConfig("no BCLK divider for this sample rate"))
    }

    /// Check everything before any I2C traffic, so a bad config never half-configures the chip.
    pub fn validate(&self) -> Result<(), Error> {
        self.rate_code()?;
        self.bclk_div_code()?;
        if self.pga_gain_db > 31 {
            return Err(Error::InvalidConfig("PGA gain must be 0..=31 dB"));
        }
        if self.slot_sources.iter().any(|&s| s > 3) {
            return Err(Error::InvalidConfig("slot sources must be ADC 0..=3"));
        }
        Ok(())
    }

    fn bclk_hz(&self) -> u32 {
        self.sample_rate_hz * CHANNELS * SLOT_BITS
    }
}
