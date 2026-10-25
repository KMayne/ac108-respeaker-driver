//! A fake AC108 for tests: a register file that behaves like the real chip in
//! the ways the setup code depends on, and records every access.

use std::cell::RefCell;
use std::rc::Rc;

use crate::regs::{self, bit};
use crate::{Delay, Error, RegisterBus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Read(u8, u8),
    Write(u8, u8),
    DelayMs(u32),
}

pub struct FakeAc108 {
    regs: [u8; 256],
    reset_values: [u8; 256],
    /// Whether the PLL reports lock once it's enabled.
    pub pll_locks: bool,
    log: Rc<RefCell<Vec<Op>>>,
}

impl FakeAc108 {
    /// A chip in the given power-on state (e.g. from `parse_regs`).
    pub fn new(reset_values: [u8; 256]) -> Self {
        FakeAc108 { regs: reset_values, reset_values, pll_locks: true, log: Rc::default() }
    }

    pub fn registers(&self) -> &[u8; 256] {
        &self.regs
    }

    /// A delay that records into this chip's log, so sleeps show up in order
    /// with the register accesses around them.
    pub fn delay(&self) -> FakeDelay {
        FakeDelay { log: Rc::clone(&self.log) }
    }

    /// Every register access and delay so far, oldest first.
    pub fn log(&self) -> Vec<Op> {
        self.log.borrow().clone()
    }

    pub fn writes(&self) -> Vec<(u8, u8)> {
        self.log
            .borrow()
            .iter()
            .filter_map(|op| match *op {
                Op::Write(r, v) => Some((r, v)),
                _ => None,
            })
            .collect()
    }
}

impl RegisterBus for FakeAc108 {
    fn read(&mut self, reg: u8) -> Result<u8, Error> {
        let v = self.regs[reg as usize];
        self.log.borrow_mut().push(Op::Read(reg, v));
        Ok(v)
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), Error> {
        self.log.borrow_mut().push(Op::Write(reg, value));
        match reg {
            // 0x00 holds the chip ID; writing 0x12 resets, anything else is ignored.
            regs::CHIP_RST => {
                if value == regs::CHIP_RST_VAL {
                    self.regs = self.reset_values;
                }
            }
            // The lock status bit is read-only and follows the PLL.
            regs::PLL_CTRL1 => {
                let lock = bit(regs::PLL_LOCKED_STATUS);
                let running = value & (bit(regs::PLL_EN) | bit(regs::PLL_COM_EN))
                    == bit(regs::PLL_EN) | bit(regs::PLL_COM_EN);
                let status = if running && self.pll_locks { lock } else { 0 };
                self.regs[reg as usize] = (value & !lock) | status;
            }
            _ => self.regs[reg as usize] = value,
        }
        Ok(())
    }
}

pub struct FakeDelay {
    log: Rc<RefCell<Vec<Op>>>,
}

impl Delay for FakeDelay {
    fn delay_ms(&mut self, ms: u32) {
        self.log.borrow_mut().push(Op::DelayMs(ms));
    }
}

/// Parse a register dump: "<addr> <value>" hex pairs, `#` comments.
/// Unlisted registers read as 0.
pub fn parse_regs(text: &str) -> [u8; 256] {
    let mut out = [0u8; 256];
    for line in text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace().map(|s| {
            u8::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex byte")
        });
        let (a, v) = (it.next().expect("address"), it.next().expect("value"));
        out[a as usize] = v;
    }
    out
}
