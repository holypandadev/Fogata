# flutter_webrtc in Fogata

This is flutter_webrtc 1.6.3 from pub.dev (MIT, see LICENSE), without its
`example` and `test` folders, its README.md a short note pointing here, and with a
`.gitignore` of its own for the libwebrtc archives its CMake build downloads and unpacks
into `third_party/` (`downloads/`, `libwebrtc/`). Fogata keeps its own copy to change how
the screen is captured on Linux. Every change from the published package is listed here.

The Objective-C code comes twice, as published: `common/darwin/Classes` and
`macos/flutter_webrtc/Sources/flutter_webrtc`. Only the second is built (the macOS podspec
compiles `Sources` alone; iOS is never built). Where a change below says "both copies,
identical", each file it names is the same in both, and `tools/test_repo_checks.py` fails when
a file present in both differs. Files Fogata added (`Fogata*`) are in the built copy only.
`tools/vendor_diff.py` (weekly) compares this fork with the package on pub.dev and says when
pub.dev has a newer one, unless a line "Reviewed against flutter_webrtc <version>" here names
it.

The fork was moved from 1.6.2+hotfix.4 to 1.6.3 on 2026-11-15: what of 1.6.3 it does not
have, and why, is in "Upstream 1.6.3" at the end.

## Screen capture under Wayland (Linux)

libwebrtc's own capture under Wayland asks the portal from its source list as well as from
the capturer (two dialogs, and a hidden second capture that keeps running), offers no
windows, may deliver every frame marked as corrupted, and crashed when asked for a window
list (before `tools/webrtc/patches/wrapper/0007`). Instead, `getDisplayMedia` without a source id, in a Wayland session:

- `linux/portal_screen_capturer.{h,cc}` (new): asks the xdg-desktop-portal ScreenCast
  interface once, for screens and windows, with the cursor drawn in; opens the stream it
  gives in PipeWire, in plain memory; turns each frame into I420 (each colour sample the
  average of its 2x2 block; `linux/portal_i420.h`, loops a release build vectorizes, as
  libwebrtc does not export libyuv; `common/cpp/test/portal_i420_test.cc`), at most the
  asked frame rate. The session bus and the portal's properties are reached without blocking the
  platform thread.
- `common/cpp/src/flutter_screen_capture.cc`: `GetDisplayMediaFromPortal` feeds those
  frames to a custom video source and answers once the person has chosen (an error
  "cancelled" if they closed the dialog).
- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_media_stream.cc`:
  `track_stoppers_` ends that capture and its portal session when its track or stream is
  disposed.
- `linux/CMakeLists.txt`: builds it with libpipewire-0.3 and gio when found.
- When the system ends the capture (its own "stop sharing", or the shared window
  closed: the portal session's `Closed` signal), an event `mediaStreamTrackEnded` goes
  to Dart and `lib/src/native/media_stream_track_impl.dart` runs that local track's
  `onEnded`, which the package never called before. A PipeWire stream that fails after it
  started (`PW_STREAM_STATE_ERROR`) ends the capture the same way, on the GLib main thread.
- Each buffer asks for a header (`SPA_META_Header`); a frame whose header or chunk is
  marked corrupted is skipped.
- A capture stopped while a portal request is still open (the dialog showing) closes that
  request, and with it the dialog, and stops listening for its answer; a session being
  made is closed by its handle, known before the answer.
- `linux/desktop_session.{h,cc}` (new): `WaylandSession` says whether GTK draws on a
  Wayland display (`GDK_IS_WAYLAND_DISPLAY` on the default display). The environment can
  say otherwise (sway, or variables imported by systemd, name both displays and no session
  type), and libwebrtc's lists would then be XWayland's, whose capture is black. Without a
  GTK display the environment decides (`WaylandEnvironment`, the same rule as call.dart's
  `waylandSession`: a Wayland display named, an empty name counting as for WebRTC, and the
  session saying Wayland or no X display named, as a program started from a user service
  has). `PortalScreenCapturer::Available` goes by it, `BuildDesktopSourcesList` asks
  libwebrtc for no list where the portal shares the screen, and the method
  `fogataWaylandSession` tells call.dart. Tested by `common/cpp/test/desktop_session_test.cc`,
  which also checks that libwebrtc itself (`tools/webrtc/patches/wrapper/0007`) gives empty
  lists and a capturer that does not start where it can make no capturer.
- `common/cpp/src/flutter_screen_capture.cc` (Linux, Windows) and `FlutterRTCDesktopCapturer.m`
  (macOS, both copies): `getDisplayMedia` answers an error, and keeps no stream or track,
  when the screen's or window's capturer does not start (libwebrtc made none for it, or the
  source could not be selected: a window that has closed); the package answered with a
  stream that never had a picture. On Linux and Windows the screen's sound is started only
  once the picture has: a share captured again at another rate keeps its sound when the new
  capture cannot start.

## Frame rate of a screen captured through the portal (Linux)

- `linux/portal_screen_capturer.{h,cc}`: `SetFps` changes how many frames a second go
  on, while the capture runs (no more than it was started at). The limit holds on
  average: a frame up to a quarter of the interval early goes on, as frames at the rate
  asked for come with jitter.
- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_screen_capture.cc`,
  `common/cpp/src/flutter_media_stream.cc`: `track_rates_` keeps that setter by track,
  dropped with the track.
- `common/cpp/src/flutter_webrtc.cc`: the method `fogataScreenFps` (`trackId`, `fps`)
  calls it, and answers false for any other track.

## The last frame of a shared screen again (Linux portal, macOS)

