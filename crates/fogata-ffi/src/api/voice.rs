//! Voice messages in direct chats: recording one from the microphone chosen in the
//! application and sending it, and playing one at 1×, 1.5× or 2×. One message plays at a
//! time.

use anyhow::{Result, anyhow};
use flutter_rust_bridge::frb;
use fogata_core::direct::VoiceNote;

use super::app::{blocking, device, file_id, run};
use crate::voice::{play, record};

/// A recording under way, or stopped and waiting for Send or Delete.
#[derive(Clone, Debug)]
pub struct UiVoiceRecording {
    pub elapsed_ms: i64,
    /// The latest levels, oldest first, one for each 200 ms, 0 to 255.
    pub bars: Vec<u8>,
    /// The microphone is recording now.
    pub capturing: bool,
    /// 15 minutes recorded: it stopped by itself.
    pub full: bool,
    /// The microphone could not be used.
    pub failed: bool,
}

/// The voice message playing or paused.
#[derive(Clone, Debug)]
pub struct UiVoicePlayback {
    /// The attachment's id and its chat's friend.
    pub file: String,
    pub device_id: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub playing: bool,
    pub speed: f64,
}

fn ms(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Starts recording from `input` (a device id as the voice engine lists it; `None` for
/// the system's default), with noise suppression when `noise_suppression`. Fails when no
/// microphone can be used; otherwise true when `input` could not be used and the system's
/// default microphone records instead.
pub async fn voice_record_start(input: Option<String>, noise_suppression: bool) -> Result<bool> {
    blocking(move || record::start(input, noise_suppression))
        .await?
        .map_err(|e| anyhow!("{e}"))
}

/// The recording as it stands, if one is under way or waiting.
#[frb(sync)]
pub fn voice_recording() -> Option<UiVoiceRecording> {
    record::state().map(|s| UiVoiceRecording {
        elapsed_ms: ms(s.elapsed_ms),
        bars: s.bars,
        capturing: s.capturing,
        full: s.full,
        failed: s.failed,
    })
}

/// Stops the microphone and keeps what was recorded, waiting for Send or Delete.
#[frb(sync)]
pub fn voice_record_stop() {
    record::stop();
}

/// Throws the recording away.
#[frb(sync)]
pub fn voice_record_cancel() {
    record::cancel();
}

/// Ends the recording and sends it to the friend; false when there was nothing to send
/// (none, or under half a second). One that does not go stays, to send again.
pub async fn voice_record_send(device_id: String) -> Result<bool> {
    let friend = device(&device_id)?;
    let serial = {
        let mut bytes = [0u8; 4];
        fogata_crypto::random_bytes(&mut bytes)?;
        u32::from_le_bytes(bytes)
    };
    let (recording, id) = blocking(move || record::finish(serial))
        .await?
        .map_err(|e| anyhow!("{e}"))?;
    let Some(recording) = recording else {
        record::forget(id);
        return Ok(false);
    };
    let note = VoiceNote {
        file: recording.file,
        duration_ms: recording.duration_ms,
        bars: recording.bars,
    };
    run(|core| async move { core.send_direct_voice(friend, note).await }).await?;
    record::forget(id);
    Ok(true)
}

/// Plays a voice message of the chat with `device_id` at `speed`: on from where it was
/// paused, or from its start; whatever else played stops.
pub async fn voice_play(device_id: String, file: String, speed: f64) -> Result<()> {
    if play::is_loaded(&device_id, &file) {
        play::resume(speed);
        return Ok(());
    }
    let (friend, id) = (device(&device_id)?, file_id(&file)?);
    let bytes = run(|core| async move {
        core.direct_file_bytes(friend, id, fogata_voice::MAX_FILE_BYTES as u64)
            .await
    })
    .await?;
    // Reading the file checks all of it (up to 4 MB): not on the threads that run the
    // core's work.
    blocking(move || play::play(device_id, file, bytes, speed))
        .await?
        .map_err(|e| anyhow!("{e}"))
}

/// Pauses the voice message playing; it goes on from there.
#[frb(sync)]
pub fn voice_pause() {
    play::pause();
}

/// Plays the voice message loaded at `speed` from where it is.
#[frb(sync)]
pub fn voice_set_speed(speed: f64) {
    play::set_speed(speed);
}

/// Stops and forgets the voice message playing or paused.
#[frb(sync)]
pub fn voice_stop() {
    play::stop();
}

/// The voice message playing or paused, if any.
#[frb(sync)]
pub fn voice_playback() -> Option<UiVoicePlayback> {
    play::state().map(|s| UiVoicePlayback {
        file: s.file,
        device_id: s.friend,
        position_ms: ms(s.position_ms),
        duration_ms: ms(s.duration_ms),
        playing: s.playing,
        speed: s.speed,
    })
}
