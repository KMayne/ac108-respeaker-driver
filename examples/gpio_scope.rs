//! A crude logic analyser for the I2S pins: samples GPIO18 (BCLK), GPIO19
//! (LRCK/FS) and GPIO20 (data in) as fast as /dev/gpiomem allows, then reports
//! duty cycles, frame-sync pulses and BCLK edges per frame.
//!
//! Debug tool only. Run on the Pi as root: `gpio_scope [samples]`.

use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::time::Instant;

const GPLEV0: usize = 0x34 / 4; // GPIO level register for pins 0-31
const BCLK: u32 = 18;
const FS: u32 = 19;
const DIN: u32 = 20;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4_000_000);
    let f = OpenOptions::new().read(true).write(true).open("/dev/gpiomem").expect("open /dev/gpiomem");
    // SAFETY: maps the 4 KiB GPIO register block that /dev/gpiomem exposes; only read below.
    let base = unsafe {
        libc::mmap(std::ptr::null_mut(), 4096, libc::PROT_READ, libc::MAP_SHARED, f.as_raw_fd(), 0)
    };
    assert!(base != libc::MAP_FAILED, "mmap failed");
    let lev = unsafe { (base as *const u32).add(GPLEV0) };

    let mut s = vec![0u32; n];
    let t0 = Instant::now();
    for v in s.iter_mut() {
        // SAFETY: lev points into the mapping above.
        *v = unsafe { std::ptr::read_volatile(lev) };
    }
    let secs = t0.elapsed().as_secs_f64();
    let rate = n as f64 / secs;
    println!("{n} samples in {:.1} ms = {:.1} M samples/s ({:.0} ns each)", secs * 1e3, rate / 1e6, 1e9 / rate);

    let bit = |v: u32, p: u32| (v >> p) & 1;
    for (name, p) in [("BCLK  GPIO18", BCLK), ("FS    GPIO19", FS), ("DATA  GPIO20", DIN)] {
        let high = s.iter().filter(|&&v| bit(v, p) == 1).count();
        let edges = s.windows(2).filter(|w| bit(w[0], p) != bit(w[1], p)).count();
        println!(
            "{name}: high {:5.1}%  edges {edges:8}  ~{:.3} MHz toggle rate",
            100.0 * high as f64 / n as f64,
            edges as f64 / 2.0 / secs / 1e6
        );
    }

    // Frame sync pulses: runs of the minority level on FS.
    let fs_high = s.iter().filter(|&&v| bit(v, FS) == 1).count() * 2 > n;
    let active = if fs_high { 0 } else { 1 }; // the short level is the pulse
    let mut starts = Vec::new();
    let mut widths = Vec::new();
    let mut i = 1;
    while i < n {
        if bit(s[i], FS) == active && bit(s[i - 1], FS) != active {
            let st = i;
            while i < n && bit(s[i], FS) == active {
                i += 1;
            }
            starts.push(st);
            widths.push(i - st);
        }
        i += 1;
    }
    println!("FS pulses: {} (active {}), polarity: pulse is {}", starts.len(), active, if active == 1 { "HIGH" } else { "LOW" });
    if starts.len() > 3 {
        let periods: Vec<usize> = starts.windows(2).map(|w| w[1] - w[0]).collect();
        let mean_p = periods.iter().sum::<usize>() as f64 / periods.len() as f64;
        let mean_w = widths.iter().sum::<usize>() as f64 / widths.len() as f64;
        println!(
            "  period {:.1} samples = {:.2} us ({:.0} Hz), width {:.1} samples = {:.2} us",
            mean_p,
            mean_p / rate * 1e6,
            rate / mean_p,
            mean_w,
            mean_w / rate * 1e6
        );
        // Rising BCLK edges between consecutive FS pulse starts = BCLKs per frame.
        let mut counts = Vec::new();
        for w in starts.windows(2).take(200) {
            let c = s[w[0]..w[1]].windows(2).filter(|x| bit(x[0], BCLK) == 0 && bit(x[1], BCLK) == 1).count();
            counts.push(c);
        }
        counts.sort();
        println!(
            "  BCLK rising edges per frame: min {} median {} max {}",
            counts[0],
            counts[counts.len() / 2],
            counts[counts.len() - 1]
        );
    }
}
