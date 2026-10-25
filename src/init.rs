use crate::{Config, Delay, Error, RegisterBus};

/// Reset the AC108 and configure it to stream 4 mics over TDM, as clock master.
pub fn configure(bus: &mut impl RegisterBus, delay: &mut impl Delay, cfg: &Config) -> Result<(), Error> {
    let _ = (bus, delay, cfg);
    todo!()
}
