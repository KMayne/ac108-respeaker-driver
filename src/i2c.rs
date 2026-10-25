//! Linux `/dev/i2c-N` access, through the i2c-dev ioctls.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;

use crate::{Error, RegisterBus};

// From <linux/i2c-dev.h> and <linux/i2c.h>. ioctl request types differ
// between glibc (c_ulong) and musl (c_int), hence the `as _` at each call.
const I2C_SLAVE: libc::c_ulong = 0x0703;
const I2C_RDWR: libc::c_ulong = 0x0707;
const I2C_M_RD: u16 = 0x0001;

#[repr(C)]
struct I2cMsg {
    addr: u16,
    flags: u16,
    len: u16,
    buf: *mut u8,
}

#[repr(C)]
struct I2cRdwrData {
    msgs: *mut I2cMsg,
    nmsgs: u32,
}

/// One device on a Linux I2C bus.
pub struct LinuxI2c {
    file: File,
    addr: u16,
}

impl LinuxI2c {
    /// Open `/dev/i2c-<bus>` for the device at `addr`.
    ///
    /// Fails with "Device or resource busy" if a kernel driver already owns the
    /// address, e.g. the old seeed-voicecard driver: two drivers must not fight
    /// over the chip.
    pub fn open(bus: u8, addr: u16) -> Result<Self, Error> {
        let path = format!("/dev/i2c-{bus}");
        let file = OpenOptions::new().read(true).write(true).open(&path).map_err(|e| {
            let hint = match e.kind() {
                io::ErrorKind::NotFound => " (is the i2c-dev module loaded?)",
                io::ErrorKind::PermissionDenied => " (run as root)",
                _ => "",
            };
            Error::Io(io::Error::new(e.kind(), format!("{path}: {e}{hint}")))
        })?;
        // SAFETY: plain ioctl on a file we own, taking an integer argument.
        if unsafe { libc::ioctl(file.as_raw_fd(), I2C_SLAVE as _, libc::c_ulong::from(addr)) } < 0 {
            let e = io::Error::last_os_error();
            return Err(Error::Io(io::Error::new(e.kind(), format!("{path} address 0x{addr:02x}: {e}"))));
        }
        Ok(LinuxI2c { file, addr })
    }

    fn transfer(&mut self, msgs: &mut [I2cMsg]) -> Result<(), Error> {
        let mut data = I2cRdwrData { msgs: msgs.as_mut_ptr(), nmsgs: msgs.len() as u32 };
        // SAFETY: `data` points at `msgs`, whose buffers outlive this call.
        if unsafe { libc::ioctl(self.file.as_raw_fd(), I2C_RDWR as _, &mut data) } < 0 {
            return Err(Error::Io(io::Error::last_os_error()));
        }
        Ok(())
    }
}

impl RegisterBus for LinuxI2c {
    /// Register address write, then a repeated-start read of one byte.
    fn read(&mut self, reg: u8) -> Result<u8, Error> {
        let mut reg_buf = [reg];
        let mut val = [0u8];
        let mut msgs = [
            I2cMsg { addr: self.addr, flags: 0, len: 1, buf: reg_buf.as_mut_ptr() },
            I2cMsg { addr: self.addr, flags: I2C_M_RD, len: 1, buf: val.as_mut_ptr() },
        ];
        self.transfer(&mut msgs)?;
        Ok(val[0])
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), Error> {
        let mut buf = [reg, value];
        let mut msgs = [I2cMsg { addr: self.addr, flags: 0, len: 2, buf: buf.as_mut_ptr() }];
        self.transfer(&mut msgs)
    }
}
