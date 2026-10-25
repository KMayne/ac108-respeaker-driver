//! Turns captured ALSA frames back into 4 mic samples.
//!
//! With 16-bit slots the AC108 sends one 64-bit TDM frame per sample period:
//! slot 1, 2, 3, 4, MSB first. The Pi captures that as two 32-bit ALSA
//! channels, so slots 1 and 2 share the first sample and slots 3 and 4 the
//! second, each with the earlier slot in the upper 16 bits.

/// One ALSA frame (two 32-bit samples) into the 4 slots, in order.
pub fn split_frame([a, b]: [i32; 2]) -> [i16; 4] {
    [(a >> 16) as i16, a as i16, (b >> 16) as i16, b as i16]
}

/// Split interleaved 2-channel ALSA samples; `emit` gets each 4-mic frame.
pub fn split(samples: &[i32], mut emit: impl FnMut([i16; 4])) {
    assert!(samples.len().is_multiple_of(2), "split takes whole 2-channel frames");
    for &frame in samples.as_chunks::<2>().0 {
        emit(split_frame(frame));
    }
}
