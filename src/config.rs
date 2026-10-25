use crate::Error;

/// The HAT's on-board oscillator feeds the AC108's MCLK pin.
pub const MCLK_HZ: u32 = 24_000_000;
/// The PLL turns MCLK into this system clock (the 48 kHz family).
pub const SYSCLK_HZ: u32 = 24_576_000;
/// One AC108, four mics.
pub const CHANNELS: u32 = 4;
/// Each mic gets a 32-bit TDM slot.
pub const SLOT_BITS: u32 = 32;

/// Settings that can vary. Everything else is fixed by the HAT's wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub sample_rate_hz: u32,
    /// Analog mic preamp gain, 0..=31 dB.
    pub pga_gain_db: u8,
    /// ADC digital volume, raw register value: 0xa0 is 0 dB, 0.75 dB per step.
    pub digital_volume: u8,
}

impl Default for Config {
    /// Matches what Seeed's driver and mixer settings produce.
    fn default() -> Self {
        Config { sample_rate_hz: 16_000, pga_gain_db: 0, digital_volume: 0xde }
    }
}

impl Config {
    /// ADC_SPRC code for the sample rate (rates the 24.576 MHz clock can make).
    pub fn rate_code(&self) -> Result<u8, Error> {
        todo!()
    }

    /// Index into regs::BCLK_DIVIDERS so BCLK = rate * channels * slot bits.
    pub fn bclk_div_code(&self) -> Result<u8, Error> {
        todo!()
    }

    pub fn validate(&self) -> Result<(), Error> {
        todo!()
    }
}
