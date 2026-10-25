use ac108::frames::{split, split_frame};

/// What one 64-bit TDM frame of four 16-bit slots looks like once the Pi has
/// captured it as two 32-bit ALSA samples: slots 1+2 in the first, 3+4 in the second.
fn alsa_frame(slots: [i16; 4]) -> [i32; 2] {
    let pack = |hi: i16, lo: i16| ((hi as u16 as u32) << 16 | lo as u16 as u32) as i32;
    [pack(slots[0], slots[1]), pack(slots[2], slots[3])]
}

#[test]
fn splits_one_alsa_frame_into_four_mics_in_slot_order() {
    assert_eq!(split_frame(alsa_frame([1, 2, 3, 4])), [1, 2, 3, 4]);
}

#[test]
fn keeps_the_sign_of_every_half() {
    for slots in [[-1, 1, -32768, 32767], [32767, -32768, -2, 0], [-12345, -1, 0, 12345]] {
        assert_eq!(split_frame(alsa_frame(slots)), slots);
    }
}

#[test]
fn splits_an_interleaved_alsa_buffer() {
    let frames = [[10, -20, 30, -40], [-50, 60, -70, 80], [0, 0, 0, 1]];
    let alsa: Vec<i32> = frames.iter().flat_map(|&f| alsa_frame(f)).collect();
    let mut got = Vec::new();
    split(&alsa, |f| got.push(f));
    assert_eq!(got, frames);
}

#[test]
#[should_panic(expected = "whole 2-channel frames")]
fn rejects_half_an_alsa_frame() {
    split(&[1, 2, 3], |_| {});
}
