# flutter_webrtc in Fogata

This is flutter_webrtc 1.6.2+hotfix.4 from pub.dev (MIT, see LICENSE), without its
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
pub.dev has a newer one, unless the line below names it.

Reviewed against flutter_webrtc 1.6.3 (2026-11-12): every change it brings was read; what was
taken and what was not, and why, is in "Upstream 1.6.3" at the end.

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
  buffer was kept already) and `Refresh` gives it again, under PipeWire's loop lock.
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
or more), which wait for each other by spinning. Each person watching a camera or a screen has
an encoder of their own, so a call of four with cameras at 540p ran nine such threads: on a
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
  lowers the screen's rate (`ScreenLoad` in `lib/src/call/fitting.dart`). Nothing here can
  give the screen more threads than the cameras: `VideoStreamEncoder` reads the field trial
  once, from the factory's (or a whole connection's) field trials, and hands the same
  limit to every encoder; the wrapper's API makes no encoder factory of the plugin's own;
  and a desktop source is not marked as a screen (`is_screencast()` is false in the
  wrapper's `ScreenCapturerTrackSource`, and the portal's capture is a custom source), so
  WebRTC encodes it as a camera. A limit of its own for the screen needs a patch of
  WebRTC (`tools/webrtc/patches/webrtc`: in `VideoStreamEncoder`, another limit when the
  content type is a screen) and the desktop sources marked as screens (a wrapper patch,
  and the portal's source), then new libwebrtc builds for every system.

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
  again when it comes back). A list that cannot be read changes nothing. A selection the
  audio device refuses (it may while recording) keeps the place it had, and the next
  change in the list or the next selection tries again.
- `common/cpp/src/flutter_media_stream.cc`: `getUserMedia` and `selectAudioInput` select
  through `SelectRecordingDevice`.
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
itself is opened with WebRTC's own noise suppressor off.

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

- `common/cpp/src/flutter_peerconnection.cc`: `updateRtpParameters` changed copies of the
  encodings and never put them back, so `setParameters` dropped every change to an
  encoding (scale, frame rate, active). It now sets them back with `set_encodings`.
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
  against a PipeWire server the test plays itself (`pipewire_fake.cc`, linked in place of
  libpipewire; no daemon is reached): which outputs are linked as the server tells of them
  in any order (the call's own sound made again during a share never is), requests carried
  out in the order asked whatever the server's delays, and PipeWire's thread against the
  platform thread stopping the capture.
- `linux/loopback_capturer_factory.cc`: uses it when built with PipeWire, and only for a
  whole screen: a window's sound cannot be narrowed to its application here, and every
  other one's would go with it. Without PipeWire, or where it cannot connect (PulseAudio
  itself is the sound server), a shared screen has no sound: the package's PulseAudio
  monitor capture (removed) carried this application's own call sound back to everyone,
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
  node gone with the old connection.
  WebRTC records from its input whenever any channel of sound sends; while the
  microphone is off and a shared screen's sound goes out, the application points WebRTC
  here so the microphone stays closed.
- Both connect to PipeWire's own socket, which a Flatpak gets only when given it
  (`--filesystem=xdg-run/pipewire-0`, in tools/beta/flatpak.sh): with only PulseAudio's,
  a shared screen goes without sound and the silent input is not made.

## The microphone's processing while a screen's sound goes out (Linux, Windows)

WebRTC has one audio processing module, for what the microphone records, and sets it from
the source of each channel of sound when that channel starts sending (a track given to
it, a new description). The package made a screen's sound source with echo cancelling,
gain control and noise removal off, so while a shared screen's sound went out the
microphone had none of them either (seen in libwebrtc's log: `aec: 0, agc: 0` from the
moment someone watched until the share stopped). The screen's sound itself never goes
through that module.

- `common/cpp/src/flutter_screen_capture.cc`, `common/cpp/include/flutter_screen_capture.h`:
  `getDisplayMedia` makes the screen's sound source with the processing its `audio` map
  names (`echoCancellation`, `autoGainControl`, `noiseSuppression`; off for any left out,
  and with `audio: true`), in both the portal path and the other one. The application
  names the microphone's settings. A change of the microphone's settings during a share
  reaches the screen's sound source only with the next share: until then, its channel
  starting to send again (someone else watching) puts back the settings it was made with.

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

## Devices listed off the main thread (macOS)

