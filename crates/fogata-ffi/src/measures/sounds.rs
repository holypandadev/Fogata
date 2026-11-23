//! The sound file writer and the player of every sound, run by hand (`--ignored`).

use super::tests::{every, every_variant, heard_for, peak, spectrum, wav, widened};
use super::*;

/// Writes every variant of every sound as a 48 kHz stereo WAV file into the folder
/// named by `FOGATA_SOUNDS_OUT`, to listen to them side by side (the rings twice, as
/// they repeat), with `measures.tsv`: each one's length, how long it is heard, its
/// loudness, peak, spectral centroid and share of power between 2 and 5 kHz.
#[test]
#[ignore = "writes files; run by hand with FOGATA_SOUNDS_OUT set and --ignored"]
fn writes_every_variant() {
    let dir = std::path::PathBuf::from(
        std::env::var("FOGATA_SOUNDS_OUT").expect("FOGATA_SOUNDS_OUT names a folder"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let mut measures = String::from(
        "sound\tvariant\tseconds\theard_s\tlufs\tpeak_dbfs\tcentroid_hz\tshare_2_5k\n",
    );
    for (sound, variant) in every_variant() {
        let frames = render_as(sound, variant, 48_000);
        let (centroid, band) = spectrum(&frames, 48_000.0);
        measures.push_str(&format!(
            "{sound:?}\t{variant:?}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\t{centroid:.0}\t{band:.4}\n",
            frames.len() as f32 / 48_000.0,
            heard_for(&frames, 48_000, -30.0),
            loudness(&widened(&frames), 48_000.0),
            20.0 * peak(&frames).log10(),
        ));
        let played: Vec<Frame> = match every(sound, variant) {
            Some(every) => {
                let mut queue = VecDeque::new();
                mix_into(&mut queue, &frames);
                let later = (every * 48_000.0) as usize;
                let mut second = vec![[0.0; 2]; later];
                second.extend_from_slice(&frames);
                mix_into(&mut queue, &second);
                queue.into_iter().collect()
            }
            None => frames,
        };
        std::fs::write(dir.join(format!("{sound:?}-{variant:?}.wav")), wav(&played)).unwrap();
    }
    std::fs::write(dir.join("measures.tsv"), measures).unwrap();
}

/// Plays every sound on this computer's default output the way the application does,
/// one every 1.5 s, then a ring with a message and a notice over it.
#[test]
#[ignore = "makes noise; run by hand with --ignored"]
fn plays_on_the_default_output() {
    for sound in Sound::ALL {
        play(sound);
        thread::sleep(Duration::from_millis(1500));
    }
    play(Sound::Ring);
    thread::sleep(Duration::from_millis(300));
    play(Sound::Message);
    thread::sleep(Duration::from_millis(300));
    play(Sound::Notice);
    thread::sleep(Duration::from_millis(1500));
}
