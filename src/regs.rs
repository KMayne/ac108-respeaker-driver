//! AC108 register map: only the registers and fields this crate touches.
//!
//! Register names, bit positions and the BCLK divider table come from `ac108.h`
//! and `ac108.c` in Seeed Studio's seeed-voicecard kernel driver (GPL-2.0;
//! authors Baozhu Zuo and panjunwen), as maintained at
//! <https://github.com/HinTak/seeed-voicecard>. See README.md, "Sources".

/// Register 0x00 reads back this chip ID.
pub const CHIP_ID: u8 = 0x4a;
/// Writing this to register 0x00 resets every register to its default.
pub const CHIP_RST_VAL: u8 = 0x12;

pub const CHIP_RST: u8 = 0x00;

// Power
pub const PWR_CTRL6: u8 = 0x06;
pub const PWR_CTRL7: u8 = 0x07;
pub const PWR_CTRL9: u8 = 0x09;

// PLL
pub const PLL_CTRL1: u8 = 0x10;
pub const PLL_CTRL2: u8 = 0x11;
pub const PLL_CTRL3: u8 = 0x12;
pub const PLL_CTRL4: u8 = 0x13;
pub const PLL_CTRL5: u8 = 0x14;
pub const PLL_LOCK_CTRL: u8 = 0x18;

// Clocks and resets
pub const SYSCLK_CTRL: u8 = 0x20;
pub const MOD_CLK_EN: u8 = 0x21;
pub const MOD_RST_CTRL: u8 = 0x22;

// I2S / TDM
pub const I2S_CTRL: u8 = 0x30;
pub const I2S_BCLK_CTRL: u8 = 0x31;
pub const I2S_LRCK_CTRL1: u8 = 0x32;
pub const I2S_LRCK_CTRL2: u8 = 0x33;
pub const I2S_FMT_CTRL1: u8 = 0x34;
pub const I2S_FMT_CTRL2: u8 = 0x35;
pub const I2S_FMT_CTRL3: u8 = 0x36;
pub const I2S_TX1_CTRL1: u8 = 0x38;
pub const I2S_TX1_CTRL2: u8 = 0x39;
pub const I2S_TX1_CTRL3: u8 = 0x3a;
pub const I2S_TX1_CHMP_CTRL1: u8 = 0x3c;
pub const I2S_TX1_CHMP_CTRL2: u8 = 0x3d;
pub const I2S_TX1_CHMP_CTRL3: u8 = 0x3e;
pub const I2S_TX1_CHMP_CTRL4: u8 = 0x3f;

// ADC (digital)
pub const ADC_SPRC: u8 = 0x60;
pub const ADC_DIG_EN: u8 = 0x61;
pub const HPF_EN: u8 = 0x66;
/// Digital volume for ADC1..4 at 0x70..=0x73.
pub const ADC_DVOL_CTRL: [u8; 4] = [0x70, 0x71, 0x72, 0x73];

// ADC (analog)
/// Analog PGA gain for ADC1..4 at 0x90..=0x93.
pub const ANA_PGA_CTRL: [u8; 4] = [0x90, 0x91, 0x92, 0x93];
/// Per-channel analog enables for ADC1..4.
pub const ANA_ADC_CTRL1: [u8; 4] = [0xa0, 0xa7, 0xae, 0xb5];
pub const ANA_ADC4_CTRL6: u8 = 0xba;
pub const ANA_ADC4_CTRL7: u8 = 0xbb;

/// Single-bit field helper.
pub const fn bit(n: u8) -> u8 {
    1 << n
}

// PWR_CTRL6
pub const LDO33ANA_ENABLE: u8 = 0;
// PWR_CTRL7
pub const VREF_SEL: u8 = 3; // 5 bits
pub const VREF_FASTSTART_ENABLE: u8 = 1;
pub const VREF_ENABLE: u8 = 0;
// PWR_CTRL9
pub const VREFP_FASTSTART_ENABLE: u8 = 7;
pub const VREFP_RESCTRL: u8 = 5; // 2 bits
pub const IGEN_TRIM: u8 = 1; // 3 bits
pub const VREFP_ENABLE: u8 = 0;

// PLL_CTRL1
pub const PLL_LOCKED_STATUS: u8 = 2; // read-only, set by the chip
pub const PLL_COM_EN: u8 = 1;
pub const PLL_EN: u8 = 0;
// PLL_CTRL2 / PLL_CTRL5
pub const PLL_PREDIV2: u8 = 5;
pub const PLL_PREDIV1: u8 = 0; // 5 bits
pub const PLL_POSTDIV2: u8 = 5;
pub const PLL_POSTDIV1: u8 = 0; // 5 bits
// PLL_LOCK_CTRL
pub const PLL_LOCK_EN: u8 = 0;

// SYSCLK_CTRL
pub const PLLCLK_EN: u8 = 7;
pub const PLLCLK_SRC: u8 = 4; // 2 bits; 0 = MCLK
pub const SYSCLK_SRC: u8 = 3; // 0 = MCLK, 1 = PLL
pub const SYSCLK_EN: u8 = 0;

// MOD_CLK_EN / MOD_RST_CTRL
pub const MOD_I2S: u8 = 7;
pub const MOD_ADC_DIGITAL: u8 = 4;
pub const MOD_MIC_OFFSET_CALIBRATION: u8 = 1;
pub const MOD_ADC_ANALOG: u8 = 0;

// I2S_CTRL
pub const BCLK_IOEN: u8 = 7;
pub const LRCK_IOEN: u8 = 6;
pub const SDO2_EN: u8 = 5;
pub const SDO1_EN: u8 = 4;
pub const TXEN: u8 = 2;
pub const GEN: u8 = 0;

// I2S_BCLK_CTRL
pub const BCLK_POLARITY: u8 = 4;
pub const BCLKDIV: u8 = 0; // 4 bits, index into BCLK_DIVIDERS

// I2S_LRCK_CTRL1
pub const LRCK_POLARITY: u8 = 4; // 1 = left high, right low (normal)
pub const LRCK_PERIODH: u8 = 0; // 2 bits

// I2S_FMT_CTRL1
pub const ENCD_SEL: u8 = 6;
pub const MODE_SEL: u8 = 4; // 2 bits; 0 = PCM (DSP)
pub const TX2_OFFSET: u8 = 3;
pub const TX1_OFFSET: u8 = 2;
pub const TX_SLOT_HIZ: u8 = 1;
pub const TX_STATE: u8 = 0;

// I2S_FMT_CTRL2
pub const SLOT_WIDTH_SEL: u8 = 4; // 3 bits
pub const SAMPLE_RESOLUTION: u8 = 0; // 3 bits

// ADC_DIG_EN
pub const DG_EN: u8 = 4;

// ANA_ADCx_CTRL1
pub const ADC_DSM_ENABLE: u8 = 2;
pub const ADC_PGA_ENABLE: u8 = 1;
pub const ADC_MICBIAS_EN: u8 = 0;

// ANA_ADCx_PGA
pub const ADC_ANALOG_PGA: u8 = 1; // 5 bits, 1 dB steps, 0..=31

// ANA_ADC4_CTRL6 / CTRL7
pub const DSM_DEMOFF: u8 = 5;

/// I2S_BCLK_CTRL BCLKDIV codes: index = register value, entry = divide ratio.
pub const BCLK_DIVIDERS: [u32; 16] = [0, 1, 2, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 176, 192];
