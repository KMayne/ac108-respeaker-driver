//! The AC108 setup sequence. It follows what the original kernel driver writes
//! across `set_fmt`, `hw_params`, DAPM power-up and `set_clock`, but runs once,
//! in order, and only turns the clock outputs on after the PLL has locked.

use crate::config::{CHANNELS, PLL_24M_TO_24_576M, SLOT_BITS};
use crate::regs::*;
use crate::{Config, Delay, Error, RegisterBus};

/// How long to wait for the PLL, polling every `PLL_POLL_MS`.
const PLL_LOCK_TIMEOUT_MS: u32 = 200;
const PLL_POLL_MS: u32 = 5;

/// Reset the AC108 and configure it to stream 4 mics over TDM, as clock master.
///
/// Nothing is written unless the config is valid and the chip ID matches.
pub fn configure(bus: &mut impl RegisterBus, delay: &mut impl Delay, cfg: &Config) -> Result<(), Error> {
    cfg.validate()?;

    let id = bus.read(CHIP_RST)?;
    if id != CHIP_ID {
        return Err(Error::WrongChipId { found: id });
    }

    // Every register back to its default, so the result doesn't depend on history.
    // The chip resets before acknowledging this write, so on real hardware it
    // fails with a NACK even though the reset happens. Ignore that, then make
    // sure the chip answers again.
    let _ = bus.write(CHIP_RST, CHIP_RST_VAL);
    delay.delay_ms(2);
    let id = bus.read(CHIP_RST)?;
    if id != CHIP_ID {
        return Err(Error::WrongChipId { found: id });
    }

    // Keep BCLK/LRCK pins as inputs (they default to outputs) and the transmitter
    // off until the clock tree is running. Both data outputs on.
    bus.write(I2S_CTRL, bit(SDO2_EN) | bit(SDO1_EN))?;

    power_up_references(bus)?;
    configure_pll(bus)?;
    configure_tdm(bus, cfg)?;
    configure_adcs(bus, cfg)?;

    // Clocks and resets for the I2S block, digital ADC, mic offset calibration
    // and analog ADC.
    let modules = bit(MOD_I2S) | bit(MOD_ADC_DIGITAL) | bit(MOD_MIC_OFFSET_CALIBRATION) | bit(MOD_ADC_ANALOG);
    bus.write(MOD_CLK_EN, modules)?;
    bus.write(MOD_RST_CTRL, modules)?; // 1 = out of reset

    // Start the PLL and wait for lock before driving any clocks onto the bus.
    let pll_on = bit(PLL_EN) | bit(PLL_COM_EN);
    bus.update_bits(PLL_CTRL1, pll_on, pll_on)?;
    wait_for_pll_lock(bus, delay)?;

    // Drive BCLK and LRCK (clock master), enable the transmitter and the block.
    bus.write(
        I2S_CTRL,
        bit(BCLK_IOEN) | bit(LRCK_IOEN) | bit(SDO2_EN) | bit(SDO1_EN) | bit(TXEN) | bit(GEN),
    )
}

/// Analog LDO, VREF and VREFP: needed by the mic bias and every ADC channel.
fn power_up_references(bus: &mut impl RegisterBus) -> Result<(), Error> {
    bus.update_bits(PWR_CTRL6, bit(LDO33ANA_ENABLE), bit(LDO33ANA_ENABLE))?;
    // VREF level 0x13, no fast start.
    bus.update_bits(
        PWR_CTRL7,
        0x1f << VREF_SEL | bit(VREF_FASTSTART_ENABLE) | bit(VREF_ENABLE),
        0x13 << VREF_SEL | bit(VREF_ENABLE),
    )?;
    // VREFP: no fast start, 1 MOhm, bias current trim +25%.
    bus.update_bits(
        PWR_CTRL9,
        bit(VREFP_FASTSTART_ENABLE) | 0x03 << VREFP_RESCTRL | 0x07 << IGEN_TRIM | bit(VREFP_ENABLE),
        0x04 << IGEN_TRIM | bit(VREFP_ENABLE),
    )
}

/// PLL from MCLK (24 MHz) to 24.576 MHz, used as the system clock. The PLL
/// itself is switched on later, once everything else is set.
fn configure_pll(bus: &mut impl RegisterBus) -> Result<(), Error> {
    let p = PLL_24M_TO_24_576M;
    bus.update_bits(PLL_CTRL5, 0x1f << PLL_POSTDIV1 | bit(PLL_POSTDIV2), p.k1 << PLL_POSTDIV1 | p.k2 << PLL_POSTDIV2)?;
    bus.write(PLL_CTRL4, (p.n & 0xff) as u8)?;
    bus.update_bits(PLL_CTRL3, 0x03, (p.n >> 8) as u8 & 0x03)?;
    bus.update_bits(PLL_CTRL2, 0x1f << PLL_PREDIV1 | bit(PLL_PREDIV2), p.m1 << PLL_PREDIV1 | p.m2 << PLL_PREDIV2)?;
    bus.update_bits(PLL_LOCK_CTRL, bit(PLL_LOCK_EN), bit(PLL_LOCK_EN))?;
    // PLL clock on, PLL fed from MCLK, system clock from the PLL, system clock on.
    bus.update_bits(
        SYSCLK_CTRL,
        bit(PLLCLK_EN) | 0x03 << PLLCLK_SRC | bit(SYSCLK_SRC) | bit(SYSCLK_EN),
        bit(PLLCLK_EN) | bit(SYSCLK_SRC) | bit(SYSCLK_EN),
    )
}

