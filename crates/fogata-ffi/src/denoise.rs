//! The microphone's noise suppression for flutter_webrtc's native code, which runs it on
//! WebRTC's audio thread after WebRTC's own processing: a table of C functions, whose
//! address the application hands to that code (see `api::app::noise_suppressor`).

use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

use fogata_denoise::Denoiser;

/// What `FogataDenoiseApi` in flutter_webrtc's `fogata_denoise.h` (Linux, Windows) and
/// `FogataDenoise.h` (macOS) reads. A change here changes `version` there too.
#[repr(C)]
pub(crate) struct DenoiseApi {
    version: u32,
    /// A new suppressor for one channel; never freed by its user.
    create: extern "C" fn() -> *mut c_void,
    /// One 10 ms frame of `frames` samples at `rate`, cleaned in place when `on`.
    process: unsafe extern "C" fn(*mut c_void, bool, i32, *mut f32, usize),
}

pub(crate) static API: DenoiseApi = DenoiseApi {
    version: 1,
    create,
    process,
};

extern "C" fn create() -> *mut c_void {
    Box::into_raw(Box::new(Denoiser::new())).cast()
}

/// # Safety
/// `state` comes from `create` and is used by one thread at a time; `samples` holds
/// `frames` floats.
#[allow(unsafe_code)]
unsafe extern "C" fn process(
    state: *mut c_void,
    on: bool,
    rate: i32,
    samples: *mut f32,
    frames: usize,
) {
    // Only 10 ms at a rate the suppressor knows, so a bad length never becomes a slice
    // longer than what the caller holds.
    if state.is_null() || samples.is_null() || !Denoiser::accepts(rate, frames) {
        return;
    }
    // SAFETY: as the caller promises above.
    let (denoiser, frame) = unsafe {
        (
            &mut *state.cast::<Denoiser>(),
            std::slice::from_raw_parts_mut(samples, frames),
        )
    };
    // A panic must not cross into WebRTC's thread; the frame then goes as it came.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        denoiser.process(on, rate as usize, frame);
    }));
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    /// Through the table as the native code calls it.
    #[test]
    fn through_the_table() {
        let state = (API.create)();
        let mut frame = vec![1000.0f32; 480];
        for _ in 0..10 {
            // SAFETY: a state from `create` and a frame of 480 samples.
            unsafe { (API.process)(state, true, 48000, frame.as_mut_ptr(), frame.len()) };
        }
        assert!(frame.iter().all(|x| x.is_finite()));
        let mut odd = vec![5.0f32; 100];
        // SAFETY: as above, with a frame of 100 samples (left alone).
        unsafe { (API.process)(state, true, 48000, odd.as_mut_ptr(), odd.len()) };
        assert!(odd.iter().all(|&x| x == 5.0));
        // A length that does not match the rate, or a rate it does not know, is refused
        // before the samples are read: here the frame is far shorter than `frames`.
        let mut short = vec![7.0f32; 16];
        for (rate, frames) in [(48000, 4800), (480_000, 4800), (0, 0), (-48000, 480)] {
            // SAFETY: refused before `samples` is read, as the test checks.
            unsafe { (API.process)(state, true, rate, short.as_mut_ptr(), frames) };
        }
        assert!(short.iter().all(|&x| x == 7.0));
        // SAFETY: made by `create` above and no longer used.
        drop(unsafe { Box::from_raw(state.cast::<Denoiser>()) });
    }
}
