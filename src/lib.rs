//! Sets up the AC108 ADC on a ReSpeaker 4-Mic Array HAT from user space.
//!
//! The AC108 is configured once over I2C and then runs as the I2S bit and frame
//! clock master, streaming 4 TDM channels continuously. The Pi side needs no
//! custom kernel code: the stock `bcm2835-i2s` driver captures the stream as a
//! clock slave (see `overlay/`).

pub mod bus;
pub mod config;
pub mod fake;
#[cfg(target_os = "linux")]
pub mod i2c;
pub mod init;
pub mod frames;
pub mod regs;

pub use bus::{Delay, RegisterBus};
pub use config::Config;
pub use init::configure;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// An I2C transfer failed.
    Io(std::io::Error),
    /// Register 0x00 didn't read back the AC108's chip ID (0x4a).
    WrongChipId { found: u8 },
    /// The PLL didn't report lock in time, so no clocks were started.
    PllNotLocked,
    /// The requested configuration isn't supported; the chip was not touched.
    InvalidConfig(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I2C error: {e}"),
            Error::WrongChipId { found } => {
                write!(f, "not an AC108: chip ID 0x{found:02x}, expected 0x{:02x}", regs::CHIP_ID)
            }
            Error::PllNotLocked => write!(f, "AC108 PLL did not lock; clocks left off"),
            Error::InvalidConfig(why) => write!(f, "invalid configuration: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
