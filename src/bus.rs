use crate::Error;

/// 8-bit register access to one AC108.
pub trait RegisterBus {
    fn read(&mut self, reg: u8) -> Result<u8, Error>;
    fn write(&mut self, reg: u8, value: u8) -> Result<(), Error>;

    /// Read-modify-write: replace the bits in `mask` with those from `value`.
    fn update_bits(&mut self, reg: u8, mask: u8, value: u8) -> Result<(), Error> {
        let old = self.read(reg)?;
        self.write(reg, (old & !mask) | (value & mask))
    }
}

/// Sleeping, behind a trait so tests run instantly.
pub trait Delay {
    fn delay_ms(&mut self, ms: u32);
}

/// Real sleeps, for use on the Pi.
pub struct StdDelay;

impl Delay for StdDelay {
    fn delay_ms(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms.into()));
    }
}