The portal's capture (PipeWire) and ScreenCaptureKit give a frame only when what is shared
changes. A shared screen goes to each watcher on a connection of its own, and that
connection's encoder starts with the first frame it gets: someone who started watching a
screen that stayed still (a slide, a page being read) got nothing, and saw a spinner until
something on it moved. WebRTC itself asks a source for a frame again only for a screen in
its "zero-hertz" mode, which these sources are not in (see "One thread for each video
encoder").

- `linux/portal_screen_capturer.{h,cc}`: keeps the last frame it converted (its I420
  buffer was kept already) and `Refresh` gives it again, under PipeWire's loop lock. With
  `FOGATA_WEBRTC_LOG` set (any level), each buffer PipeWire gives and each frame given
  again is a line on standard error with its time, from which
  `tools/smoke-wayland/still.sh` proves the shared screen stayed still.
- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_screen_capture.cc`,
  `common/cpp/src/flutter_media_stream.cc`: `track_refreshers_` keeps it by track,
  dropped with the track, as `track_rates_`.
- `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterScreenCaptureKitCapturer.m` and its
  header: keeps the last frame's pixel buffer, and `refreshFrame` gives it again, timed
  now on the clock ScreenCaptureKit's frames are timed on.
- `FlutterRTCDesktopCapturer.{h,m}` (both copies, identical): `refreshScreenTrack:` finds
  the running ScreenCaptureKit capture by its track (kept from its start to its stop).
- `common/cpp/src/flutter_webrtc.cc` and `FlutterWebRTCPlugin.m` (both copies, identical):
  the method `fogataScreenRefresh` (`trackId`) does it, and answers false for any other
  track (the other captures give frames all the time). `lib/src/call/media.dart` asks it
  when someone starts watching, and again while nothing has gone out to them.

## Bandwidth estimate on lossy links (all platforms)

WebRTC's newer loss-based bandwidth estimate (`WebRTC-Bwe-LossBasedBweV2`) holds the
estimate where it is and stops probing while packets are lost at random, as on Wi-Fi or
satellite links: a shared screen stayed at a few hundred kbit/s on links with far more
room. It is turned off, so the older one lowers the estimate only past 10 % loss; the
delay-based estimate still finds the limit of a congested link.

- `common/cpp/src/flutter_webrtc_base.cc` (Linux, Windows): `kFogataFieldTrials` goes
  into the field trials the factory is made with.
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS): `kFogataFieldTrials` is
  added to the string given to `configureFieldTrials:`.

## One thread for each video encoder (all platforms)

libvpx gives a VP8 encoder of more than 640×480 pixels three threads (on a computer with six
or more), which wait for each other by spinning. Each person watching a camera has an encoder
of their own (a shared screen has one for all who watch it at one size: "One encode of a
shared screen" below), so a call of four with cameras at 540p ran nine such threads: on a
16-thread desktop they took 2.1 cores and the cameras went out at 21 frames a second; with one
thread each, 0.63 cores and 30 (measured with `tools/call-lab`).

- `common/cpp/src/flutter_webrtc_base.cc` (Linux, Windows) and `FlutterWebRTCPlugin.m` (both
  copies, identical; macOS): `kFogataFieldTrials` also has
  `WebRTC-VideoEncoderSettings/encoder_thread_limit:1/`.
- In WebRTC M156 only the VP8 encoder reads that limit (`LibvpxVp8Encoder`, the only video
  codec Fogata's builds have); H.264 already encodes on one thread, and VP9 and AV1 ignore
  it. Another codec, if one is ever added, needs its own look.
- The limit holds for a shared screen too, and one thread may not encode a moving 1080p
  screen at 60 frames a second: WebRTC then says the processor holds it back, and the app
  lowers the screen's rate (`ScreenLoad` in `lib/src/call/fitting.dart`).
  `VideoStreamEncoder` reads the field trial once, from the factory's (or a whole
  connection's) field trials, and hands the same limit to every encoder, and the wrapper's
  API makes no encoder factory of the plugin's own. On Linux and Windows a desktop source
  is not marked as a screen (`is_screencast()` is false in the wrapper's
  `ScreenCapturerTrackSource`, and the portal's capture is a custom source), so WebRTC
  encodes it as a camera. On macOS it is one: the plugin asks for
  `videoSourceForScreenCast:YES`, whose `ObjCVideoTrackSource` answers `is_screencast()`
  true, so the screen is encoded as screen content there. The one encoder a shared screen
  now has can be given more threads alone, through the shared encode's `threads`
  parameter (below); that is to be measured before it is set.

## One encode of a shared screen (all platforms)

Every connection that sends a track has a `VideoStreamEncoder` of its own, which made an
encoder of its own: a screen shared to seven watchers was encoded seven times, with seven
libvpx encoders, their buffers and their threads, although each is given the same frames.
libwebrtc's `SharedEncoderFactory` (wrapper patch 17, `src/internal/shared_encoder.h`; the
macOS framework through Apple patch 7) has the connections that send one screen at the same
settings share one encoder: a frame is encoded once and every connection gets the image with
its own RTP timestamps. Each connection still packetizes, encrypts, paces and estimates on its
own, so nothing on the wire changes and a watcher of any version sees no difference. A
watcher's first key frame is made at once; key frames one watcher asks for are given to
all (at most one a second), and one owed on a still screen is made of its last picture
again. A new watcher's encoder still needs one frame to ask for (`fogataScreenRefresh` gives
it on a still screen); that frame now brings its key frame at once; the bitrate is the
lowest watcher's;
a watcher far slower than the others, or losing key frames again and again, gets an
encoder of its own, and every watcher gets one when the shared encoder keeps failing.

- The field trial `Fogata-SharedScreenEncode/Enabled/` is set with Fogata's own
  (`common/cpp/src/flutter_webrtc_base.cc`; `FlutterWebRTCPlugin.m`, both copies). With
  `FOGATA_SHARED_SCREEN_ENCODE=off` in the environment it is left out, in every build (it
  only gives the saving up): `tools/call-lab` measures before and after on one build that
  way. On macOS too, since it was run there (`tools/webrtc/MACOS.md`, "Checking patch 7"):
  until then it was off unless `FOGATA_SHARED_SCREEN_ENCODE=on`, and with that set it
  shared nothing, for the reason the next point gives. `FOGATA_WEBRTC_FIELD_TRIALS` (debug builds) replaces it with the others, and can set
  its parameters, e.g. `Fogata-SharedScreenEncode/Enabled,key_ms:500,threads:2/`
  (`SharedEncodeConfig` in the wrapper's header lists them).
- An encoder is shared only if it says, before it is initialized, that it encodes in
  software (`SharedInner`). On macOS the plugin's factory gave WebRTC's
  `SimulcastEncoderAdapter` over libvpx, which says nothing of its own until it is
  initialized, so WebRTC's default stood ("hardware") and every connection kept an encoder
  of its own. With the shared encode on, the macOS plugin gives the peer connection
  factory its VP8 factory itself (`FlutterWebRTCPlugin.m`, both copies): libvpx's own
  encoder, which says it is software. Calls send one stream of VP8, which the adapter only
  passed on. With `FOGATA_SHARED_SCREEN_ENCODE=off` the adapter is used as before.
- Only frames of a source marked as a shared screen are shared: the plugin calls
  `RTCVideoSource::EnableSharedEncode()` on the desktop source and on the portal's custom
  source (`common/cpp/src/flutter_screen_capture.cc`), and `enableFogataSharedEncode` on
  the screen's `RTCVideoSource` on macOS (`FlutterRTCDesktopCapturer.m`, both copies,
  identical; asked with `respondsToSelector:`, so a framework without Apple patch 7 still
  builds and runs, without sharing). Cameras are never marked; a camera's connection has
  its encoder from `InitEncode` on, as without the trial. The source also marks its frames
  as its own (`MarkSharedEncodeFrame`): the black frames VideoBroadcaster makes for a
  disabled track carry the frame ids but not the mark, and are never shared.
- `fogataSharedScreenEncode` answers whether it is on: the trial is set (and, on macOS, the
  framework has Apple patch 7; on Linux and Windows `libwebrtc_version.ini` pins a library
  that has it). The app then counts the shared encode once per size in its processor load
  (`ScreenLoad` in `lib/src/call/fitting.dart`).
- A connection's `outbound-rtp` statistics name the encoder `libvpx (shared)` while it shares
  one (`encoderImplementation`); its `totalEncodeTime` then includes the wait for the shared
  encode, and its `framesEncoded` and `keyFramesEncoded` the frames and key frames made for
  all.

## libwebrtc's log and field trials for tests (Linux, Windows, macOS)

- `common/cpp/src/flutter_webrtc_base.cc`: with `FOGATA_WEBRTC_LOG` set to `verbose`,
  `info`, `warning` or `error`, libwebrtc's own log goes to standard error, each line with
  the time, from before the factory is made: what the bandwidth estimate does, probes,
  route changes. `FOGATA_WEBRTC_FIELD_TRIALS` ("Key/Value/Key/Value/") replaces Fogata's
  own field trials, to try others; release builds (`NDEBUG`) ignore it. Nothing changes
  without them.
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS): with `FOGATA_WEBRTC_LOG` set,
  `initialize` takes its level instead of the `logSeverity` option, and every line of the
  log also goes to standard error.

## The chosen microphone stays chosen (Linux, Windows)

libwebrtc keeps the chosen input only as its place in the list of inputs, and on Linux
(PulseAudio, or PipeWire's PulseAudio service) it reads that list again each time it starts
recording: once an input that came before it goes away (a headset unplugged, a virtual
device removed), the same place is the next input, or past the end the system's default
microphone. A call that stopped recording (alone in a room) and started again when someone
came back recorded from another microphone that way.

- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_webrtc_base.cc`:
  `SelectRecordingDevice` keeps the chosen input's id with its place; `KeepRecordingDevice`
  finds it in the list again and selects it at its new place (while it is gone, the
  system's default, as a recording whose input goes away moves there by itself; it is taken
  again when it comes back). A list that cannot be read changes nothing.
- `common/cpp/src/flutter_media_stream.cc`: `getUserMedia` and `selectAudioInput` select
  through `SelectRecordingDevice`, both the first input of the id asked for (`getUserMedia`
  selected every input of that id, one after the other: on Linux, where the id is the
  description, two microphones of the same model ended on the other one than the call
  chose, its recording started again on the way).
- The input recorded from already is not asked for again. The wrapper's
  `SetRecordingDevice` (`rtc_audio_device_impl.cc`), while recording, stops the recording,
  selects and starts it again within a millisecond, whatever the input. A call selects its
  input as it starts and `getUserMedia` selects it again when the microphone opens; when
  someone already hears this device then (the microphone opened late, or turned on in the
  middle of a call), the voice engine records already, and on PipeWire 1.0 (Ubuntu 24.04)
  a remapped or filtered source answered that restart with silence to every recording of
  it until none was left: the device sent only silence (`tools/call-lab/mic.sh late`, with
  `LAB_FIXED_QUANTUM=0`). `SelectRecordingDevice` now leaves it as it is when the input is
  the one the plugin asked for last, at the same place, by the same id (the guid, Windows'
  endpoint id; on Linux, which gives none, the description in the sound server; nothing
  for the list's first entry, the system's default), with no change counted since. What
  was asked for is kept, not only what was chosen: while the chosen input is gone,
  `KeepRecordingDevice` asks for the default, and choosing the default then is the same.
  The counter `input_changes_` grows as each of these is told, before the list is read:
  a notice of Linux's input watcher (a source added to the sound server or removed from
  it, or the watcher connected again); libwebrtc's own device change (on Linux only the
  sound server lost and found again, on Windows a change of state of any endpoint,
  outputs too); and the voice engine kept (`KeepVoiceEngine`), as its start selects the
  system's default (the default communication device on Windows) whatever was asked for
  before. An input unplugged and plugged in again at the same place, with the same id, is
  counted, so it is asked for anew; a different input, a different place or the first
  selection always are. Whether the audio device took a selection is not known: the
  wrapper does it later on WebRTC's network thread and always answers 0, so a refusal there
  goes unseen until the next selection after a change counted, or until
  `KeepRecordingDevice` finds the input at another place. With `FOGATA_WEBRTC_LOG`,
  standard error says "Fogata: input N selected", "selected again" (`KeepRecordingDevice`)
  or "is selected already, left as it is". Linux and Windows share this code; macOS has
  its own audio module and plugin code, unchanged. Known limits:
  - Anything counted between the call's selection and the microphone's opening (a
    Bluetooth headset connecting, the monitor of a new output, the application's own silent
    input for a shared screen's sound) has the same input asked for again, and its
    recording started again once, as before: the silence on PipeWire 1.0 is avoided only
    while nothing was counted in between.
  - With WebRTC's ALSA module (Linux without a PulseAudio or PipeWire sound server) nothing
    is counted but the voice engine's start: the same input at the same place is never
    asked for again, so an ALSA device unplugged and plugged in again there is taken for
    the same, and only the voice engine stopping and starting its recording (the
    microphone turned off and on) opens it again.
  - The system's default input changing while the default is chosen is not counted: the
    sound server moves the recording there by itself (PipeWire, PulseAudio 15 or later).
- `linux/input_watcher.{h,cc}` (new, with libpulse): listens to the sound server for inputs
  added or removed and has `KeepRecordingDevice` run on the platform thread, where the
  audio device is used; libwebrtc tells nothing about them on Linux. It then sends Dart the
  package's `onDeviceChange` event too (`common/cpp/src/flutter_webrtc_base.cc`), as libwebrtc's
  own device change does elsewhere (in the same task, after `KeepRecordingDevice`), so `navigator.mediaDevices.ondevicechange` runs on Linux
  as well: the application tells the person when the microphone in use goes away and comes
  back. Elsewhere libwebrtc's
  own device change event runs it, on the platform thread too.

## The microphone's noise suppression (Linux, Windows, macOS)

Noise suppression is RNNoise (nnnoiseless, BSD-3-Clause, in the application's Rust
library: `crates/fogata-denoise`), run as the audio processing module's capture
post-processing: on WebRTC's audio thread, on every 10 ms of microphone sound, after echo
cancelling and gain control, at the rate WebRTC processes at (48 kHz as a rule; 16 and
32 kHz are resampled to 48 kHz and back). The switch flips a flag the processor reads, so
a change applies from the next 10 ms without opening the microphone again. The Rust
library hands over a table of C functions (`noise_suppressor()` in fogata-ffi's
`api/app.rs`, `DenoiseApi` in `denoise.rs`); the application passes its address with the
method `fogataNoiseSuppression` (`on`, `api`), which makes the processor on its first call
and sets the flag on every call, and answers false where it cannot run. The microphone
asks for WebRTC's own noise suppressor off. On Linux and Windows that reaches WebRTC's
audio processing only with Fogata's libwebrtc's wrapper patch 0013 (before it the
processing kept its default, that suppressor on at its highest level, whatever the
application asked; macOS's WebRTC took the options from its source all along). The
processing runs RNNoise, as its echo canceller, gain control and own suppressor, only
while its output is used, which with libwebrtc's patch 0008 means while any channel
sends sound (see "The microphone's processing while a screen's sound goes out").

- `common/cpp/include/fogata_denoise_table.h` (new; Linux, Windows) and
  `FogataDenoise.m` (macOS): the address is taken only inside a library loaded in the
  process, with both functions in that same library (`dladdr`, `GetModuleHandleExW`); any
  other is not read past finding where it lies, and nothing is called.
  `common/cpp/test/denoise_table_test.cc` (Linux, in `run_tests.sh`) tries a table in the
  test itself, on the heap, on the stack, one with a function of another library, another
  version and no address at all.
- `common/cpp/include/fogata_denoise.h` (new; Linux, Windows): `FogataDenoiser`, a
  `RTCAudioProcessing::CustomProcessing`. libwebrtc hands it the first channel's whole
  band (not split into bands), 10 ms at a time. A frame longer than the buffer libwebrtc
  says it hands, or than 10 ms at 48 kHz, goes on untouched (the macOS processor checks
  the same), and the Rust side takes only 10 ms at 16, 32 or 48 kHz, before it reads a
  sample.
- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_webrtc_base.cc`:
  `SuppressNoise` makes it and sets it as the capture post-processing. It is never freed:
  libwebrtc keeps a plain pointer to it and cannot be given none (it would call into a
  null pointer).
- `common/cpp/src/flutter_webrtc.cc`: the method.
- `macos/flutter_webrtc/Sources/flutter_webrtc/FogataDenoise.{h,m}` (new, macOS): the same
  as an `ExternalAudioProcessingDelegate` added to `AudioManager`'s capture
  post-processing adapter, one suppressor per channel (two at most).
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS only): the method.

## Sender parameters (Linux, Windows)

- `common/cpp/src/flutter_peerconnection.cc`: the degradation preference
  `maintain-framerate-and-resolution`, the name the Dart side sends (its `DISABLED` has
  none), was ignored, and `disabled` set the library's `DISABLED`, which it turns into
  libwebrtc's `BALANCED`. Both now set `MAINTAIN_FRAMERATE_AND_RESOLUTION`, which
  reaches libwebrtc as its `DISABLED`: the size and the frame rate stay whatever the
  network or the processor. macOS already took the name.

## Sound of a shared screen (Linux)

The package's loopback capture records everything the default output plays, so the
others in a call would hear their own voices back. Instead:

- `linux/pipewire_apps_capturer.{h,cc}` (new): a PipeWire input stream, not linked by the
  session manager, that links itself to every other application's output
  (`Stream/Output/Audio`), reading each output's owner from its full properties and from
  its client's process as the server saw it on the socket (`pipewire.sec.pid`, through
  the node's `client.id`), and leaves out this application's own (same process or same
  executable, without the ` (deleted)` the kernel adds after an update). An output is
  linked only once its process or executable is known, whose it is is decided again on
  every update of its properties, and one found to be this application's own loses the
  links already made (a call's sound stream made again when a viewer comes back had
  everyone hear themselves). In a Flatpak only the executable tells (`OwnOutput` in
  `linux/screen_sound_node.h`): every sandbox numbers its processes from 1, so another
  sandboxed program's stream could give this one's process number and be left out. It
  feeds 10 ms frames of 48 kHz stereo to the loopback
  audio source. A link it made is let go (`pw_proxy_destroy`) when either of its ports
  goes away. For tests, `FOGATA_SCREEN_SOUND_FROM` names the one program whose sound is
  taken (debug builds), judged once the output's executable is known; with `FOGATA_CALL_LOG` set each decision (node, process,
  executable, client's process, `node.link-group`, taken or not and why) goes to standard
  error as `[ScreenSound]`.
- `linux/screen_sound_node.h` (new): an output that passes on sound from elsewhere is
  never linked, as it may carry this application's own call played into a sink it reads
  from (the others then heard themselves): a loopback (`pw-loopback`, PipeWire's or
  PulseAudio's `module-loopback`), a combined output (`module-combine-sink`), a
  filter-chain or an effect host (EasyEffects). Told by the node's properties
  (`node.link-group`, `node.group`, `stream.is-loopback`, `node.virtual`, a `Filter`
  category or role) or by its program being the sound server's or an effect host's
  (`pipewire`, `pipewire-pulse`, `wireplumber`, `pw-loopback`, `easyeffects`,
  `pulseeffects`); once so, always so, and links already made go. Programs' own streams
  (browsers, games through Wine or Proton, media players, Flatpak apps) carry none of
  these: through pipewire-pulse their program is their own, as libpulse names it. Tested
  by `common/cpp/test/screen_sound_node_test.cc`; in the call lab with
  `CALL_LAB_LOOPBACK=1` (`tools/call-lab/lab.sh`), and by `tools/call-lab/mic.sh echo`,
  which measures whether someone who leaves the room and comes back hears their own voice
  in a screen shared with sound.
- `common/cpp/test/screen_sound_capture_test.cc` and `silent_input_test.cc` (in
  `run_tests.sh`, with ThreadSanitizer): the capture's and the silent input's own code
  against a PipeWire server the test plays itself (`pipewire_fake.{h,cc}`, linked in place of
  libpipewire; no daemon is reached): which outputs are linked as the server tells of them
  in any order (the call's own sound made again during a share never is), requests carried
  out in the order asked whatever the server's delays, and PipeWire's thread against the
  platform thread stopping the capture.
- `linux/loopback_capturer_factory.cc`: uses it when built with PipeWire, and only for a
  whole screen: a window's sound cannot be narrowed to its application here, and every
  other one's would go with it. Without PipeWire, or where it cannot connect (PulseAudio
  itself is the sound server), a shared screen has no sound: the package's PulseAudio
  monitor capture (`linux/pulse_loopback_capturer.{h,cc}`, removed) carried this application's own call sound back to everyone,
  and its stop could block the platform thread.
- `common/cpp/src/flutter_webrtc.cc`, `linux/pipewire_apps_capturer.{h,cc}`: the method
  `fogataScreenSoundServer` answers "pipewire" when the build has PipeWire and its server
  takes a connection (`PipeWireAppsCapturer::Reachable`, which connects and lets go at
  once, on a thread of its own), else "none", so the application can say why a shared
  screen goes without sound.
- `common/cpp/src/flutter_screen_capture.cc`: the portal path (Wayland) captures this
  sound too when `getDisplayMedia` asks for audio and the portal says a whole screen was
  chosen (`source_type`, `PortalScreenCapturer::SharesScreen`; a portal older than
  version 3 only when it offered no windows), and stops it with the picture.
- `linux/silent_input.{h,cc}` (new) and the method `fogataSilentInput` in
  `common/cpp/src/flutter_webrtc.cc`: a source of silence, "Fogata silence (<pid>)"
  (node `fogata-silence-<pid>`): a null sink that PipeWire runs as a virtual source,
  owned by the app's connection so it goes away with the app. Named after the process,
  so the application picks its own and never another running Fogata's, which goes away
  with that one. `fogataSilentInput` answers true only once the server has made the
  node (a sync after the request, 2 s at most); a PipeWire without the null sink factory
  answers false at once. It runs on a thread of its own, one request after the other in
  the order asked and each answered before the next starts (`SilentInput::Ask`, with
  `common/cpp/include/fogata_turns.h`), so a stalled server does not freeze the window. A connection lost
  (PipeWire restarted, as after an update) is noticed by the core's error, and the next
  `fogataSilentInput` makes the node again on a new one; it answered true at once for the
  node gone with the old connection. The application asks for it again when the list of
  devices no longer has it while it is in use ("A sound server that goes away and comes
  back").
  WebRTC records from its input whenever any channel of sound sends; while the
  microphone is off and a shared screen's sound goes out, the application points WebRTC
  here so the microphone stays closed. Three choices of input go one after the other,
  each waiting for the one before it: this one, the way back to the microphone, and
  another microphone chosen in the settings during a call (on every system). The calls
  made there have no time limit: on Linux and Windows the list of devices and the
  selections run on the platform thread, so one that does not return holds every later
  one there as well, and giving up on it would only leave the application believing
  another input chosen than the one that is (a silent input removed while it works, or
  kept while it is gone); the choices wait, and are then carried out in order. Two
  choose outside that order: a call's start, and the microphone opened again at an
  unmute, which names its input to `getUserMedia` (`sourceId`), where the plugin selects
  it (Linux, Windows). Known limits, as before: a microphone chosen in the settings while
  a slow start still chooses can be replaced by the start's own choice, made later; and
  a call's end waits for that order 5 s at most (its steps of letting go), so an ended
  call's last choice can land after the next call has started.
- Both connect to PipeWire's own socket, which a Flatpak gets only when given it
  (`--filesystem=xdg-run/pipewire-0`, in tools/beta/flatpak.sh): with only PulseAudio's,
  a shared screen goes without sound and the silent input is not made.

## The microphone's processing while a screen's sound goes out (Linux, Windows)

WebRTC has one audio processing module for what the microphone records, the voice
engine's. A shared screen's sound never goes through it (it comes from a custom source
of its own). Measured in the call lab (`tools/call-lab/mic.sh echo`, with WebRTC's log),
the sharer's voice went out 10 to 12 dB weaker while its screen's sound went out to a
watcher, and traces of the others' voices came back for a while. Three things, each
fixed in Fogata's libwebrtc:

- The processing did nothing at all for most of a call. WebRTC tells it that its output
  will be muted when a channel's send streams are all muted
  (`WebRtcVoiceSendChannel::MuteStream`, `set_output_will_be_muted`), and it then passes
  the microphone through untouched (`minimize_processing_for_unused_output`): no echo
  cancelling, no noise suppression, no gain control, no RNNoise. With unified plan every
  audio section has a channel of its own (the voice, the screen's sound, one for each
  connection), so the channel touched last decided for all of them: the application
  makes a connection again and closes the old one at every entry into a room, and the old
  one's voice sender stopping turned the processing off while the new one sent the
  microphone. It came on again only when some sender got a track (the screen's sound
  starting to go out to a watcher), which was the level change measured: WebRTC's noise
  suppressor takes the lab's steady tones for noise. `tools/webrtc/patches/webrtc/0008`:
  the engine keeps the send streams that send sound, of every channel, and the output
  counts as muted only while there is none. macOS's WebRTC (webrtc-sdk's) never takes a
  sender with no track for muted, and keeps its own rule (`patches/apple/0001`).
- The application's settings never reached the processing. In M156 a channel's options
  are only kept; WebRTC's own factory sets them as global options when it makes an audio
  source, which the wrapper did not. `tools/webrtc/patches/wrapper/0013`: a microphone
  source's options (echo cancelling and gain control on, WebRTC's noise suppressor and
  high-pass filter off, as `getUserMedia` passes them) set the processing; a custom
  source's (the screen's sound) never do. `0015`: the processing gets the factory's
  field trials too.
- The processing was initialized again each time a screen's sound started or stopped
  going out in stereo: WebRTC processed the microphone in the most channels any channel
  sends (`AudioState::UpdateAudioTransportWithSendingStreams`,
  `AudioTransportImpl::RecordedDataIsAvailable`,
  `AudioProcessingImpl::MaybeInitializeCapture`), and the echo canceller and gain control
  started from nothing. `tools/webrtc/patches/wrapper/0011`: the microphone is processed
  in one channel always; the screen's sound stays stereo. Tested by the wrapper's
  `test/custom_audio_transport.test.cc`.
- `common/cpp/src/flutter_screen_capture.cc`, `common/cpp/include/flutter_screen_capture.h`:
  `getDisplayMedia` makes the screen's sound source with the processing its `audio` map
  names (`echoCancellation`, `autoGainControl`, `noiseSuppression`; off for any left out,
  and with `audio: true`). That was meant to keep the microphone's settings while the
  screen's sound went out (the log line `aec: 0, agc: 0` it answered was a channel's kept
  options, never what the processing did); with 0013 a custom source's options no longer
  touch the processing at all.

## An echo that comes earlier (Linux, Windows)

Now and then a whole turn of the other person's voice (about a second) went back to them
in the middle of a call. Measured in the call lab (`tools/call-lab/mic.sh echo` with
`ECHO_STALL`, which stops the lab's PipeWire now and then, and the echo timed in the
samples each copy wrote for playout and read from its microphone): when the sound server
misses its deadlines it leaves out some of what was played or recorded, and the echo
reaches the microphone earlier than the echo canceller has it. AEC3 keeps 2 ms of room for
that (`delay_headroom_samples`, 32 at 16 kHz): an echo even a few ms earlier was out of
its linear filter's reach until its delay estimator found it again, a second or more
later. A lost cycle is about 10 ms at a quantum of 480 or 512 and 21 ms at 1024.
`tools/webrtc/patches/wrapper/0019` gives it 512 samples (32 ms); the linear filters keep
WebRTC's lengths (13 blocks, 12 in the initial state used for the first 2.5 s of the other
side's sound after every reset), which with the echo 8 blocks in reach 5 blocks past it
(4 in the initial state). After a reset AEC3 still aligns from the device's delay with 2
blocks of room until it finds the delay; keeping the 8 there too, and having legacy
transparent mode judge a filter sane from where the room puts the echo, were tried and
made a call's start worse (the next section). As first written, the patch also made the
filters 8 blocks longer, so that they reached as far after the echo as before; the tables
below were measured that way, and the cost it had at a call's start, and why it went, are
at the end of this section. Measured with no patch, 16, 32 and 48 ms of room (turns let
through):

| | none | 16 ms | 32 ms | 48 ms |
|---|---|---|---|---|
| after a move of up to 16 ms | 30 of 68 | 2 of 66 | 4 of 73 | 9 of 63 |
| after a move of 16 to 32 ms | 26 of 79 | 15 of 78 | 4 of 71 | 11 of 66 |
| after a move of over 32 ms | 13 of 53 | 9 of 53 | 8 of 52 | 10 of 51 |
| quantum free, stalls of 20 and 40 ms | 25 of 131 | 5 of 132 | 5 of 132 | 14 of 129 |
| quantum 480, stalls of 40 and 80 ms | 19 of 128 | 14 of 130 | 7 of 131 | 13 of 130 |
| no stalls | 0 of 363 | 2 of 193 | 0 of 189 | 4 of 98 |
| a call's first turn, median under one's own voice | 76 dB | 75 dB | 76 dB | 60 dB |

With the initial-state filters 8 blocks longer as well, 32 ms helped as much against the
stalls but a call's first turn went out less cancelled (57 dB at the median, once only
18 dB): the longer filters converge more slowly. As patched, later turns are as before
(72 dB at the median, 73 without) and WebRTC's audio threads use the same processor time
(2.88 % of a core with it and without). 48 ms let turns through even without stalls.
Moves larger than the room (a stall of a tenth of a second or more) still can. On macOS
the echo is cancelled by WebRTC's own AEC3 too, not by Apple's voice processing: the
application bypasses it (`bypassVoiceProcessing`, `lib/main.dart`), and with WebRTC's
noise suppression asked off its options rule the platform path out. That AEC3 is made by
WebRTC-SDK's `RTCDefaultAudioProcessingModule` (`AudioManager.m`) with stock settings: the
wrapper's patch 0019 does not reach it, so its headroom stays at 2 ms. Whether a Mac's audio engine moves the echo as PipeWire does
has not been measured.

Measured again in one session with WebRTC patches 0010 to 0012, interleaved with the
library as published and with every echo controller restarted (turns let through):

| | as published | with 0019 | every one restarted |
|---|---|---|---|
| quantum free, stalls of 20 and 40 ms | 35 of 137 | 10 of 136 | 9 of 136 |
| quantum 480, stalls of 40 and 80 ms | 28 of 109 | 4 of 110 | 1 of 110 |

As it is now (32 ms, WebRTC's filters), in one session of 6 runs each with WebRTC's own
configuration, the filters 8 blocks longer, and 16 ms with them longer (turns let through):

| | WebRTC's configuration | 32 ms, WebRTC's filters | 32 ms, filters +8 | 16 ms, filters +8 |
|---|---|---|---|---|
| quantum free, stalls of 20 and 40 ms | 51 of 111 | 4 of 159 | 7 of 154 | 8 of 151 |
| quantum 480, stalls of 40 and 80 ms | 39 of 121 | 11 of 148 | 12 of 151 | 21 of 141 |

The longer filters converged more slowly at a call's start. With a synthesised voice on
the other side that does not repeat (`mic.sh unmute` with `UNMUTE_FAR_FILE`), a call's start
with speakers sent it back for 2.7 s at the median (0 to 7.6) at -49 dBFS at the loudest,
against 0 s and -66 dBFS with WebRTC's configuration (10 runs each, one session, p 0.008
for both). In another session (10 runs each): 32 ms with WebRTC's filters 0 s at the median
(0 to 1.4) at -54 dBFS, against 0 s (0 to 7.7) at -56 dBFS with the library before the
echo controller was restarted for an unmute (p 1.0 and 0.34), 0 s at -60 dBFS with WebRTC's
configuration, and 6.8 s at -46 dBFS with 16 ms and the filters longer (p 0.005 and 0.019);
the initial-state filters lengthened as well were worse still (5.8 s at -35 dBFS, 6 runs).
Its unmutes with speakers come back 0.3 s at the median (1.6 s with the library before the
restart for an unmute, which is what shortens them). After each of one's own words, with a
steady tone on the other side (`unmute:speakers`), the tone comes back for up to 0.25 s as
with WebRTC's configuration (-55 against -58 dBFS at the median of each unmute's worst,
p 0.28; -47 with the filters longer): AEC3 holds its gentler suppression for 200 ms after
one's own voice (`dominant_nearend_detection.hold_duration`, 50 blocks).

With the filters 8 blocks longer, a known cost was a far side that repeats itself exactly
(not measured again with WebRTC's filters). The unmute check's
own far side (`UNMUTE_FAR=words`: the same 2.1 s phrase every 3 s, three words of a sawtooth
whose pitch falls within each syllable between 160 and 106 Hz) came back at a call's start
with speakers for 4.8 s at the median (0 to 8.9), at -40 dBFS at the loudest, where AEC3
with WebRTC's 2 ms took 0.9 s (0 to 3.7) at -52 dBFS (8 runs each, p 0.05; the same with
AEC3 restarted rather than made anew). The logs of the lab show the same delay found in
the same time and the filters converged as soon; why the signal that repeats comes back
longer is not known. With speech-like noise that repeats only every 30 s there is no
difference. The synthesised voice's call start, within the runs' scatter in an earlier
session of 8 runs each (p 0.06 and 0.22), was the longer filters' cost given above.

## A mute silences every sender at once (Linux, Windows, macOS)

Closing the microphone's track (`trackDispose`, `flutter_webrtc.cc` and
`FlutterWebRTCPlugin.m`) only lets go of it: every sender holding it goes on sending the
microphone. A mute therefore silences the senders first, as it is asked, before any
connection's own step:

- Linux and Windows: the track is disabled (`mediaStreamTrackSetEnable`), and every
  sender holding it mutes its stream (`AudioRtpSender::OnChanged`, `SetAudioSend` with the
  track disabled, `WebRtcVoiceSendChannel::MuteStream`), which here only tells the audio
  processing that its output is unused (`tools/webrtc/patches/webrtc/0008`).
- macOS: the track is taken off every sender (`rtpSenderReplaceTrack` with none), as the
  call always did. Disabling it would have WebRTC-SDK's `MuteStream` (its rule replaces
  0008's, `tools/webrtc/patches/apple/0001`) call the audio module's `SetMicrophoneMute`,
  which with voice processing on mutes Apple's voice processing input (a mode the
  plugin's own documentation, `lib/src/helper.dart`'s `setMicrophoneMuteMode`, says plays
  the platform's mute sound; not measured here), a way the call never took; a sender
  with no track skips it. The
  microphone's track is never disabled on macOS while a sender holds it.

## A microphone turned on again while the other side talks (Linux, Windows, macOS)

Turning the microphone off stops the voice channel's send stream (its encoding stopped,
or the channel made to only receive, see `docs/architecture/4-voice-and-video.md`), which
stops the audio device's recording once no other channel of sound sends: on Linux, while
a shared screen's sound goes out, recording goes on from the silent input, and from the
microphone for the moment (about 3 s) the silent input takes to be chosen. Turning it
on starts it again, and patch 0009 made
AEC3 anew there, as at a call's start. A new AEC3 does not know whether the room has an
echo. With a headset there is none, and AEC3 lets the microphone through untouched while
the other side talks only once its transparent mode has seen 6 s of their sound without
echo; until then its suppressor takes the microphone for echo whenever their sound
plays. Measured in the call lab (`tools/call-lab/mic.sh unmute`, the other side holding
a tone, three runs of three unmutes each): the first words after an unmute went out 8 dB
weaker on average (17 dB at worst) and the room's noise 12 dB lower, for 3 to 6 s, and
then came back at once.
`tools/webrtc/patches/webrtc/0010` keeps the transparent mode when recording starts again:
AEC3 still drops what it aligned of the played sound with the recorded one (what 0009 was
for), but not what it knows of the room. The WebRTC patches go into macOS's framework too,
and there muting also stops the voice channel's send stream and AEC3 does the cancelling,
so 0009 made it anew at every unmute on a Mac as well and 0010 applies there (measured on
Linux only).

| unmute, the other side holding a tone | before | after |
|---|---|---|
| headset: first word, against the later ones | -8 dB (-17 at worst) | 0 dB |
| headset: noise in the first 0.5 s, against later | -12 dB | 0 dB |
| speakers: noise in the first 0.5 s, against later | -8 dB | -2 dB |
| speakers: the other's tone sent back between the first words (median) | -59 dBFS | -61 dBFS |

Only a microphone turned on again in the call it was turned off in (the same people heard
all along) keeps the room; a call's start makes AEC3 anew as 0009 did, and so does an
unmute after a change of audio devices (`tools/webrtc/patches/webrtc/0011`: a device
plugged in or out, the default or a port changed, a stream moved, a device chosen; each
platform's device module marks it). Restarted, AEC3 processes nothing before fresh played
sound arrives, as a new one. Measured in one session of the call lab, everything
interleaved (`tools/call-lab/mic.sh unmute`, the other side talking in speech-like noise
that repeats only every 30 s; with speakers unless said; 8 or 6 runs, medians and ranges; "heard":
how long their voice came back to them above -50 dBFS; p of a rank test against 0009):

| | 0009 (published) | now | p |
|---|---|---|---|
| a call's start: loudest sent back | -68 dBFS (-83 to -42) | -70 dBFS (-83 to -40) | 0.96 |
| a call's start: heard | 0 s (0 to 1.5) | 0 s (0 to 1.6) | 0.47 |
| an unmute in the call (16): heard | 0.3 s (0 to 0.5) | 0.4 s (0 to 1.0) | 0.28 |
| a second call in the same process: heard | 0.3 s (0.3 to 0.4) | 0.4 s (0.3 to 2.8) | 0.44 |
| a headset unplugged while muted: heard | 0.3 s (0.2 to 0.3) | 0.3 s (0.2 to 0.3) | 1.0 |
| a headset unplugged while muted: loudest | -24 dBFS (-27 to -19) | -25 dBFS (-31 to -17) | 0.59 |
| headset to speakers while muted, no device event: heard | 0.2 s (0 to 0.3) | 1.1 s (0.3 to 2.2) | 0.01 |
| the same: loudest | -32 dBFS (-54 to -27) | -15 dBFS (-29 to -11) | 0.006 |

The last two rows are what keeping the room costs: when the room changes and no device
says so (a headset taken off but still plugged in and laid by the microphone, a single
output that serves headphones and speakers with nothing to tell them apart), the
transparent mode kept says there is no echo, and the other side hears themselves for about
a second after the unmute until AEC3 finds it. The same echo added with the microphone on
(4 runs each, another session) came back for 0.5 s at -16 dBFS, with 0009 alike (0.4 s;
p 0.29 and 0.88): the unmute that keeps the room takes about half a second longer. Not seen as a change: a Bluetooth headset on Linux that
switches profile as recording stops and starts removes and adds its devices, so its
unmutes most likely still get a new AEC3, as with 0009; on macOS, an output switching its
data source on the same device. The unmute check with a headset (`mic.sh unmute:headset`,
6 runs) and with speakers (3 runs) passed; the other side talking over one's words cut as
much of them as with 0009, and later in a call less (16 % of each word against 37 %, 4
runs). Forcing an echo path change at the first delay found after the restart, keeping
wrapper patch 0019's headroom after a reset, judging the transparent mode's sane filter
from that headroom and making AEC3's render buffer anew at the restart were tried and left
out (patch 0010's header).

## Sound of a shared screen (macOS)

macOS's WebRTC hands what its audio module records to every channel of sound that sends,
so a screen's sound on a channel of the call's connections would carry the voice too.
Instead it goes on connections of its own, made by a second factory:

- `macos/flutter_webrtc/Sources/flutter_webrtc/FogataScreenSound.{h,m}` (new, macOS 13):
  takes every application's sound but this one's with ScreenCaptureKit (the filter names
  applications, none left out: one made of windows brings no sound; the picture is not
  used), mixes it into one channel at 48 kHz and keeps at most 150 ms of it. Its factory's
  audio
  module renders by itself every 10 ms, without any device (manual rendering), from a
  source node reading that sound: it never opens the microphone or the speakers. Echo
  cancelling, noise removal and gain are off. If manual rendering cannot be set, nothing
  is sent. A stop while a start is under way stops the capture as soon as it has started,
  and that start answers false.
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS only): `createPeerConnection`
  with `fogataScreenSound: true` in its configuration makes the connection with that
  factory and adds a channel sending the sound; the method `fogataScreenSound` (`on`,
  and `window`, a shared window's number) starts or stops taking it and answers whether
  it is taken.
- For a shared window, `startForWindow:completion:` takes only the sound of the
  application the window belongs to (found by its number among ScreenCaptureKit's
  windows), with its helpers: applications whose bundle id starts with the owner's
  followed by a dot (Chrome's and Electron's helpers), and for Safari WebKit's
  processes (`com.apple.WebKit.*`), which play its pages' sound for every application
  that shows web pages. A window that cannot be found takes nothing. The filter is a
  display's, including those applications (one made of windows brings no sound). Another
  window asked for while a start is under way follows it; asked for while one runs, it
  replaces it. Each capture's last line in the log says how many blocks of sound it took
  and the loudest sample.

## A screen's sound goes with its capture (Linux X11, Windows)

`getDisplayMedia` outside the portal kept one loopback capture, stopped by the end of
whichever desktop capture ended: a screen captured again (another frame rate) starts the
new capture, with its sound, before the old one stops, and the old one's end stopped the
new sound. The sound was also started before the source was found and its capturer made,
and kept going when either failed.

- `common/cpp/src/flutter_screen_capture.cc`, `common/cpp/include/flutter_screen_capture.h`:
  the source and its capturer are found and made first; the loopback capture remembers the
  desktop capturer it was started with (`loopback_owner_`), and only that one's
  `OnStop` or `OnError` stops it (`StopSoundOf`).
- `common/cpp/include/loopback_capturer.h`, `linux/loopback_capturer_factory.cc`,
  `windows/loopback_capturer_factory.cc`: `CreateLoopbackCapturer` is told whether the
  source is a window.
- A capture that fails once started (a shared window that closed) sends
  `mediaStreamTrackEnded` for its video track, as the portal's end does, so the share
  stops instead of leaving those who watch a frozen picture; it only stopped the sound
  before. `OnStop` and `OnError` come on libwebrtc's signalling thread (the capturer's
  `BlockingCall` from its capture thread): their work is posted
  to the platform thread, which alone touches the loopback members and the capture's
  track (`capture_tracks_`), and holds the capturer so that another made meanwhile at its
  address is not taken for it. Tested with ThreadSanitizer by
  `common/cpp/test/screen_capture_end_test.cc` (the screen capture's own code, libwebrtc's
  objects the test's).

## Sound of a shared window (Windows)

The package's `ApplicationLoopbackCapturer` takes, for a shared window (the id it is listed
by is its window handle), the sound of the window's process and its children only
(`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`). Asked for in the render endpoint's
floating-point format, that capture started and never got a packet, so a shared window
went with no sound at all.

- `windows/application_loopback_capturer.cc`: one process's capture asks for plain 16-bit
  PCM at 48 kHz, two channels; a whole screen's keeps the endpoint's format. Both channels
  go to the source as they came (the package mixed them into one), as on Linux: a call
  that agreed on stereo for the screen's sound encodes both.
- `windows/application_loopback_capturer.cc`: the activation's answer sets an event of
  the handler's own, and the handler and the opening thread decide by an atomic exchange
  which came second: an answer after the 5 s the opening waits lets go of the client it
  brings, and the opening keeps its references to a handler it gave up on (one small
  object and one event each time), so a late answer never sets a closed handle nor one
  whose value Windows gave to another object. The package closed the event while its
  worker could still set it. The start no longer runs on the platform thread (below).
- `windows/loopback_capturer_factory.cc`: a window whose process cannot be found (closed
  meanwhile, an id that is no window) gets no sound at all; the package took the whole
  system's sound then.
- `windows/test/loopback_test.cc` and `run_tests.ps1` (new): build with Visual Studio's
  tools against the libwebrtc a Windows build unpacks, find a window by its title, check
  the plugin lists it by its handle, and take its sound while another program plays
  (none may come) and once its own program plays too (it must); whoever runs it starts
  those programs, playing into a virtual output.

## A screen's sound started off the platform thread (Windows)

The package started a share's sound on the platform thread, inside `getDisplayMedia`: the
activation (waited for 7 s, then for as long as the audio service took), the client's
set-up and its start; and stopped it there too. A slow or stalled audio service froze the
window as a share with sound started or stopped.

- `common/cpp/include/loopback_capturer.h`, `windows/application_loopback_capturer.{h,cc}`:
  `Open` (the activation, the client's set-up and start, in the multithreaded apartment
  for the while) and `Close` (the client's stop and release) are the parts that may wait
  on the audio service. `Start` after `Open` only starts the threads that read and feed;
  `Quiet` only ends them (the reading one is woken at once, the feeding one within 10 ms)
  and lets go of the source. `Start` without `Open`, and `Stop`, do everything, as before
  (`windows/test/loopback_test.cc`).
- `common/cpp/include/fogata_sound_start.h` (new): `SoundStart` runs `Open` on a thread
  of its own and gives up at a limit. It ends as opened, failed, timed out or cancelled,
  told once on the platform thread through a `PlatformQueue` (which refuses once its
  owner went), unless the platform thread cancelled it first. What opens too late or
  is no longer wanted is closed off the platform thread and never handed over; an opened
  sound nobody took (the plugin went) is closed when the start goes. Tested with
  ThreadSanitizer by `common/cpp/test/sound_start_test.cc` (in `run_tests.sh`), with an
  audio service that answers at once, late or never.
- `common/cpp/src/flutter_screen_capture.cc`, `common/cpp/include/flutter_screen_capture.h`:
  `getDisplayMedia` starts the picture as before and returns. The answer is the same as
  before, given once the sound is known: with the sound's track, or with none when it
  could not start or took over 7 s (the activation's own 5 s and 2 s for the set-up, the
  platform thread's wait before). Meanwhile, a second share with sound has the first one
  answered without its sound, as a share with sound stopped a running one before; the
  picture's capture ending (`OnStop`, `OnError`) answers the error of a capture that does
  not start, keeping no stream nor track. A running sound stops with `Quiet` on the
  platform thread and `Close` on a thread of its own. When the plugin goes, a sound still
  starting answers nobody and is closed once it opens; a running one stops feeding before
  libwebrtc ends. For tests, `FOGATA_TEST_SOUND_STALL=<milliseconds>` (debug builds)
  holds each start that long first, as a stalled audio service would.
- `windows/flutter_webrtc_plugin.cc`: the task runner is made before the plugin's
  `FlutterWebRTC` and goes after it, as on Linux (it went first), so the sound's queue
  closes before the runner goes. `windows/task_runner_windows.cc`: each task runs with
  the queue let go of; it ran under the queue's lock, so a task that queued another (a
  capture's end, which sends an event) locked it again.

## Window sharing (macOS)

The package shared a window with WebRTC's own capturer, which takes one still picture
after another: on recent macOS about 5 a second. On macOS 13 and later a window goes
through ScreenCaptureKit like a display, at the asked frame rate:

- `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterScreenCaptureKitCapturer.{h,m}`:
  `startCaptureWithFPS:sourceId:window:onStarted:` captures the window whose number is the
  source id, alone (`initWithDesktopIndependentWindow:`), at its size in pixels (on macOS
  13, by the scale of the screen that shows most of it), and follows that size from the
  content rectangle and scales each frame carries when the window is resized or moves to
  a screen of another scale.
- `FlutterRTCDesktopCapturer.m` (both copies, identical): uses it for windows.

## No screen captured larger than the others take (macOS)

Every receiver drops a picture larger than 8K (see "Pictures too large to draw"), and the
application keeps out of the call whoever sends one. A window captured at its size in
pixels can be larger than that (a window stretched across two 5K screens), and so can a
display.

- `macos/flutter_webrtc/Sources/flutter_webrtc/FogataCaptureSize.h` (new):
  `FogataCaptureSize` gives the size to capture at: the same shape, made smaller only as far
  as 8K takes it (7680 by 4320 pixels, no side over 8192, the limits of
  `lib/src/call/fitting.dart` and of the renderers), in even numbers, at least 2 by 2.
- `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterScreenCaptureKitCapturer.m`: a
  display's and a window's capture, and a window's new size when it is resized, are set
  through it; the package set a display's capture to the display's own size.
- `macos/test/capture_size_test.c` (new): its cases (a laptop's screen as it is, odd sizes
  made even, 8K as it is, a window wider than any side, two 5K screens, no size at all);
  `tools/check.sh` builds and runs it on a Mac.

## Fogata's own windows are not shared (macOS, Windows)

Shared, Fogata's window showed the share inside itself, captured again with it, and so on
without end: every frame changed and the share went out at a few frames a second.

- `FlutterRTCDesktopCapturer.m` (both copies, identical): `getDesktopSources` leaves out
  the windows whose number is one of the application's own (`NSApp.windows`).
- `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterScreenCaptureKitCapturer.m`: a
  display is captured without the application's own windows
  (`excludingApplications:`), those opened after the capture started too.
- `common/cpp/src/flutter_screen_capture.cc` (Windows): `GetDesktopSources` leaves out
  the windows (their source id is the HWND) that this process owns. Under X11 the list
  says nothing of a window's process; the app leaves Fogata's window out by its title.

## A screen capture that does not start, or ends by itself (macOS)

- `FlutterRTCDesktopCapturer.m` (both copies, identical): `getDisplayMedia` through
  ScreenCaptureKit answers once the capture runs, or with ScreenCaptureKit's error (the
  screen recording permission refused, the window already gone) and no track, instead of
  a live track that never gets a frame.
- `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterScreenCaptureKitCapturer.{h,m}`: is
  the stream's delegate; a running capture that stops by itself (`didStopWithError:`: the
  shared window closed, the system's own "stop sharing") or delivers a frame whose status
  is "stopped" runs `onEnded` once, never after a stop asked for here. The desktop
  capturer then sends `mediaStreamTrackEnded` for the track, as on Linux, and the track's
  `onEnded` runs.

## Other sounds during a call (macOS)

While Apple's voice processing runs, macOS lowers every other sound the computer plays,
the application's own short sounds included, for as long as the call lasts.

- `FlutterWebRTCPlugin.m` (both copies, identical; macOS only): once the factory exists,
  turns on the audio module's advanced ducking (other sounds are lowered only while
  someone speaks) when the module has that setting, found by name
  (`advancedDuckingEnabled` or `isAdvancedDuckingEnabled`); without it nothing changes.
  The log says which: "Fogata: advanced ducking on" or "... not offered by this audio
  module".

## The others' voices play without a microphone (macOS)

WebRTC-SDK's AudioEngine audio module plays and records with one AVAudioEngine. Asked to
record while it plays (WebRTC does as soon as a channel of sound sends), it stops the engine
to add the input, and when the input then cannot be opened (no input device, as on a Mac
mini without one or before an iPhone's microphone is there; no format yet; the microphone
permission not granted) it rolls back without starting the engine again and without
changing its state: it still counts itself as playing, so asking it to play does nothing,
and nobody's voice is played (`totalSamplesReceived` 0 while packets arrive) until
something makes the engine again.

- `macos/flutter_webrtc/Sources/flutter_webrtc/FogataPlayout.{h,m}` (new, macOS):
  - Tells the module its input is not available (`setEngineAvailability:`) while it
    could not be opened: no permission granted, no input device (the module's first input,
    "default", has an empty name then), or one that is gone, has no input channels or no
    rate. The module then plays alone and keeps WebRTC's requests to record for when the
    input is available. The input is looked at again when the system's devices or its
    default input or output change (after 600 ms of quiet, past the module's own 500 ms; a
    change of the list that is only its own aggregates made or destroyed is not one), when
    `getUserMedia` takes a microphone and when one is chosen (`FlutterRTCMediaStream.m`,
    both copies, identical); a microphone that became usable is opened at once.
  - As the module's observer (unless an embedding plugin registered one): when the engine
    stops while playout goes on, it looks once the module's change is done, and again
    500 ms later; an engine still stopped while the module counts itself as playing (or,
    with devices chosen, as recording with its input available: a call where nobody else
    sends sound only records, and in Device mode the module's output follows its input)
    is released and made again (three times in a row at most; an engine made again that
    then ran 5 s worked, and a later stop starts the count again). It is made as it was
    first, the input in it if it was: an engine stops with its input in it as well when
    the system switches devices, and the microphone had no part in that. Taking every
    such stop for a
    microphone that cannot be opened lost the microphone at each change of output, until
    the system's devices changed. Only when the engine made again with the input does not
    run, or stops again at once, is it made with the speaker alone; the input is then
    tried again by itself after 2, 10 and 30 s (the first before the call's own check of a
    microphone that reads nothing, 8 s), and at each change above, of the default output
    and of the output chosen; three such tries at most until one of those changes, each
    costing at most two quick remakes when the input still fails. When every engine that
    stopped was only to play, the input was not the cause (a speaker that went to sleep
    or changed its profile): it is made again as it was. Without the observer (another
    one was set), no stop is heard, and the input is taken for the cause once the engine
    made as it was did not run.
  - Gives the engine the devices chosen: see "The devices chosen, with the microphone"
    below.
  - Its work runs on a queue of its own: the module's getters (`isPlaying`,
    `isEngineRunning`) and setters wait for its thread (WebRTC-SDK's `RTCAudioDeviceModule`
    calls the module with `BlockingCall` on the factory's network thread, which is also
    WebRTC's worker thread, where every change of the engine runs from start to end), up
    to two seconds while an engine starts. So the check 500 ms after a stop never sees a
    start half done. `stop` waits for the queue before the factory goes.
  - The log says "Fogata: microphone not usable, playing only: ...", "microphone usable",
    "microphone could not be opened", "the audio engine stopped while playing; made
    again without the microphone" (or "as it was") and "trying the microphone again (n of
    3)".
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS only): made with the factory,
  stopped in `dealloc`.
- A headless check outside the app (the module of `app/macos/WebRTC-SDK.podspec`, its
  input refused through the observer, platform voice processing off so no microphone is
  opened): alone, the module goes on counting itself as playing with its engine stopped,
  and `startPlayout` changes nothing; with this, the engine runs again within a second,
  recording asked later leaves it running, and a microphone that fails again is handled
  the same way.

## The devices chosen, with the microphone (macOS)

The module's engine runs without Apple's voice processing: the microphone asks WebRTC's
own noise suppressor off (Fogata runs RNNoise, "The microphone's noise suppression"), and
with that the module's audio processing options rule the platform path out, so the echo
is cancelled by WebRTC's AEC3 and the engine's state says `vp=0`. Without voice
processing an AVAudioEngine on macOS plays and records through one audio unit (AUHAL),
which is given one device (`kAudioOutputUnitProperty_CurrentDevice`); left alone it
follows the system's default input and output, putting the two together itself when they
are different devices. The module gave that unit the device chosen whenever it made its
engine (WebRTC-SDK's `audio_engine_device.mm`, the step "Configure device"), the output
chosen even while the engine recorded. A unit with its input enabled refuses a device
without an input (CoreAudio's `kAudioUnitErr_InvalidPropertyValue`, -10851): any output
that is not the microphone's own device. The module then rolled back what it had made in that change, but it had already
stopped and released the engine before, and it keeps its state as before the change: it
counts itself as recording with no engine, nothing makes one again, and choosing the first
output again is no change for it (its device is already that one). The microphone sent
nothing from the change on, and 8 s later the call said it could not be used and turned
it off. The input chosen while the output was another device met the same.

- `FogataPlayout.{h,m}`: as the module's observer it gives the engine the devices chosen
  itself, and the module is kept on the system's devices. An AVAudioEngine whose unit was
  never given a device follows the system's default input and output, whatever their
  rates (as every call did before); a unit once given a device cannot be put back to
  following, only a new engine follows again, and the module makes a new one whenever its
  input and output are both made unavailable and then available again.
  - `selectAudioOutput` and `selectAudioInput` (`FlutterRTCMediaStream.m`, both copies,
    identical) tell it (`outputChosen:name:`, `inputChosen:name:`, with the name the list
    gave) instead of the module. On its queue the module is told the device only with
    nothing enabled (`setEngineAvailability:` neither input nor output) and back to the
    system's at once: it marks the change for the echo canceller (Apple patch 0008, WebRTC
    patch 0011) and never gives a device to an engine itself. Its engine, if it had one, is
    made again then. A device chosen, the same one again included, is always tried so, and
    what the engine refused before is forgotten; only the system's default chosen while it
    already was changes nothing (a call's start), so a call with nothing chosen goes as
    before.
  - R1. One question decides: can the pair of the output chosen (else the system's) and
    the microphone the call uses (the one chosen, else the system's), recording or not,
    be put together? From the devices' properties only, nothing opened: the same device,
    or two at the same nominal rate (an aggregate cannot resample), not refused by the
    engine in this call.
  - R2. If not, the engine is never given a device: it follows the system's devices, muted
    or not, as with nothing chosen.
  - R3. If so, in the module's `willEnableEngine` (before it reads the formats of the
    engine's input and output) and `willStartEngine`, the engine's unit is given the output
    chosen alone while the engine does not record (with only a microphone chosen, nothing),
    and the pair while it does: the one device, or a private aggregate
    (`AudioHardwareCreateAggregateDevice`, listed in this process only, its unique id
    starting with `fogata.route.`): the output's clock, the input drift compensated. A
    microphone turned on goes the module's own way: WebRTC's `InitRecording` stops the
    engine that only played and enables its input, and the aggregate is given to its unit
    before the input's format is read. Each device given is kept only when the engine
    itself then reads an output, and an input when it records, with channels and a rate
    (what the module checks next); else the unit is given back what it had. An
    AVAudioEngine given an aggregate can read its output with no channels at all while the
    HAL and the audio unit see two ("Output device not available"), in one order of the two
    devices and not in the other, whatever the aggregate's other settings, and which order
    it reads is not fixed: the input's device first is tried, then the output's (only when
    the output records nothing, so that the engine records from the microphone). When the
    input's device plays too and comes first, the engine's one channel is mapped to the
    output's first two channels alone (`kAudioOutputUnitProperty_ChannelMap`), set only
    once the device is read (R7), so a device refused leaves no map behind; a map that
    cannot be set, or a device of more than 64 channels, is refused. Making an aggregate
    waits 100 ms at most for its streams, on the module's thread.
  - The aggregate of the pair chosen is kept while that pair is chosen, present and
    usable: made at the choice (ahead of the microphone, while the engine only plays, so
    WebRTC has seen that change before the first unmute; only with the microphone
    permission granted, else at the first unmute after it is), kept through the mutes (the
    engine that only plays is given the output device itself), destroyed when the choice
    changes, a device goes or the pair is refused. Making or destroying a device tells
    WebRTC the devices changed (Apple patch 0008), and its echo canceller is then made anew
    at the next unmute instead of keeping what it knows of the room (WebRTC patches 0010,
    0011); kept, mutes and unmutes change no device. A choice itself still costs that once:
    the devices did change. An aggregate with no engine on it runs no device: made and left
    unused, neither of its devices nor the aggregate is running anywhere (the HAL's
    `kAudioDevicePropertyDeviceIsRunningSomewhere`), so the microphone is not opened. The
    last one stays until the process ends, or the next call chooses other devices.
  - R4. When the engine refuses at run time what R1 allowed (both orders, at a choice or
    at an unmute), or an engine given the pair with the microphone does not start
    (`checkStarted`, once the module's change is done) or stops twice in a row
    (`lookAtEngine`; a single stop only makes the engine again), or an engine with the pair
    does not run after a choice, the pair is taken
    as refused for this call (R1 then says no), and a new engine follows the system's
    devices (`followAnew`, asked once until made). If the module's request to record was
    dropped by a failed start, that engine only plays, and the call starts the recording
    again (see below). The same when an engine given a device must follow the system
    again.
  - R6. Once the devices settle after a change, an output (or a microphone, while
    recording) chosen that is there again but not in use is given back to the engine, as
    if chosen again, when R1 allows it.
  - `state:` answers facts only: for the output and the input, the one chosen and the name
    the list gave it, whether it is in the system now, and the one in use with its name
    (what follows the system is given as the system's devices, by their own ids; the input
    in use only while something records; no output in use while there is no engine running);
    whether
    the pair can be put together (R1) and "rate" when the rates differ; whether a choice is
    being carried out; whether the module was asked to record, whether it records (asked,
    its input let in, its engine running), and whether its input is let in. The method
    `fogataAudioOutput` (`FlutterWebRTCPlugin.m`, both copies) answers them, read-only.
  - `getSources` (both copies) leaves the aggregates out.
  - The log says "Fogata: the output chosen (…) given to the engine made again (result,
    running)", "a device of both made, the input first", "playing on …, recording from
    …, through a device of both", "the devices chosen cannot be put together (why): the
    engine follows the system's devices", "the engine reads from … an output of 0 channels
    …", "the output … cannot be used with the microphone …: why", "the engine refused …;
    following the system's devices", "a new engine following the system's devices", "the
    output chosen is there again: given back to the engine" and "the audio engine stopped
    while playing; made again without the devices chosen".
  - A call starts clean (`newCall`, the method `fogataNewCall`, before the call chooses its
    devices): the devices an earlier call chose, the pair it gave and the devices in use,
    and the pairs it saw refused are forgotten. An engine released forgets the pair given
    to it and the devices in use.
  - For the call test only, the method `fogataTestRefuse` (`testRefuse:`) makes R1 answer
    no for every pair (1), refuses the next pair given with the microphone in either order,
    once (2), or makes the next start of an engine given the pair fail, once, its formats
    read (3: the observer's `willStartEngine` answers an error, and the module undoes its
    change as when the engine does not start). It acts only in a process started with
    FOGATA_TEST_REFUSE set, which only the call test's launch sets; elsewhere, whatever
    calls it, nothing changes; and each call starts with it off.
- The application decides what to tell from those facts, every 2 s in a call with a
  device chosen (`Call._checkOutput`; with nothing chosen nothing is asked):
  `OutputWatch` tells "Sound cannot play on …. It plays on …" (with "… with this
  microphone" when the facts say the rates differ) when the engine plays on another
  output while the one chosen is there, seen twice in a row (a device plugged in again is
  given back within a moment, R6), and "… was unplugged. Sound plays on …" when the one
  chosen goes away, each time either begins; `InputWatch` tells "Could not record from ….
  Recording from …" the same way (a microphone unplugged is told from the device list,
  `MicWatch`). The one chosen being the system's own device, followed, is the one in use:
  nothing to say. Facts about another choice than the call's last, or taken while a choice
  is carried out, say nothing; back on the one chosen, a later change is told again. The
  person's choice stays chosen, shown and kept as made; the device panel shows "In use"
  beside the one used instead (`Call.outputInUse`, `inputInUse`).
- R5. When the module was asked to record and nothing records, or it was not asked
  although the call sends voice (a start that failed: the module then drops WebRTC's
  request and nothing asks again), two readings in a row with the microphone on and its
  input let in (`RecordingWatch`), the call has every voice encoding off, on every
  connection, up or not (one WebRTC finds up before the call does runs its streams), all
  of them before any starts again (WebRTC stops recording when its last sending stream
  stops). The step turns the screen's sound off too where the call's own channel sends it
  (Linux and Windows); on macOS that sound goes on connections of its own, with their own
  factory, and has no part in the microphone's recording, so this matters nowhere today.
  Then they are on again, so that WebRTC's
  AudioState asks the module to record afresh and restarts its echo canceller, as at any
  unmute (`Call._recoverVoice`). Not through `setMic`: the others are told nothing and the
  microphone's check is not started over. Given up as soon as the person turns the
  microphone off or the call ends. Once for each loss, and again when the engine changed
  since (the pair now refused, R4). Seen 4 to 6 s after the loss (two readings in a row,
  the voice sent read from the reading before), or, once the pair is seen refused, at the
  first reading with the microphone on, someone in the call up to take the voice since
  the reading before, and nothing recorded, asked or not (2 s or so; that holds until
  something records or the microphone is turned off; alone in the call it waits for
  someone, and one who has just come is given a second reading, since WebRTC may not
  record for them yet). The system's devices chosen again forget all of it. Each participant's own
  step is the only one that sets their encoding: while the restart runs it wants it off (a
  step of any origin meanwhile, a participant who arrives, does the same), and once no
  encoding of sound is on anywhere (3 s at most) every step runs again, turning them on
  with its own retries, the screen's sound as it was.
- The call judges whether its microphone reads anything from the start again when an
  output is chosen on macOS (`Call.useOutput`, `MicCheck.reset`), as it does for another
  input: the engine is made again then and records nothing for a moment.
- Without the observer (an embedding plugin registered one) the module chooses the devices
  itself, as before.
- Known: with an output chosen, the module counts itself on the system's default output,
  so a change of the system's default output makes its engine again (a short gap) where it
  kept the device chosen before. An engine made again without a change of what it enables
  (the system's default changed) reads its formats before it is given the devices chosen:
  a microphone chosen at another rate than the system's default input is read at the
  default's rate, as when the module gave the device itself. A device's rate changed while
  it is chosen is seen only at the next engine made.
- Not done: a Bluetooth headset as the microphone, whose rate changes when its microphone
  opens, after the aggregate was made at the rate it had: the engine would read the
  aggregate's formats at the old rate, and if it then does not start, the pair is taken as
  refused as above; if it starts at the wrong rate, nothing here sees it. The system's
  default output changing while a pair chosen is in use: the engine made again reads its
  formats, channels as well as rate, from the system's devices before the aggregate is
  given in `willStartEngine`, and a mismatch shows only as an engine that does not start
  (refused as above) or plays wrongly.
- `app/integration_test/call_test.dart`'s FOGATA_TEST_OUTPUT_SWAP chooses another output
  in the middle of a measured call and the first one again, turning the microphone off
  and on twice while the other plays (60 s each, FOGATA_TEST_MEASURE 186; under 56 is
  refused, as a stretch with the microphone on then has too few readings to judge its
  level), logs what the copy
  sent and its microphone read in each stretch (packets, seconds read, level, seconds
  spanned) and these lines, and fails (`integration_test/output_swap_verdict.dart`) when:
  past the two readings after a change, a reading has no voice sent or no sound read, or
  the level is exactly 0 in every judged reading of a stretch of five or more; while the
  facts say the pair can be put together, the output in use is not the one chosen (6 s
  after the choice, and at each reading with the microphone on), or an output notice is
  given; the microphone was turned off, or the person told it could not be used; or the
  module records at the end of a stretch with the microphone off, or 3 s after leaving the
  room. A reading spanning under a second (the last, as the measure ends) is not judged.
  FOGATA_TEST_REFUSE=pair has R1 answer no throughout, and the run fails unless the
  engine is never given the output chosen and the notice is given (R2);
  FOGATA_TEST_REFUSE=unmute has the second unmute's pair refused in either order, and
  FOGATA_TEST_REFUSE=start has that unmute's start fail with the pair given, and either
  run fails unless, at the end of the other output's time, the facts say the pair cannot
  be put together and the engine follows the system and records, and, when a reading of
  that stretch past its four (a second or more long) read no sound from the microphone,
  or always with =start, the call started the recording again (R4 and R5); that
  stretch is let take four readings.

## A received track is found as the connection that received it (all platforms)

A connection that receives a track another connection of this same application sends
gets it under the same id (libwebrtc names a received track after the sender's msid). The
application has such a loop: `lib/src/mic_test.dart`'s `LevelLoop`, which reads the
microphone's level while the person is alone in a call (and in the settings' microphone
test), and keeps the received track disabled so it does not play. The package looked a
track up among the local tracks first, so disabling the received track disabled the
microphone itself: every call that started alone then sent only silence (the microphone's
`audioLevel` still read, as WebRTC measures it before a disabled track is muted) until the
microphone was turned off and on, which opens a new track.

- `common/cpp/include/fogata_track_owner.h` (new), `common/cpp/include/flutter_webrtc_base.h`,
  `common/cpp/src/flutter_webrtc_base.cc`: `MediaTrackForId(id, owner)`, for a connection
  `owner` (the track's own `peerConnectionId`, neither `local` nor left out), finds a track
  that connection receives (its observer's list, then its receivers), or else one it sends
  (a sender's track is held with its connection's id), and nothing else: never this
  device's own track of that id nor another connection's, also once that connection is
  gone. `local`, or no owner, looks as the package did. `MediaStreamForId(id, ownerTag)`
  likewise finds no local stream for a connection's owner.
- `common/cpp/src/flutter_webrtc.cc`: `mediaStreamTrackSetEnable`, `setVolume`,
  `mediaStreamAddTrack`, `mediaStreamRemoveTrack` and `captureFrame` look up by owner;
  `trackDispose` for a connection's owner lets go of nothing unless that connection sends
  the id and does not receive it (a sender's track is this device's own); `streamDispose`
  for a connection's owner lets go of nothing (a received stream goes with its
  connection). `mediaStreamAddTrack` notes a received track with the local stream it is
  added to (`ReceivedInStreams`), and `common/cpp/src/flutter_media_stream.cc`:
  `MediaStreamDispose` and `MediaStreamTrackDispose` leave such a track out, so letting go
  of the stream `call.dart` makes to show a participant's picture never stops the camera
  or the shared screen of the same id; every other track goes by id, as before.
- Never by wrapper pointer, nor a wrapper kept past its statement: libwebrtc's C++ wrapper
  makes a new `RTCMediaTrack` for the same native track on every `RTCRtpReceiver::track()`
  and `RTCRtpSender::track()` (and new receivers and senders on every `receivers()` and
  `senders()`), and its `RTCMediaStream` makes new ones for all its tracks whenever one is
  added or removed (`MediaStreamImpl::OnChanged`, which the native stream calls inside
  `AddTrack`): a shared screen's stream, its sound added after its picture, never again
  gives the picture's object that `local_tracks_` holds. A track the owner-aware lookups
  find is held in a `scoped_refptr` while used (a bare pointer to one made by
  `receiver->track()` crashed `setVolume`). A renderer keeps the wrapper it was given and
  removes itself from that same one, so a new wrapper for the same track only moves it.
- `common/cpp/src/flutter_peerconnection.cc`: `getStats` with a track looks in that
  connection's receivers, then its senders, and nowhere else; an id it did not know among
  all tracks gave the whole report, so a narrow reading silently became a full one.
- `FlutterWebRTCPlugin.m`, `FlutterRTCPeerConnection.m` (both copies, identical; macOS):
  `trackForId:peerConnectionId:` and `streamForId:peerConnectionId:` the same;
  `mediaStreamAddTrack`, `mediaStreamRemoveTrack` (which found only local tracks),
  `trackDispose` the same as on Linux (by where the id is found: a sender's or receiver's
  `track` is a new object on each call), `streamDispose` by object (which holds there: a
  local stream's tracks are the very objects added to it, those `localTracks` keeps),
  and `mediaStreamAddTrack` and
  `mediaStreamRemoveTrack` answer once (an error was followed by a second answer); `getStats`
  with a track takes a receiver before a sender of the same id.
- `FlutterRTCPeerConnection.{h,m}`, `FlutterWebRTCPlugin.m` (both copies, identical;
  macOS): a connection's `remoteTracks` and `remoteStreams`, written by its delegate
  methods on WebRTC's signalling thread (WebRTC-SDK calls them there, and the package did
  not move them to the main thread) and read by these lookups on the main thread, are
  used only through `fogataRemoteTrack:` and its siblings, under one lock. The lock is
  never held while anything else is called, and what leaves a map is let go of after it,
  as freeing a received track or stream may wait for the signalling thread.
- `lib/src/native/media_stream_impl.dart`, `media_stream_track_impl.dart`: `addTrack`,
  `removeTrack` and `stop()` send the track's `peerConnectionId`, `dispose()` the stream's
  `ownerTag`. `lib/src/native/factory_impl.dart`: `createLocalMediaStream` makes a stream
  owned by `local`; the package gave it its label as owner, which now would read as a
  connection's.
- Tested by `common/cpp/test/track_owner_test.cc` (the rule, in plain C++) and the app's
  `test/webrtc_owner_test.dart` (what Dart sends). Checked by `tools/call-lab/mic.sh alone`
  (Linux): copy a is alone in the call for 6 s before b enters, and b must hear its voice.

## What libwebrtc's threads share with the platform thread (Linux, Windows)

libwebrtc calls a connection's observer on its signalling thread, while the platform
thread looks up what it received (`setVolume` as soon as a voice's `onTrack` arrives, a
track enabled or disabled, a renderer given a stream) and starts or stops listening to
its events. Both were plain containers with no lock.

- `common/cpp/include/fogata_remote_tracks.h` (new), `common/cpp/include/flutter_peerconnection.h`,
  `common/cpp/src/flutter_peerconnection.cc`: the observer's received streams and tracks
  are kept under a lock, never held while calling out: a search through a stream's
  tracks runs on a copy of the list, as it asks libwebrtc, which may wait for the
  signalling thread. Tested with ThreadSanitizer by `common/cpp/test/remote_tracks_test.cc`.
- `common/cpp/src/flutter_common.cc`: an event channel's queue, sink and listening flag
  are kept under a lock; events queued before Dart listened still go out first. Tested
  with ThreadSanitizer by `common/cpp/test/event_channel_test.cc` (Dart listening,
  cancelling and listening again while events come).

## A renderer freed while its stream changes

- `lib/src/native/rtc_video_renderer_impl.dart`: setting `srcObject` tells the renderer
  once the platform answers; a renderer disposed in between (a call that ends just as a
  stream arrives, someone coming and going at once) is no longer told, which the package
  did and which failed with "used after being disposed".


## A renderer disposed while its texture is made

- `lib/src/native/rtc_video_renderer_impl.dart`: `initialize` asks the platform for a
  texture and published it when the answer came, even for a renderer disposed meanwhile
  (a call that ends as it starts, renderers made and dropped quickly); `dispose` had no
  texture to let go of then, so nothing ever did. Now a texture that comes for a renderer
  being disposed is let go of at once and never published nor listened to, and `dispose`
  marks the renderer disposed whether it had a texture or not. Tested by
  `app/test/webrtc_event_errors_test.dart` (100 renderers whose textures come late).
## Only VP8 (macOS)

- `FlutterWebRTCPlugin.m` (both copies, identical): the encoder and decoder factories offer
  only VP8, and the decoder factory makes no decoder of another codec. Calls use VP8 alone
  (the application keeps only VP8 in every description). Fogata's own WebRTC-SDK holds no
  other video codec and Opus as its only audio codec (`tools/webrtc/MACOS.md`, patch
  0004); this check stays for any other framework. It replaces the package's raising of
  H.264's profile level, which no longer applies.

## No frame cryptor (all platforms)

The package's end-to-end encryption of frames and data packets (`FrameCryptor`,
`KeyProvider`, `DataPacketCryptor`) needs WebRTC's frame cryptor, which Fogata's own
libwebrtc and WebRTC-SDK leave out: calls never use it.

- `common/cpp/src/flutter_frame_cryptor.cc`, `common/cpp/src/flutter_data_packet_cryptor.cc`
  and their headers (`common/cpp/include/flutter_frame_cryptor.h`,
  `common/cpp/include/flutter_data_packet_cryptor.h`; Linux, Windows) and, for macOS, both
  copies of `FlutterRTCFrameCryptor` and `FlutterDataPacketCryptor`
  (`common/darwin/Classes/FlutterRTCFrameCryptor.{h,m}`,
  `common/darwin/Classes/FlutterDataPacketCryptor.{h,m}`,
  `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterRTCFrameCryptor.m`,
  `macos/flutter_webrtc/Sources/flutter_webrtc/FlutterDataPacketCryptor.m`,
  `macos/flutter_webrtc/Sources/flutter_webrtc/include/flutter_webrtc/FlutterRTCFrameCryptor.h`,
  `macos/flutter_webrtc/Sources/flutter_webrtc/include/flutter_webrtc/FlutterDataPacketCryptor.h`)
  are removed, with their method handlers and key provider maps
  (`flutter_webrtc.{h,cc}`, `flutter_webrtc_base.{h,cc}`, `FlutterWebRTCPlugin.{h,m}`) and
  the sources in `linux/`, `windows/` and `elinux/` `CMakeLists.txt`. Their methods now
  answer "not implemented"; the Dart classes (`frame_cryptor_impl.dart`,
  `data_packet_cryptor_impl.dart`) stay as the package has them, unused.

## libwebrtc (Linux, Windows)

The package downloads webrtc-sdk's prebuilt libwebrtc. Fogata builds its own instead
(`tools/webrtc` at the root of the repository, see its README): WebRTC M158 (branch-head
8127) with webrtc-sdk's C++ wrapper, the same API, with VP8 and Opus as the only codecs
and VP8 keyframes larger than 8K refused before they are decoded (see "Pictures too large
to draw").

- `third_party/libwebrtc_version.ini`: `fogata-libwebrtc.m158.8127.3`, with the SHA-256 of
  every archive (`sha256_<os>-<arch>`); only the systems Fogata ships are listed.
- `third_party/CMakeLists.txt`: the archive must match that SHA-256, wherever it comes
  from; archives are named after the version, and a stamp in `libwebrtc/` makes a new
  version or archive be extracted again (the package kept using whatever archive it had
  downloaded first). What is extracted gets the time of its extraction: every file of an
  archive carries one fixed date, so headers of another archive looked unchanged and a
  build that was not clean kept objects compiled against the old ones (seen as a failed
  link when the wrapper's types changed). The build takes them from this computer's store,
  `<home>/fogata-libwebrtc/<version>/` (or `FOGATA_LIBWEBRTC_DIR`), where `tools/webrtc`
  puts what it builds, or from `FOGATA_LIBWEBRTC_ARCHIVE`, naming one archive; the ini's
  `download_url`, the releases of holypandadev/fogata-libwebrtc, is the last place tried.
- The archive carries `NOTICE.md`, the licences of everything built into the library, and
  the wrapper's `LICENSE`. `tools/third_party_notices.py` puts them into THIRD-PARTY-NOTICES
  from the copies kept in `tools/notices/libwebrtc` (so it also runs on a Mac, which never
  unpacks the archive), and fails when the unpacked archive's files differ from the copies:
  copy the new ones over when libwebrtc changes.
- macOS (`macos/flutter_webrtc.podspec`): WebRTC-SDK 158.8127.3, Fogata's own build of the
  same WebRTC (`tools/webrtc/MACOS.md`), which the app's Podfile takes from
  `app/macos/WebRTC-SDK.podspec` (this computer's store, checked by SHA-256). iOS
  (`ios/flutter_webrtc.podspec`, never built) still names webrtc-sdk's 150.7871.04.
- `macos/flutter_webrtc/Package.swift` and `ios/flutter_webrtc/Package.swift` are removed:
  they declared webrtc-sdk's WebRTC.xcframework 150.7871.04 as a binary, and a build with
  Swift Package Manager on would link it beside Fogata's (the app turns it off in
  `app/pubspec.yaml`, which `tools/test_repo_checks.py` pins; without a Package.swift the
  plugin is built through its podspec even if that setting were lost).

## What a method call and a statistics read keep (Linux, Windows)

- `linux/flutter/core_implementations.cc`, `linux/flutter/binary_reply.h` (new; Linux): the
  package took a reference to the response handle of every method called from Dart and
  never gave it back, so each call kept the engine's handle (about 40 bytes) for good, and
  `getStats` is called every few hundred milliseconds in a call. The handle is now let go
  of once the call's one reply is sent; one never replied to stays held, as before (the
  engine complains about a handle let go of unanswered, on whichever thread that happens).
  `common/cpp/test/binary_reply_test.cc` tests it. Windows uses Flutter's own channel code.
- `getStats` (`common/cpp/src/flutter_peerconnection.cc`) is unchanged, but libwebrtc now
  lets go of what it is given (`tools/webrtc/patches/wrapper`, the statistics callback and
  `portable::vector`; see `tools/webrtc/README.md`): its callback kept the two closures,
  and in them the shared pointer to the call's result and the result itself, and every
  array of reports and of their members was never freed, about 1.9 KB for each read in all.
  macOS uses WebRTC's Objective-C SDK instead, whose statistics callback lets go of its
  block once the report is delivered (`RTCPeerConnection+Stats.mm`); nothing was changed
  there.

## Pictures too large to draw (all platforms)

The renderer copies each picture of a remote track into a buffer of width * height * 4
bytes, made again whenever the size changes; a peer can send frames up to the codec's
16383 by 16383 pixels, a gigabyte each. Frames larger than 8K (7680 by 4320 pixels, the
largest screen shared as captured) or with a side over 8192 are dropped, with one line in
the log ("Dropping video frames of WxH: too large to draw").

The first such frame of each track the renderer is given is told to Dart, with an event
`fogataFrameTooLarge` (`width`, `height`) that `lib/src/native/rtc_video_renderer_impl.dart`
hands to the renderer's `onFrameTooLarge`: the application keeps out of the call whoever
sends one, at once, where the statistics read every 2 s show only the last frame's size.

- `common/cpp/include/flutter_video_renderer.h`, `common/cpp/src/flutter_video_renderer.cc`
  (Linux, Windows): `FrameSizeAllowed`; `OnFrame` keeps no such frame and sends no other
  event for it, and `CopyPixelBuffer` never sizes its buffer for one.
- Linux and Windows: Fogata's libwebrtc does not decode a VP8 keyframe larger than that
  (nor the frames after it until the next keyframe it decodes): it hands on a black
  picture of 2 by 2 whose `RTCVideoFrame::TooLarge` gives the size refused. `OnFrame`
  takes that size as the frame's, so the stand-in is dropped and told like any frame too
  large. A peer sending such pictures costs no more memory than an ordinary call.
  `common/cpp/test/video_renderer_test.cc` checks the event for a stand-in.
- `FlutterRTCVideoRenderer.m` (both copies, identical; macOS): `renderFrame:` drops
  such frames, keeps none for `copyPixelBuffer`, and tells the first of a track. With Fogata's own WebRTC framework (`tools/webrtc/MACOS.md`), whose VP8
  decoder refuses such keyframes before decoding them, a stand-in's buffer gives the size
  refused (`refusedSize`, read by name so WebRTC-SDK's builds, which lack it, still work):
  it is taken as the frame's size, so the stand-in is dropped and told the same way.

## The renderer's picture buffer (Linux, Windows)

The renderer made its `FlutterDesktopPixelBuffer` again at the first frame of every track it
was given, on the thread frames come on and outside its lock, while the raster thread could
be reading the one `CopyPixelBuffer` had just returned (a stream shown in place of another).

- `common/cpp/src/flutter_video_renderer.cc`: `initialize` makes it once; it lives as long
  as the renderer, and only `CopyPixelBuffer` changes it, on the raster thread. It takes
  the newest frame under the lock and converts it after: held through the conversion
  (milliseconds for a large picture), the lock held up every frame coming in. The same
  frame asked for again is not converted again.
- What `OnFrame` keeps of the frames before (size, rotation, first frame, a frame too
  large told) is set back by `SetVideoTrack` on the platform thread while frames come on
  another: both take the lock for it. `SetVideoTrack` removes the renderer from the old
  track first, and never under the lock, which `OnFrame` takes while the track holds its
  own.
- `common/cpp/test/video_renderer_test.cc` and `run_tests.sh` (new; Linux): a test built
  with ThreadSanitizer: the events sent for a track's frames, then the platform thread
  changing tracks while frames (some too large) come on another and a third asks for the
  picture and reads it; any data race fails it. It needs the libwebrtc a Linux build
  unpacks, and the Flutter on PATH for the engine's headers; `tools/ci.sh` runs it after
  its Linux build.
- `common/cpp/test/rejected_section_test.cc` (new; Linux, in `run_tests.sh`): not a change
  of the plugin, a check of what call.dart counts on. A real offer with a section rejected
  as `onlyCallCodecs` rejects one (port 0, out of the BUNDLE group) is taken by
  `SetLocalDescription`, and that section's transceiver stops; with the first section
  rejected but still in the group, the offer is refused. It reaches the sound server, as
  libwebrtc's audio module needs one at start, but adds no track.

## The newest frame drawn, converted only when drawn (macOS)

The renderer wrote every frame into one `CVPixelBuffer`, under its lock, and dropped any
frame that came before Flutter had taken the last one: whenever Flutter drew late, the
newest frame was lost and an older one stayed on screen. Every frame was also converted as
it came, on the thread it came on, whether Flutter drew it or not: a tile off screen, a
window hidden, this device's own screen while it is shared and not shown (its renderer
gets every captured frame, for its size), frames faster than the display.

- `FlutterRTCVideoRenderer.m` (both copies, identical): `renderFrame:` keeps only the
  newest frame, as it came (one buffer of the decoder or the capture held per renderer),
  in place of the one before, tells Flutter a frame is there, and still sends the size,
  rotation and first-frame events for every frame, drawn or not, as the application
  watches them. `copyPixelBuffer` (Flutter's raster thread, when it draws the texture)
  converts that frame into a buffer of its own from a `CVPixelBufferPool` of the frame's
  size (turned), outside the lock frames take, and answers that picture again until a
  newer frame comes: a frame is converted once at most, and one never drawn never. A
  buffer Flutter may still draw from is never written again. A renderer disposed answers
  no picture and keeps no frame nor picture, also one disposed during a conversion.
  `setSize:` does nothing.
- `FogataFrameConvert.{h,m}` (new, built copy only): the conversion. The others' cameras
  and screens come decoded by libvpx (VP8 alone, the framework has no VideoToolbox
  decoder) as I420 buffers, converted with libyuv's `I420ToARGB` as before (`toI420` gives the
  buffer itself, no copy; a frame turned is turned first). This device's
  own camera and screen come as NV12 `CVPixelBuffer`s (the camera's '420v' or '420f',
  ScreenCaptureKit's '420f'), which were copied whole into I420 first: their luma is now
  read in place and only the chroma is split into U and V, in a buffer the renderer keeps,
  then the same `I420ToARGB` (libyuv's `NV12ToARGB` is not exported by the framework, and
  `RTCYUVHelper` has none). The picture is the same to the bit, with the same colour
  matrix and range as always (BT.601, video range, also for '420f'). A frame turned,
  scaled, cropped at an odd place, or in another format goes the way it went.
- `macos/test/run_tests.sh` (new, in `tools/check.sh` on a Mac): `frame_convert_test.m`
  converts every kind of frame both ways and compares them byte for byte (`--measure` also
  times them); `video_renderer_test.m` runs the renderer itself, with the FlutterMacOS
  framework but no engine, under ThreadSanitizer: frames given but not drawn are not
  converted and still told, the newest is drawn and converted once, sizes and turns, a
  frame too large, a dispose with a frame waiting, and frames, pictures asked for and a
  dispose on three threads.
- Measured on an Apple silicon Mac (`--measure`, -Os as a release build, the same frame
  again and again, so the caches flatter the copies made before): an I420 frame takes the
  same as before (0.17 ms at 1280x720, 0.67 ms at 2560x1440); an NV12 frame 0.19 ms instead
  of 0.21 at 1280x720 and 0.76 instead of 0.83 at 2560x1440, and allocates nothing instead
  of 1.4 MB and 5.5 MB a frame. The larger saving is in the frames not converted at all.

## A texture before its first frame (Linux)

Flutter's Linux engine draws a pixel-buffer texture by asking for its pixels; given none
(a renderer with no frame yet, as with a camera that never sends one), it makes a texture
of the size drawn without filling it, which showed whatever that graphics memory held:
pieces of other pictures, their colours swapped. Windows and macOS draw nothing then.

- `linux/flutter/core_implementations.cc`: `fl_texture_proxy_copy_pixels` gives one
  transparent pixel until there is a frame.

## PipeWire buffers checked before reading (Linux)

- `linux/portal_screen_capturer.cc`: a shared screen's frame is read only when its stride
  holds a row and every row lies within the buffer (`chunk->offset`, `chunk->stride`
  against `maxsize`); otherwise the frame is skipped.
- `linux/pipewire_apps_capturer.cc`: a screen's sound is read only when the chunk lies
  within the buffer and starts on a sample.

## The application's other sounds on the chosen speaker (Linux)

WebRTC plays a call on the output chosen with `selectAudioOutput`; the application's
short sounds (through ALSA) and chat videos (through GStreamer) open the system's default.

- `linux/output_keeper.{h,cc}` (new, with libpulse): keeps the chosen output's description
  and moves every sink input of this process to that sink as it appears (and when the
  sink appears again); the system's default leaves streams where they open. A connection
  lost (the sound server restarted) is made again 1 s later, then after 2 and 4 s, then
  every 5 s; it kept a dead connection and moved nothing until the application
  restarted. `linux/input_watcher.cc`'s connection does the same, and once connected again
  runs the change it would have missed.
- `common/cpp/src/flutter_webrtc.cc`: the method `fogataSoundsTo` (`deviceId`, as
  `getSources` names outputs) sets it, outside a call too.
- `common/cpp/src/flutter_media_stream.cc`: `selectAudioOutput` sets it as well.

## The chosen microphone for voice messages (Linux)

The application records voice messages itself (in its Rust library, through ALSA's plugin for
the sound server), from the microphone chosen in its settings. WebRTC lists inputs by their
description in the sound server, which ALSA's plugin does not take; it opens an input by the
server's own name for it.

- `linux/input_name.{h,cc}` (new, with libpulse): `InputName` asks the sound server once for
  the input whose description (or name) is the one given and answers its name; empty when
  none matches, when the server does not answer within a second, or without libpulse.
- `common/cpp/src/flutter_webrtc.cc`: the method `fogataInputName` (`deviceId`, as
  `getSources` names it) answers that name.

## No call without a sound server (Linux)

WebRTC's voice engine, made with a call's first connection, checked that its audio device
starts (`adm_helpers.cc`: `RTC_CHECK_EQ(0, adm->Init())`, also in release builds) and ended
the process otherwise; Fogata's libwebrtc keeps the engine up without a device now
(`tools/webrtc/patches/webrtc/0007`), but a call made then would play and record nothing. On Linux Fogata's libwebrtc takes the PulseAudio device whenever libpulse loads (it has ALSA too, never used then), so with no
sound server answering (none running, a sandbox without its socket) entering a call aborted
the application.

- `linux/input_watcher.{h,cc}`: `InputWatcher::CheckServer` connects to the sound server
  with libpulse (never starting one) in a loop of its own that sleeps until the server
  answers, the connection fails or 5 s pass, and says which: ready, none (the connection
  was refused or failed), timeout (it took the connection but did not answer in 5 s) or
  unknown (without libpulse, or no memory).
- `common/cpp/src/flutter_webrtc.cc`: the method `fogataSoundServer` runs that check on a
  thread of its own and answers "ready", "none", "timeout" or "unknown" through the
  method's result, which posts to the platform thread: the platform thread never waits.
  A check still running when the plugin goes answers nothing (`AnswerGate`). The
  application asks before it makes a call (`AppState._startCall`): on "timeout" it asks
  once more, as a slow server (a fresh session, a busy computer) answers then; it makes no
  call on "none" or on a second "timeout" (WebRTC's own connection to such a server fails
  the same way, after 5 s, and the call would have no sound), saying why;
  the room and its chat stay (`tools/call-lab/mic.sh broken`, `alsa` and `hang` check it).
- `linux/CMakeLists.txt`: a build other than Debug fails without libpulse's headers, as the
  check would always say "unknown" and the voice engine could end the application.

## A sound server that goes away and comes back (Linux)

PipeWire's PulseAudio service, or PulseAudio, restarted while the application runs (as
after a system update: `systemctl --user restart pipewire pipewire-pulse wireplumber`)
left libwebrtc's PulseAudio device with a dead connection for good: its lists held only
"System default", and nothing played or recorded until the application restarted (the
voice engine stays, "The voice engine stays", so it was never made again).

- `tools/webrtc/patches/webrtc/0006` (Fogata's libwebrtc): PulseAudio's mainloop thread
  sees the device's connection fail and posts to WebRTC's network thread (its worker
  thread before M158), where the voice engine calls the device and where everything else
  happens. There the device stops
  playout and recording, keeping what the voice engine had asked for; lists no input and
  no output at all, not even the default, while no server answers; tells its observer
  (the wrapper's `OnDeviceChange`, which on Linux libwebrtc never told before); and
  connects again on the same mainloop, without waiting for an answer: 0.5 s after the
  loss, then after 1, 2 and 4 s, then every 5 s; a server that takes the connection and
  does not answer in 5 s is tried again the same way. Once connected it takes the chosen
  output and input again by the server's own names for them where they are still listed
  (else the default), starts playout and recording again as they were (what does not
  start, as with no device listed yet, is tried again every second for 5 s, then every
  5 s), and tells its observer again. A chosen device not listed yet (the server answers
  before its devices are back, or it was unplugged meanwhile) is waited for on the
  default: the device follows the server's sinks and sources, and moves playout or
  recording to it as soon as the server adds it; the plugin's own `KeepRecordingDevice`
  could not, as the input's place in the list was the same as before. Every wait of the device on the server is bounded:
  its first connection (it waited for ever for a server that took the connection and
  never answered) and every question to it are given up after 5 s, as a stream starting
  already was after 9 s.
- `tools/webrtc/patches/wrapper/0010`: the lists' counts are read on that thread
  too, the listener is set and called under a lock, and a null one stops it.
- `common/cpp/src/flutter_webrtc_base.cc`: the plugin hears of it as of any other change
  (`KeepRecordingDevice`, then `onDeviceChange`, on the platform thread), which reads no
  device while the server is gone and so selects nothing; it lets go of its listener as
  it goes.
- `linux/input_watcher.cc`, `linux/output_keeper.cc`: their own connections are made again
  after 1, 2 and 4 s, then every 5 s (up to 30 s before: a call's chosen microphone, and
  the chosen output of the application's other sounds, waited that long after the server
  was back); a connection that could not even be made is tried again too.
- The application (`app/lib/src/call.dart`, `SoundServerWatch` in `call/media.dart`): a
  list with no sound device at all, on Linux, is the voice engine reaching no server.
  Meanwhile nothing is said of the microphone, and it is not judged as reading nothing
  (it would be turned off); a server gone 5 s is told ("This computer's sound system
  stopped..."), and its return then told too. A silent input in use that is no longer
  listed (it went with PipeWire's connection) is made and chosen again.
- Checked by `tools/call-lab/mic.sh restart` (PipeWire's PulseAudio service alone, all
  three, the server gone 20 s, and gone a minute) on two copies in a call.

## The Linux task runner

`linux/task_runner_linux.{h,cc}`: the package ran queued tasks with `g_main_context_invoke`,
which on the platform thread runs them at once, inside whatever queued them, and under the
queue's lock, so a task that queued another (an event sent from a task) waited for itself.
Each task now runs later, from an idle source on the main context, with the queue let go.
The queue is shared with the sources still to run and closed when the runner goes, so a
source left after it runs nothing. `linux/flutter_webrtc_plugin.cc` makes the runner before
the plugin's `FlutterWebRTC` and lets it go after, so what that queues while it goes still
finds it.

## The Windows task runner

- `windows/task_runner_windows.cc`: as 1.6.3 has it (each task run with the queue's lock
  let go: Fogata's own tasks queue others, a shared window that closes sends
  `mediaStreamTrackEnded` from a task), with `<utility>` included for the `std::move` it
  uses.
- `windows/flutter_webrtc_plugin.cc`: the runner is declared before the plugin's
  `FlutterWebRTC`, so it is freed after it, as on Linux: freed first, it was gone while
  `FlutterWebRTC` was torn down and posted its last events through it. 1.6.3 gets the
  same order by letting go of `FlutterWebRTC` first in the plugin's destructor; the
  declaration order does it without one.

## A made-up camera for tests (Linux)

- `linux/test_camera.{h,cc}` (new): in a debug build (no `NDEBUG`), with
  `FOGATA_TEST_CAMERA=1` and no camera on the computer, `getUserMedia` gets a made-up one instead of no picture: a textured background
  that sways, a shape moving over it and a little noise on every frame, at the asked size
  and rate, fed to a custom video source from a thread of its own. The encoder works on it
  about as on a real camera (at 1280x720 it fills 1.5 Mbit/s), so tests on computers and
  virtual displays without a camera can measure what a camera sends. With
  `FOGATA_TEST_CAMERA_GIANT=<w>x<h>` as well, once a second it sends a flat picture of
  that size (at most 16383 by 16383) in place of an ordinary one, as a modified copy
  could, for tests of what a receiver does with pictures larger than any copy sends.
- `common/cpp/src/flutter_media_stream.cc`: `GetUserVideo` uses it then; its capturer is
  kept and stopped like a real one's.

## Cameras through the Camera portal (Linux)

In a Flatpak there is no `/dev/video*` short of every device of the computer
(`--device=all`), and libwebrtc opened cameras through V4L2 alone. Its wrapper now also takes
WebRTC's PipeWire path through xdg-desktop-portal's Camera interface
(`tools/webrtc/patches/wrapper/0009`, `patches/webrtc/0005`), and the plugin chooses:

- `linux/camera_access.{h,cc}` (new): in a Flatpak (`/.flatpak-info`) the cameras come
  through the portal, V4L2 standing in only while the portal is unavailable (never after a
  refusal); elsewhere V4L2, as before. `FOGATA_CAMERA=portal` takes the portal outside a
  Flatpak too, `FOGATA_CAMERA=direct` V4L2 inside one. A plain build stays on V4L2, which
  it can open itself: through the portal it works too (KDE Plasma 6.6, xdg-desktop-portal
  1.21, PipeWire 1.6, WirePlumber 0.5: listed, opened, cameras plugged in and out), but the
  portal then asks the person about an application it cannot name (KDE's question for a
  program outside a sandbox), on every system where V4L2 needed nothing; it lists only the
  cameras the session manager hands to PipeWire; and a portal started before PipeWire was
  ready offers no Camera interface at all until it starts again (seen in
  tools/smoke-wayland's session). GNOME was not tried.
- `common/cpp/src/flutter_webrtc_base.cc`: sets the video device's way at start
  (`RTCVideoDevice::SetCameraPortal`).
- `common/cpp/src/flutter_webrtc.cc`: `getUserMedia` with a camera asks the portal first
  unless granted (the system asks the person once; a refusal or no portal is asked again,
  since the person may have allowed it since), and answers once it has; the answer comes
  on GLib's main loop or PipeWire's thread, and the rest runs from the platform thread,
  unless the plugin went meanwhile. `fogataCameraAccess` (new) tells the way and the
  answer ("direct", "notAsked", "asking", "granted", "denied", "unavailable"), and with
  `request` asks first; the app's settings ask when the camera's choice shows.
- `common/cpp/src/flutter_media_stream.cc`, `common/cpp/include/flutter_media_stream.h`
  (`GetUserVideo` answers false, with the error to tell, where it answered nothing): a
  camera refused through the portal, or no portal to reach any (and none opened
  directly), fails `getUserMedia` with
  `NotAllowedError ... (fogataCameraDenied)` or `NotReadableError ...
  (fogataCameraUnavailable)`, which the app tells apart; a camera listed but not opened,
  or one that does not start, fails it too (`NotReadableError`) instead of giving a black
  picture, on Windows as on Linux. The camera is opened before the microphone, so a
  failure leaves nothing open; a device gone between its count and its name is skipped.
- A camera PipeWire reaches through V4L2 keeps the id V4L2 gives it (its bus), so a camera
  chosen while cameras were opened directly stays chosen.
- `common/cpp/test/camera_portal_test.cc` (new): the way chosen, and, in a session with a
  portal (`tools/beta/test-flatpak-camera.sh`, also in a Flatpak), access granted or
  refused, the camera listed with its V4L2 id and giving pictures, cameras plugged in and
  out meanwhile.

## Errors of the event channels (Dart)

The package rethrew the errors of an `EventChannel` from the stream listener, where nothing
could catch them and the connection or renderer went on as if nothing had happened.

- `lib/src/native/event_errors.dart` (new, exported by `lib/flutter_webrtc.dart`):
  `onWebRtcEventError`, set once by the
  application (`lib/main.dart`, which notes it in the day's log), gets every such error with
  what it was about; without it the error is printed.
- `RTCPeerConnectionNative.errorListener` (`lib/src/native/rtc_peerconnection_impl.dart`):
  reports the error, and the first one makes the
  connection fail: `onConnectionState` is called with `RTCPeerConnectionStateFailed`, as for a
  network failure (the call's own handlers for that state then run), and it is closed.
  `close()` runs once however many times it is called.
- `RTCVideoRenderer.errorListener`: reports the error, and the first one makes the renderer
  fail: its value is emptied, it listens to the channel no more, `failed` is true and the new
  `onError` callback is told once (the call puts a new renderer in its place, up to three times, and
  then gives the picture up). `dispose()` runs once however many times it is called.

Every other listener of the plugin's event channels has an error handler, and none throws:
`reportEventError` (`lib/src/native/event_channel.dart`) logs the error and the events go
on. Upstream, the listeners of the shared `FlutterWebRTC.Event` channel (native logs,
device changes, the desktop capturer, a capture's end) had none, and the handlers of the
others (connections, data channels, renderers, frame cryptors, platform views) threw the
error again; either way it reached the zone as an uncaught error. The shared channel's
own errors now go on to its listeners, each of which logs them. Among them:
`lib/src/native/mediadevices_impl.dart` (device changes), `lib/src/native_logs_listener.dart`
(native logs) and `lib/src/native/rtc_data_channel_impl.dart` (a data channel's
`errorListener`). Tested by `app/test/webrtc_events_test.dart`.

## What the Dart side prints (Dart)

The package printed some errors with `print`, which a release build writes as they are to
standard output (on Linux, where a desktop starts applications in a scope of systemd, into
the system's journal); they can name a device or a source. They go through `debugPrint`
instead (`rtc_video_renderer_impl.dart`, `rtc_video_platform_view_controller.dart`,
`desktop_capturer_impl.dart`, `frame_cryptor_impl.dart`, `event_errors.dart`), which the
application's release build sends to its day's log, scrubbed, and not to the console
(`lib/src/console.dart`).

## A renderer freed once the engine is done with it (Linux)

The engine copies a texture's pixels on its raster thread without holding a reference,
and reads them after the copy returns; `videoRendererDispose` unregistered the texture
and freed the renderer at once, so a participant leaving as a frame of theirs was drawn
had the raster thread read freed memory.

- `linux/flutter/include/flutter/texture_registrar.h`, `linux/flutter/texture_registrar_impl.h`,
  `linux/flutter/core_implementations.cc`: `UnregisterTextureLater` keeps the texture
  (its proxy) and runs a release on the platform thread only once the raster thread has
  started copying another texture after the unregistering (the rule of
  `packages/fogata_video`'s `retire`); the proxy's last unref runs on the raster thread.
  The proxy's texture pointer is atomic, and the registrar's list of textures has a lock
  (frames are marked available on the threads they arrive on).
- `common/cpp/src/flutter_video_renderer.cc`: a disposed renderer is held by that release
  until then. Windows already waits for the engine's own callback.
- `linux/flutter/retired_textures.h` (new): the list. When the engine copies no other
  texture afterwards (the last picture of a call gone, nothing else drawn), the release
  still runs 5 s after the unregistering, from a timeout on the platform thread: only a
  copy that started before the unregistering could read the renderer (the texture's
  pointer to it is cleared first), and none lasts seconds. The texture itself still waits
  for the raster thread's next copy, for its last unref there. Before, a call left with
  no picture kept the last renderer and its picture (up to 132 MB at 8K) indefinitely.
  Tested with ThreadSanitizer by `common/cpp/test/retired_textures_test.cc`.

## A renderer's dispose (Linux, Windows)

1.6.3's "stop VideoRendererDispose from releasing a renderer twice" (`videoRendererDispose`
takes the renderer out of the map at once and closes its event channel while the texture id
is still its own) is as published. `common/cpp/test/video_renderer_test.cc` (Linux) checks
that a disposed renderer has left its track and closed its channel.

## A connection freed after its close (Linux, Windows)

`peerConnectionClose` lets go of the connection but not of its observer (which holds it,
its event channel and its remote tracks), and `peerConnectionDispose` looked the
connection up only among those not closed: disposed after its close, as the application
does with every connection (`closeConnection` in `app/lib/src/call.dart`), nothing was
freed until the application quit.

- `common/cpp/src/flutter_webrtc.cc`, `common/cpp/include/flutter_peerconnection.h`:
  `peerConnectionDispose` finds a closed connection through its observer
  (`FlutterPeerConnectionObserver::peerconnection`), so the observer is detached and both
  go. macOS keeps a closed connection listed until its dispose, and needed no change.

## A transceiver added while its connection closes (Windows)

On Windows `addTransceiver` and `addTrack` run on a thread of their own (the first audio
sender's set-up kept the platform thread, the window's message loop, for about 600 ms),
and the application closes a connection whose participant left while an offer to them is
being made: the close came while one of the offer's transceivers was being added. The
thread had the connection by a plain pointer, which the close and dispose let go of, and
libwebrtc's connection, being let go of by the close, still took the call: it ran on the
destroyed `PeerConnection` and made a video channel with its `Call` already freed (an
access violation in `WebRtcVideoSendChannel`'s constructor, once in some 30 calls).

- `common/cpp/src/flutter_peerconnection.cc` (`AddTransceiver`, `AddTrack`): the thread
  holds the connection's object (`scoped_refptr`) until its call is done.
- `tools/webrtc/patches/wrapper/0023`: libwebrtc's connection is taken out under a lock
  before it is let go of, and every call holds it to its end or finds it closed; a call
  that comes after the close is answered with nothing (no transceiver, which the plugin
  answers with an error; no sender; the descriptions' and offers' failure). The
  application's call code takes it as it takes a call made after the close
  ("peerConnection is null"): the step fails and a line goes in the call's log.
- The same patch makes a close wait for WebRTC's own close: the plugin frees its observer
  after `close()`, and a connection still held by such a call went on calling it. The
  close now reports the connection closed, as on macOS: `iceConnectionState`,
  `peerConnectionState` and `signalingState` events with `closed` reach the application
  after `close()`. The call code ignores them for a connection it no longer has (the
  connection state) or writes a line in the call's verbose log (`ice …: closed`, `screen
  sound to/from …: closed`); nothing else follows from them.

Linux and macOS call the connection on the platform thread only, where a close cannot
come in the middle of a call.

Those two Windows threads answer only while the plugin is there: the application may quit
while one waits (the first audio sender's set-up), and its answer went through the
plugin's task runner after the plugin had gone. They answer under the plugin's gate, as
the sound server's checks do (`common/cpp/include/fogata_answer_gate.h`, tested by
`common/cpp/test/answer_gate_test.cc`). And `addTrack` without a sender answers with an
error, as `addTransceiver` does; its empty success made the Dart side read a sender from
nothing.

They also held the connection, its track and what the call returned after the plugin
went: quitting while one waited, the plugin's `FlutterWebRTC` went, libwebrtc ended in
`~FlutterWebRTCBase` (`LibWebRTC::Terminate`, then its factory and threads), and the
thread, once its call returned, read the transceiver or sender for its answer and let go
of all it held, on libwebrtc already ended. The plugin now waits for them as it goes
(`common/cpp/include/fogata_worker_fence.h`, tested by `common/cpp/test/worker_fence_test.cc`
under ThreadSanitizer):

- `common/cpp/src/flutter_peerconnection.cc` (`AddTransceiver`, `AddTrack`): everything
  of libwebrtc's a call holds (the connection, the track, the init, the transceiver or
  sender it returns) and its result are in one place. A thread that starts after the
  plugin started going makes no call and keeps them; one whose call returns after the
  plugin stopped waiting for it reads nothing, answers nobody and keeps them; any other
  lets go of them before the plugin's wait returns.
- `common/cpp/src/flutter_webrtc.cc`: `~FlutterWebRTC`, after closing the gate and before
  libwebrtc ends, refuses every later thread and waits up to 2 s for those in their call
  (the first audio sender's set-up takes about 600 ms). One still in its call then is
  left in it (nothing can take it out, and libwebrtc ends under that call as before),
  with a line on standard error; once the call returns, it keeps what it holds for good.
  The wait covers a call that takes up to 2 s, which a stalled signalling thread can
  exceed. One whose call had returned is
  waited for to its end, with no limit of the plugin's own: its call returned, so
  libwebrtc's signalling thread was answering, and the application ends the process 10 s
  after a quit begins in any case.
- These two are the plugin's only threads that hold libwebrtc's objects. On Linux they
  run on the platform thread, so there is never one to wait for.

## The voice engine stays (all platforms)

WebRTC M156 starts its voice engine with the first connection and stops it when the last
one is freed (`ConnectionContext::ReleaseMediaEngine`, `WebRtcVoiceEngine::Terminate`),
which terminates the audio device too. On Linux and Windows the wrapper's audio device then
answers -1 for the number of inputs and outputs (`AudioDeviceModuleImpl`, not initialized),
so `getSources` listed no microphone and no speaker at all; the wrapper initializes the
device once when the factory is made, never again. After the settings' microphone test, or
a call, freed its connections, the settings showed only "System default" until the
application was restarted. On macOS the audio module stops following the system's devices
(its listeners go with `Terminate`), so a device plugged in or out afterwards was never
listed or left the list. Everywhere, the engine made again by the next connection went back
to the system's default devices.

- `common/cpp/include/flutter_webrtc_base.h`, `common/cpp/src/flutter_webrtc_base.cc`,
  `common/cpp/src/flutter_peerconnection.cc` (Linux, Windows): `KeepVoiceEngine` makes an
  idle connection of the plugin's own (no ICE servers, nothing added) right after the
  first connection the application makes, and keeps it until the plugin goes, so the
  engine never stops. Not before: without a sound server, starting the engine ends the
  application ("No call without a sound server"), and the application makes its first
  connection only once it knows there is one. Recording and playing still stop with the
  last channel that sends or receives sound; only the device stays initialized.
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS): `keepVoiceEngine`, the same,
  for connections of the plugin's own factory (not the screen sound's).
- `common/cpp/src/flutter_peerconnection.cc`: `peerConnectionDispose` also has the factory
  forget the connection (`Delete`): the wrapper's factory lists every connection it makes,
  and kept each one until the application quit.
- The application chooses the system's default devices by their own ids when none is
  chosen (`app/lib/src/call.dart`), as the engine now keeps the devices it was given last
  from one call or microphone test to the next.
- A sound server restarted while the application runs (Linux) is reached again by the
  audio device itself: see "A sound server that goes away and comes back".
- `common/cpp/test/voice_engine_test.cc` (new; Linux, in `run_tests.sh`): what this counts
  on, with the real libwebrtc and the computer's sound server: the device lists no input
  and no output once the last connection is freed, and keeps its lists while an idle
  connection stays. Checked by `tools/call-lab/mic.sh devices` (Linux).

## The system's default microphone and speaker (Windows)

The application takes the first device listed for the system's default (`_first` in
`app/lib/src/call.dart`, `_choose` in `app/lib/src/mic_test.dart`), and the package's
`getUserMedia` without a device records from the first input. On Linux libwebrtc's
PulseAudio device lists the default first. On Windows its CoreAudio device lists the active
endpoints in the order Windows enumerates them, with no default among them: the first may
be any device (a virtual cable's 16-channel input, a phone's virtual microphone), so a call
or a microphone test with "System default" played to, and recorded from, whichever came
first. Only the voice engine's own start (`adm_helpers::Init`) takes Windows' default for
calls (the communications role), and since it stays (above) and calls choose the default
by its id, nothing else did.

- `common/cpp/include/fogata_default_device.h` (new): `DefaultAudioEndpoint` asks Windows
  for the endpoint id of its default input or output for calls
  (`IMMDeviceEnumerator::GetDefaultAudioEndpoint`, `eCommunications`, as libwebrtc's own
  start), on the platform thread, where the runner has started COM; elsewhere, or when
  Windows cannot tell, it is "", and nothing changes. `DefaultDevicePlace` finds that id
  among the devices' guids (libwebrtc gives the same endpoint id), `ListingOrder` puts it
  first. Tested by `common/cpp/test/default_device_test.cc` (in `run_tests.sh`).
- `common/cpp/src/flutter_media_stream.cc`: `getSources` lists the default input and output
  first (a device is still selected by its id, at its own place in libwebrtc's list), and
  `getUserMedia` without a device records from the default input.
- With `FOGATA_CALL_LOG` set, the call's log names the devices of each kind in the order
  listed whenever the system's default is taken (the first).

## Each connection from its own configuration (Linux, Windows)

`createPeerConnection` parsed each connection's configuration over one kept in the plugin
(`configuration_`), so whatever a connection's map left out stayed as the connection
before had it: ICE servers, the ICE transport policy, the bundle policy. After a call
(its media bridge as the only, relay-only TURN server), the settings' microphone test,
whose loop names no ICE server (`app/lib/src/mic_test.dart`), made both its connections
relay-only through a bridge no longer there: they never connected, nothing was recorded,
and the test's meter stayed empty until the application was restarted. The same held for
the loop that reads this device's level while it is alone in a call, once a connection to
someone had been made before it.

- `common/cpp/src/flutter_peerconnection.cc`: `CreateRTCPeerConnection` starts from a
  fresh `RTCConfiguration` each time. macOS already made a new one for each connection.
- Checked by `tools/call-lab/mic.sh devices` (Linux): every microphone test after the call
  must hear the tone.

## What the plugin holds, counted for tests (Linux, Windows)

- `common/cpp/src/flutter_webrtc.cc`: the method `fogataCounts` answers how many
  connections, connection observers, local streams and tracks, capturers, track stoppers,
  renderers and data channels the plugin holds; only the numbers. `tools/call-lab/mic.sh
  devices` checks with it that 20 microphone tests leave nothing more behind and that
  nothing of a call is held once it ended: the process's memory says nothing reliable
  there (over 20 tests it moved by 0.6 to 37 MB with nothing kept, as malloc's free space
  came and went, while 20 connections kept on purpose did not show in it at all). macOS
  answers "not implemented", and the check then counts nothing.

## Upstream 1.6.3

The fork is on 1.6.3: every change of 1.6.2+hotfix.4 to 1.6.3 (its CHANGELOG, and the diff
of the two published packages, file by file) was taken onto the changes above, except
those below. Where a change of 1.6.3 was already here, in an earlier form of Fogata's, the
code is now 1.6.3's: `getSources` off the main thread (#2229, macOS), the encodings
written back by `setParameters` (#2211), the Windows task runner (#2244) and the
renderer's dispose (#2235).

Taken as published, with no change of Fogata's crossing them:

- "captureFrame frees its RGBA buffer and no longer busy-waits" (#2185,
  `common/cpp/src/flutter_frame_capturer.cc` and its header): the application never calls
  `captureFrame`.
- "resolve the plugin instance per engine and free it on engine teardown" (#2221,
  `FlutterWebRTCPlugin.{h,m}`, both copies, identical), with its logger block holding the
  plugin weakly, next to Fogata's copy of the log to standard error (`FOGATA_WEBRTC_LOG`).
  Fogata runs one Flutter engine. The block held the plugin strongly before, so the plugin
  was never freed; now it can be, and its `-dealloc` (the playout watcher stopped, the
  voice engine's keeper and the peer connections closed) can run for the first time, at
  the application's end. Quitting during a call on macOS is to be tried by hand.
- "duplicate null check for video renderer" (#2219, `FlutterRTCVideoRenderer.m`, both
  copies, identical).
- iOS (its podspec and its copy of the Objective-C code), Android (#2230, #2213, Gradle)
  and the web (#2209), none of them built; the version in both podspecs.

Left out, as code Fogata replaced, does not ship or does not use:

- libwebrtc m150.7871.07 and WebRTC-SDK 150.7871.04, which drop the compromised eMudhra
  certificate authority from WebRTC's built-in roots (`third_party/libwebrtc_version.ini`,
  the macOS podspec's WebRTC-SDK, `Package.swift`): Fogata builds its own libwebrtc and
  WebRTC-SDK ("libwebrtc"). Those roots are read only for TURN over TLS (`turns:`), which
  Fogata never asks for: its one TURN server is the media bridge on this computer (`turn:`
  over UDP, `app/lib/src/call/signalling.dart`).
- "let native plugins attach video processors to local tracks" (#2233): a new API
  (`AddVideoProcessor`, `video_sources_`; for eLinux `FlutterWebRTCPluginSharedInstance`
  in `elinux/flutter_webrtc_plugin.cc` and `elinux/flutter_webrtc/flutter_web_r_t_c_plugin.h`,
  and libwebrtc's headers in `elinux/CMakeLists.txt`) that no plugin of Fogata's uses, and that would cross Fogata's changes of
  `flutter_webrtc_base.{h,cc}` and `flutter_media_stream.cc` (a track's stoppers, its
  owner, the voice engine) for nothing.
- The plugin's destructor letting go of `FlutterWebRTC` first (`linux/` and
  `windows/flutter_webrtc_plugin.cc`): the order of the members does it here ("The Linux
  task runner", "The Windows task runner").
- The example and the tests, as for every version.

Not taken, against Fogata's own changes:

- "list the host app's own windows in desktop sources" (#2226, Windows): lists Fogata's own
  windows, which Fogata leaves out on purpose ("Fogata's own windows are not shared"). It
  also moves the listing and every capturer's last release to a thread of its own, with a
  proxy in place of `FlutterScreenCapture` as libwebrtc's observer: the listing on the
  platform thread skipped the application's own windows (their messages wait for that
  thread), which Fogata leaves out anyway, and a capturer's last release could wait for a
  capture of one of the application's own windows, which Fogata never captures. The rest
  (libwebrtc's observers called while the plugin is torn down, at the application's end)
  would need `flutter_screen_capture.cc`'s portal, sound and `capture_tracks_` changes
  rebuilt around that thread. Not worth it for now.
- "list desktop sources off the main thread" (#2228, macOS, and its iOS copy):
  `getDesktopSources`, `updateDesktopSources`, `getDesktopSourceThumbnail` and the source
  lookup of `getDisplayMedia` on a serial queue, as libwebrtc's listing waits for the
  windows' thumbnails (seconds with many windows) on Flutter's UI thread. It would help
  Fogata's screen picker, but crosses its changes of `FlutterRTCDesktopCapturer.m` (both
  copies): its own windows left out by `NSApp.windows`, read on the main thread only, and
  the ScreenCaptureKit path of `getDisplayMedia`, which answers once the capture runs.
  Worth taking as a change of its own, by someone who can open the picker on a Mac with
  many windows and share one of them afterwards: the window numbers read on the main
  thread before the listing goes to the queue, and `getDisplayMedia`'s lookup there too.
