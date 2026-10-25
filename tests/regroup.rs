use ac108::regroup::Regrouper;

/// What the AC108 puts on the wire: rounds of mic1..mic4. Each sample encodes
/// (round, mic) so a test can tell exactly where it came from.
fn wire(rounds: usize) -> Vec<i32> {
    (0..rounds).flat_map(|r| (0..4).map(move |m| sample(r, m))).collect()
}

fn sample(round: usize, mic: usize) -> i32 {
    (round as i32) * 16 + mic as i32
}

/// ALSA hands us the same samples as interleaved 2-channel frames.
fn frames_out(rg: &mut Regrouper, alsa: &[i32]) -> Vec<[i32; 4]> {
    let mut out = Vec::new();
    rg.push(alsa, |f| out.push(f));
    out
}

#[test]
fn regroups_pairs_into_four_mic_frames() {
    let mut rg = Regrouper::new(0);
    let got = frames_out(&mut rg, &wire(3));
    assert_eq!(
        got,
        vec![
            [sample(0, 0), sample(0, 1), sample(0, 2), sample(0, 3)],
            [sample(1, 0), sample(1, 1), sample(1, 2), sample(1, 3)],
            [sample(2, 0), sample(2, 1), sample(2, 2), sample(2, 3)],
        ]
    );
}

#[test]
fn half_frame_phase_skips_to_the_start_of_the_next_round() {
    // Capture locked on at mic3: the stream starts with (mic3, mic4).
    let alsa = &wire(3)[2..];
    let mut rg = Regrouper::new(1);
    let got = frames_out(&mut rg, alsa);
    assert_eq!(
        got,
        vec![
            [sample(1, 0), sample(1, 1), sample(1, 2), sample(1, 3)],
            [sample(2, 0), sample(2, 1), sample(2, 2), sample(2, 3)],
        ]
    );
}

#[test]
fn reads_can_split_anywhere_without_losing_or_shifting_samples() {
    let alsa = wire(50);
    let mut whole = Regrouper::new(0);
    let want = frames_out(&mut whole, &alsa);

    // ALSA delivers whole 2-channel frames, so splits land on even indexes;
    // vary them to cross every alignment within a 4-mic round.
    for chunk in [2, 4, 6, 10, 26, 98] {
        let mut rg = Regrouper::new(0);
        let mut got = Vec::new();
        for part in alsa.chunks(chunk) {
            rg.push(part, |f| got.push(f));
        }
        assert_eq!(got, want, "chunk size {chunk}");
    }
}

#[test]
#[should_panic(expected = "whole 2-channel frames")]
fn rejects_half_a_2_channel_frame() {
    let mut rg = Regrouper::new(0);
    rg.push(&[1, 2, 3], |_| {});
}
