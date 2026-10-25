//! Turns the 2-channel ALSA stream back into 4-mic frames.
//!
//! The Pi's I2S block captures 2 slots per frame, so each AC108 round of
//! mic1..mic4 arrives as two ALSA frames: (mic1, mic2) then (mic3, mic4).

pub struct Regrouper {
    /// Drop the leading (mic3, mic4) pair when capture locked on mid-round.
    skip_first_pair: bool,
    /// The (mic1, mic2) pair waiting for its (mic3, mic4) partner. Kept across
    /// `push` calls, so ALSA reads can end anywhere.
    pending: Option<[i32; 2]>,
}

impl Regrouper {
    /// `phase` is 0 if the stream starts with (mic1, mic2), 1 if it starts
    /// with (mic3, mic4).
    pub fn new(phase: usize) -> Self {
        assert!(phase <= 1, "phase must be 0 or 1");
        Regrouper { skip_first_pair: phase == 1, pending: None }
    }

    /// Feed interleaved 2-channel samples as read from ALSA; `emit` gets each
    /// complete [mic1, mic2, mic3, mic4] frame.
    pub fn push(&mut self, samples: &[i32], mut emit: impl FnMut([i32; 4])) {
        assert!(samples.len().is_multiple_of(2), "push takes whole 2-channel frames");
        for &pair in samples.as_chunks::<2>().0 {
            if std::mem::take(&mut self.skip_first_pair) {
                continue;
            }
            match self.pending.take() {
                None => self.pending = Some(pair),
                Some([a, b]) => emit([a, b, pair[0], pair[1]]),
            }
        }
    }
}
