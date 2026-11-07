//! Voice messages on this computer: recording one from the microphone ([`record`]) and
//! playing one ([`play`]). The files themselves are made and read by `fogata-voice`, and
//! travel as a direct chat's attachments.

pub(crate) mod play;
pub(crate) mod record;

/// `$build::<T>(...)` for the sample type `T` of the cpal format `$format`, in `Some`;
/// `None` for one no stream here takes (DSD, which is not sound in samples). Devices
/// work in any of them: some only in 8 or 24 bits, or in 64-bit floats.
macro_rules! by_sample_type {
    ($format:expr, $build:ident($($arg:expr),* $(,)?)) => {
        match $format {
            cpal::SampleFormat::I8 => Some($build::<i8>($($arg),*)),
            cpal::SampleFormat::I16 => Some($build::<i16>($($arg),*)),
            cpal::SampleFormat::I24 => Some($build::<cpal::I24>($($arg),*)),
            cpal::SampleFormat::I32 => Some($build::<i32>($($arg),*)),
            cpal::SampleFormat::I64 => Some($build::<i64>($($arg),*)),
            cpal::SampleFormat::U8 => Some($build::<u8>($($arg),*)),
            cpal::SampleFormat::U16 => Some($build::<u16>($($arg),*)),
            cpal::SampleFormat::U24 => Some($build::<cpal::U24>($($arg),*)),
            cpal::SampleFormat::U32 => Some($build::<u32>($($arg),*)),
            cpal::SampleFormat::U64 => Some($build::<u64>($($arg),*)),
            cpal::SampleFormat::F32 => Some($build::<f32>($($arg),*)),
            cpal::SampleFormat::F64 => Some($build::<f64>($($arg),*)),
            _ => None,
        }
    };
}
pub(crate) use by_sample_type;

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    /// Records three seconds from the input `FOGATA_VOICE_MIC` names (the default when not
    /// set) into the file `FOGATA_VOICE_OUT`. Run by hand, with a virtual microphone.
    #[test]
    #[ignore]
    fn records_from_the_input_named() {
        let mic = std::env::var("FOGATA_VOICE_MIC").ok();
        let fell_back = record::start(mic, std::env::var("FOGATA_VOICE_DENOISE").is_ok()).unwrap();
        eprintln!("the default microphone instead: {fell_back}");
        thread::sleep(Duration::from_secs(3));
        let state = record::state().unwrap();
        assert!(!state.failed && state.capturing, "{state:?}");
        assert!(state.elapsed_ms > 2500, "{state:?}");
        let rec = record::finish(7).unwrap().0.unwrap();
        std::fs::write(std::env::var("FOGATA_VOICE_OUT").unwrap(), &rec.file).unwrap();
        eprintln!(
            "{} ms, {} bytes, bars {:?}",
            rec.duration_ms,
            rec.file.len(),
            rec.bars
        );
    }

    /// Plays the file `FOGATA_VOICE_FILE` at `FOGATA_VOICE_SPEED` (1 when not set) on the
    /// default output, to its end. Run by hand, with a null sink as the output.
    #[test]
    #[ignore]
    fn plays_a_file_to_its_end() {
        let bytes = std::fs::read(std::env::var("FOGATA_VOICE_FILE").unwrap()).unwrap();
        let speed = std::env::var("FOGATA_VOICE_SPEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        play::play("friend".into(), "file".into(), bytes, speed).unwrap();
        let start = std::time::Instant::now();
        // Paused for a second after `FOGATA_VOICE_PAUSE_AT` ms, when set.
        if let Some(at) = std::env::var("FOGATA_VOICE_PAUSE_AT")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            thread::sleep(Duration::from_millis(at));
            play::pause();
            eprintln!("paused at {} ms", play::state().unwrap().position_ms);
            thread::sleep(Duration::from_secs(1));
            play::resume(speed);
        }
        while play::state().is_some_and(|s| s.playing) {
            thread::sleep(Duration::from_millis(50));
            assert!(start.elapsed() < Duration::from_secs(120));
        }
        let state = play::state().unwrap();
        assert_eq!(state.position_ms, state.duration_ms);
        eprintln!(
            "played {} ms in {} ms",
            state.duration_ms,
            start.elapsed().as_millis()
        );
        // What the output still holds.
        thread::sleep(Duration::from_millis(2000));
    }

    /// A stream of every format of samples cpal names plays and records, but DSD.
    #[test]
    fn every_format_of_samples_is_taken() {
        fn taken<T: cpal::SizedSample + cpal::FromSample<f32>>() -> bool
        where
            f32: cpal::FromSample<T>,
        {
            true
        }
        use cpal::SampleFormat::*;
        for format in [I8, I16, I24, I32, I64, U8, U16, U24, U32, U64, F32, F64] {
            assert_eq!(by_sample_type!(format, taken()), Some(true), "{format}");
        }
        assert_eq!(by_sample_type!(DsdU8, taken()), None);
    }

    /// The limits a friend's copy checks are the ones a recording here keeps to.
    #[test]
    fn limits_agree_with_what_travels() {
        assert_eq!(fogata_voice::MAX_MS, fogata_proto::MAX_VOICE_MS);
        assert_eq!(fogata_voice::BARS, fogata_proto::VOICE_BARS);
        assert_eq!(
            fogata_voice::MAX_FILE_BYTES as u64,
            fogata_proto::MAX_VOICE_BYTES
        );
    }
}