/// DSP mode A (`dsp_a`) TDM: 4 slots of 32 bits, data one BCLK after the LRCK
/// pulse, normal clock polarities.
fn configure_tdm(bus: &mut impl RegisterBus, cfg: &Config) -> Result<(), Error> {
    bus.update_bits(I2S_BCLK_CTRL, bit(BCLK_POLARITY) | 0x0f << BCLKDIV, cfg.bclk_div_code()? << BCLKDIV)?;

    // LRCK period in BCLKs, minus one: the whole frame in PCM mode.
    let period = (CHANNELS * SLOT_BITS - 1) as u16;
    bus.update_bits(
        I2S_LRCK_CTRL1,
        bit(LRCK_POLARITY) | 0x03 << LRCK_PERIODH,
        bit(LRCK_POLARITY) | ((period >> 8) as u8 & 0x03) << LRCK_PERIODH,
    )?;
    bus.write(I2S_LRCK_CTRL2, (period & 0xff) as u8)?;

    // PCM mode, no channel-number encoding, 1-BCLK data offset, drive (not
    // hi-z) during unused slot time, transmitter state normal.
    bus.write(I2S_FMT_CTRL1, 0 << ENCD_SEL | 0 << MODE_SEL | bit(TX2_OFFSET) | bit(TX1_OFFSET) | bit(TX_STATE))?;
    // 32-bit samples in 32-bit slots (code 7 = 32 bits).
    bus.write(I2S_FMT_CTRL2, 7 << SLOT_WIDTH_SEL | 7 << SAMPLE_RESOLUTION)?;
    // MSB first, sign extension in unused bits, LRCK 1 BCLK wide, linear PCM.
    bus.update_bits(I2S_FMT_CTRL3, 0x80 | 0x03 << 5 | 0x04 | 0x03, 0x03 << 5)?;

    // Slots 1-4 on, carrying the configured ADCs.
    bus.write(I2S_TX1_CTRL1, (CHANNELS - 1) as u8)?;
    bus.write(I2S_TX1_CTRL2, 0x0f)?;
    bus.write(I2S_TX1_CTRL3, 0x00)?;
    let [a, b, c, d] = cfg.slot_sources;
    bus.write(I2S_TX1_CHMP_CTRL1, a | b << 2 | c << 4 | d << 6)?;
    bus.write(I2S_TX1_CHMP_CTRL2, 0x00)?;
    bus.write(I2S_TX1_CHMP_CTRL3, 0x00)?;
    bus.write(I2S_TX1_CHMP_CTRL4, 0x00)
}

/// Sample rate, high-pass filters, gains, and the analog and digital ADC enables.
fn configure_adcs(bus: &mut impl RegisterBus, cfg: &Config) -> Result<(), Error> {
    bus.update_bits(ADC_SPRC, 0x0f, cfg.rate_code()?)?;
    bus.write(ADC_DIG_EN, bit(DG_EN) | 0x0f)?; // digital part + ADC1..4
    bus.write(HPF_EN, 0x0f)?; // DC-blocking high-pass on ADC1..4

    for reg in ADC_DVOL_CTRL {
        bus.write(reg, cfg.digital_volume)?;
    }
    for reg in ANA_PGA_CTRL {
        bus.write(reg, cfg.pga_gain_db << ADC_ANALOG_PGA)?;
    }
    for reg in ANA_ADC_CTRL1 {
        bus.write(reg, bit(ADC_DSM_ENABLE) | bit(ADC_PGA_ENABLE) | bit(ADC_MICBIAS_EN))?;
    }
    // As left by the original driver's DAPM power-up.
    bus.write(ANA_ADC4_CTRL6, bit(DSM_DEMOFF))?;
    bus.write(ANA_ADC4_CTRL7, 0x0f)
}

fn wait_for_pll_lock(bus: &mut impl RegisterBus, delay: &mut impl Delay) -> Result<(), Error> {
    let mut waited = 0;
    loop {
        if bus.read(PLL_CTRL1)? & bit(PLL_LOCKED_STATUS) != 0 {
            return Ok(());
        }
        if waited >= PLL_LOCK_TIMEOUT_MS {
            return Err(Error::PllNotLocked);
        }
        delay.delay_ms(PLL_POLL_MS);
        waited += PLL_POLL_MS;
    }
}