Taken from upstream 1.6.3 (its "discover capture devices off the main thread in
getSources"):

- `FlutterRTCMediaStream.m` (both copies, identical): `getSources` finds the cameras and
  asks the audio module for its inputs and outputs on a queue of the system's, and answers
  on the main thread. The first camera discovery of a process loads the system's camera
  extensions and waits for the camera daemon (about a second), and the audio module's
  lists wait for WebRTC's worker thread; on the main thread, Flutter's UI thread on macOS,
  the window froze meanwhile (the settings opened, a call started, a device plugged in).

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
    default input or output change (after 600 ms of quiet, past the module's own 500 ms), when
    `getUserMedia` takes a microphone and when one is chosen (`FlutterRTCMediaStream.m`,
    both copies, identical); a microphone that became usable is opened at once.
  - As the module's observer (unless an embedding plugin registered one): when the engine
    stops while playout goes on, it looks once the module's change is done, and again
    500 ms later; an engine still stopped while the module counts itself as playing is
    released and made again (three times in a row at most). When an engine that stopped
    was to record (the module's `isRecordingEnabled`, the state it was changing to), it
    is made with the speaker alone, and the input is not tried again until one of the
    changes above. When every engine that stopped was only to play, the input was not
    the cause (a speaker that went to sleep or changed its profile): it is made again as
    it was, the microphone left as it is. Without the observer (another one was set), no
    stop is heard, and the input is taken for the cause.
  - Its work runs on a queue of its own: the module's getters (`isPlaying`,
    `isEngineRunning`) and setters wait for its thread (WebRTC-SDK's `RTCAudioDeviceModule`
    calls the module with `BlockingCall` on the factory's network thread, which is also
    WebRTC's worker thread, where every change of the engine runs from start to end), up
    to two seconds while an engine starts. So the check 500 ms after a stop never sees a
    start half done. `stop` waits for the queue before the factory goes.
  - The log says "Fogata: microphone not usable, playing only: ...", "microphone usable",
    "microphone could not be opened" and "the audio engine stopped while playing; made
    again without the microphone" (or "as it was").
- `FlutterWebRTCPlugin.m` (both copies, identical; macOS only): made with the factory,
  stopped in `dealloc`.
- A headless check outside the app (the module of `app/macos/WebRTC-SDK.podspec`, its
  input refused through the observer, platform voice processing off so no microphone is
  opened): alone, the module goes on counting itself as playing with its engine stopped,
  and `startPlayout` changes nothing; with this, the engine runs again within a second,
  recording asked later leaves it running, and a microphone that fails again is handled
  the same way.

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

- `common/cpp/src/flutter_frame_cryptor.cc`, `flutter_data_packet_cryptor.cc` and their
  headers (Linux, Windows) and `FlutterRTCFrameCryptor.{h,m}`, `FlutterDataPacketCryptor.{h,m}`
  (macOS, both copies) are removed, with their method handlers and key provider maps
  (`flutter_webrtc.{h,cc}`, `flutter_webrtc_base.{h,cc}`, `FlutterWebRTCPlugin.{h,m}`) and
  the sources in `linux/`, `windows/` and `elinux/` `CMakeLists.txt`. Their methods now
  answer "not implemented"; the Dart classes (`frame_cryptor_impl.dart`,
  `data_packet_cryptor_impl.dart`) stay as the package has them, unused.

## libwebrtc (Linux, Windows)

The package downloads webrtc-sdk's prebuilt libwebrtc. Fogata builds its own instead
(`tools/webrtc` at the root of the repository, see its README): WebRTC M156 (branch-head
8078) with webrtc-sdk's C++ wrapper, the same API, with VP8 and Opus as the only codecs
and VP8 keyframes larger than 8K refused before they are decoded (see "Pictures too large
to draw").

- `third_party/libwebrtc_version.ini`: `fogata-libwebrtc.m156.8078.5`, with the SHA-256 of
  every archive (`sha256_<os>-<arch>`); only the systems Fogata ships are listed.
- `third_party/CMakeLists.txt`: the archive must match that SHA-256, wherever it comes
  from; archives are named after the version, and a stamp in `libwebrtc/` makes a new
  version or archive be extracted again (the package kept using whatever archive it had
  downloaded first). The build takes them from this computer's store,
  `<home>/fogata-libwebrtc/<version>/` (or `FOGATA_LIBWEBRTC_DIR`), where `tools/webrtc`
  puts what it builds, or from `FOGATA_LIBWEBRTC_ARCHIVE`, naming one archive; the ini's
  `download_url`, the releases of holypandadev/fogata-libwebrtc, is the last place tried.
- The archive carries `NOTICE.md`, the licences of everything built into the library, and
  the wrapper's `LICENSE`. `tools/third_party_notices.py` puts them into THIRD-PARTY-NOTICES
  from the copies kept in `tools/notices/libwebrtc` (so it also runs on a Mac, which never
  unpacks the archive), and fails when the unpacked archive's files differ from the copies:
  copy the new ones over when libwebrtc changes.
- macOS (`macos/flutter_webrtc.podspec`): WebRTC-SDK 156.8078.5, Fogata's own build of the
  same WebRTC (`tools/webrtc/MACOS.md`), which the app's Podfile takes from
  `app/macos/WebRTC-SDK.podspec` (this computer's store, checked by SHA-256). iOS
  (`ios/flutter_webrtc.podspec`, never built) still names webrtc-sdk's 150.7871.04.
- `macos/flutter_webrtc/Package.swift` and `ios/flutter_webrtc/Package.swift` are removed:
  they declared webrtc-sdk's WebRTC.xcframework 150.7871.04 as a binary, and a build with
  Swift Package Manager on would link it beside Fogata's (the app turns it off in
  `app/pubspec.yaml`, which `tools/test_repo_checks.py` pins; without a Package.swift the
  plugin is built through its podspec even if that setting were lost).

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
  lost (the sound server restarted) is made again 1 s later, then after 2, 4... up to
  30 s between tries; it kept a dead connection and moved nothing until the application
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

WebRTC's voice engine, made with a call's first connection, checks that its audio device
starts (`adm_helpers.cc`: `RTC_CHECK_EQ(0, adm->Init())`, also in release builds) and ends
the process otherwise. On Linux Fogata's libwebrtc takes the PulseAudio device whenever libpulse loads (it has ALSA too, never used then), so with no
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
  the same way and ends the application), saying why;
  the room and its chat stay (`tools/call-lab/mic.sh broken`, `alsa` and `hang` check it).
- `linux/CMakeLists.txt`: a build other than Debug fails without libpulse's headers, as the
  check would always say "unknown" and the voice engine could end the application.

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

Taken from upstream 1.6.3 (its "run task runner tasks without holding the queue lock",
and the order the plugin's members are freed in):

- `windows/task_runner_windows.cc`: `ProcessTasks` ran each task with the queue's lock
  held, so a task that queued another locked the same `std::mutex` again on the platform
  thread. Fogata's own tasks do: a shared window that closes (`OnStop` in
  `common/cpp/src/flutter_screen_capture.cc`, posted to the platform thread) sends
  `mediaStreamTrackEnded`, and every event of the plugin's channels goes through the
  runner. The lock is now held only to take a task.
- `windows/flutter_webrtc_plugin.cc`: the runner is declared before the plugin's
  `FlutterWebRTC`, so it is freed after it, as on Linux: freed first, it was gone while
  `FlutterWebRTC` was torn down and posted its last events through it.

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

Taken from upstream 1.6.3 (its "stop VideoRendererDispose from releasing a renderer
twice"):

- `common/cpp/src/flutter_video_renderer.cc`, `common/cpp/include/flutter_video_renderer.h`:
  `videoRendererDispose` takes the renderer out of the manager's map at once, on the
  platform thread, and `FlutterVideoRenderer::Dispose` stops its frames and closes its
  event channel then, while the texture id is still its own. The renderer itself still
  lives until the engine has let go of its texture (above, and on Windows the engine's
  own callback). On Windows the map entry went in that callback, on the raster thread,
  while the platform thread used the map, and a second dispose of the same texture
  before it erased the same entry twice. Everywhere the event channel went only with
  the renderer: an id given again meanwhile to a new renderer's texture (the id is the
  texture's address: the engine's on Windows, the plugin's proxy on Linux) had its
  channel's handler cleared by the old renderer's going. `common/cpp/test/video_renderer_test.cc`
  (Linux) checks that a disposed renderer has left its track and closed its channel.

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
- A sound server restarted while the application runs is not reached again by the audio
  device (Linux), as before for a server restarted before the first call ended; now it
  is not reached again at all until the application restarts. libwebrtc's PulseAudio
  device connects once, in `Init`, and keeps a dead context after the server goes (its
  lists then hold only "default", and nothing plays or records); the wrapper's API has no
  way to initialize the device again. The only one is to let the engine stop (the
  plugin's idle connection let go while no other is open) and start again with the next
  connection, which is not done: `adm_helpers::Init` ends the process (`RTC_CHECK`) if
  the server refuses then, and `InitPulseAudio` waits with no limit for one that takes
  the connection and never answers, on libwebrtc's worker thread; the devices chosen
  also go back to the system's defaults. Doing it safely needs the device's own
  `Terminate`/`Init` reachable from the wrapper with a bounded wait (a wrapper and WebRTC
  patch), or proof on Linux that the stop and start are safe when `InputWatcher` has
  just connected again. To see it: in `tools/call-lab/session.sh <folder> bash`, start a
  debug build with `FOGATA_WEBRTC_LOG=info`, run the settings' microphone test, restart
  the session's `pipewire-pulse` (or `pipewire` and all three), then open the settings
  again and run the test: by the code, the lists should hold "System default" alone and
  the meter stay empty until the application restarts (not yet seen on a computer).
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

The fork stays on 1.6.2+hotfix.4: re-applying every change above to a new base costs far
more than taking the few of 1.6.3 that apply. Each change of 1.6.3 (its CHANGELOG, and the
diff of the two published packages, file by file):

Taken, each with its own section above:

- "run task runner tasks without holding the queue lock" (#2244): Windows, in "The Windows
  task runner". Linux's runner is Fogata's own ("The Linux task runner"), which already
  runs each task with the queue let go.
- The plugin's task runner freed after `FlutterWebRTC` (`windows/flutter_webrtc_plugin.cc`):
  Windows, in "The Windows task runner"; Linux's already was.
- "stop VideoRendererDispose from releasing a renderer twice" (#2235): "A renderer's
  dispose", on Linux too (its event channel).
- "discover capture devices off the main thread in getSources" (#2229): "Devices listed
  off the main thread (macOS)".

Already here in another form:

- "write encoding changes back in RtpSender setParameters" (#2211): "Sender parameters".

Left out, as code Fogata replaced, does not ship or does not use:

- libwebrtc m150.7871.07 and WebRTC-SDK 150.7871.04, which drop the compromised eMudhra
  certificate authority from WebRTC's built-in roots (`third_party/libwebrtc_version.ini`,
  the podspecs, `Package.swift`): Fogata builds its own libwebrtc and WebRTC-SDK
  ("libwebrtc"). Those roots are read only for TURN over TLS (`turns:`), which Fogata never
  asks for: its one TURN server is the media bridge on this computer (`turn:` over UDP,
  `app/lib/src/call/signalling.dart`).
- "let native plugins attach video processors to local tracks" (#2233): a new API
  (`AddVideoProcessor`, `video_sources_`) no plugin of Fogata's uses.
- "captureFrame frees its RGBA buffer and no longer busy-waits" (#2185): the application
  never calls `captureFrame`.
- "resolve the plugin instance per engine and free it on engine teardown" (#2221), with its
  logger block holding the plugin weakly: Fogata runs one Flutter engine, for as long as
  the process.
- "duplicate null check for video renderer" (#2219): no change of behaviour (a message to
  nil answers nil in Objective-C).
- Android (#2230, #2213, Gradle), iOS, the web (#2209), eLinux and the example.

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
- "list desktop sources off the main thread" (#2228, macOS): `getDesktopSources`,
  `updateDesktopSources`, `getDesktopSourceThumbnail` and the source lookup of
  `getDisplayMedia` on a serial queue, as libwebrtc's listing waits for the windows'
  thumbnails (seconds with many windows) on Flutter's UI thread. It would help Fogata's
  screen picker, but crosses its changes of `FlutterRTCDesktopCapturer.m` (both copies):
  its own windows left out by `NSApp.windows`, read on the main thread only, and the
  ScreenCaptureKit path of `getDisplayMedia`, which answers once the capture runs. Worth
  taking as a change of its own, by someone who can open the picker on a Mac with many
  windows and share one of them afterwards: the window numbers read on the main thread
  before the listing goes to the queue, and `getDisplayMedia`'s lookup there too.
