# flutter_webrtc in Fogata

This is flutter_webrtc 1.6.2+hotfix.4 from pub.dev (MIT, see LICENSE), without its
`example` and `test` folders. Fogata keeps its own copy to change how the screen is
captured on Linux. Every change from the published package is listed here.

## Screen capture under Wayland (Linux)

libwebrtc's own capture under Wayland asks the portal from its source list as well as from
the capturer (two dialogs, and a hidden second capture that keeps running), offers no
windows, may deliver every frame marked as corrupted, and crashed when asked for a window
list (before `tools/webrtc/patches/wrapper/0007`). Instead, `getDisplayMedia` without a source id, in a Wayland session:

- `linux/portal_screen_capturer.{h,cc}` (new): asks the xdg-desktop-portal ScreenCast
  interface once, for screens and windows, with the cursor drawn in; opens the stream it
  gives in PipeWire, in plain memory; turns each frame into I420, at most the asked frame
  rate. The session bus and the portal's properties are reached without blocking the
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
  (`Stream/Output/Audio`), reading each output's owner from its full properties, and
  leaves out this application's own (same process or same executable). It feeds 10 ms
  frames of 48 kHz stereo to the loopback audio source. A link it made is let go
  (`pw_proxy_destroy`) when either of its ports goes away. For tests,
  `FOGATA_SCREEN_SOUND_FROM` names the one program whose sound is taken (debug builds).
- `linux/loopback_capturer_factory.cc`: uses it when built with PipeWire; where PipeWire
  cannot connect (PulseAudio itself is the sound server) and libpulse is there, the
  package's PulseAudio monitor capture instead.
- `common/cpp/src/flutter_screen_capture.cc`: the portal path (Wayland) captures this
  sound too when `getDisplayMedia` asks for audio, and stops it with the picture.
- `linux/silent_input.{h,cc}` (new) and the method `fogataSilentInput` in
  `common/cpp/src/flutter_webrtc.cc`: a source of silence, "Fogata silence (<pid>)"
  (node `fogata-silence-<pid>`): a null sink that PipeWire runs as a virtual source,
  owned by the app's connection so it goes away with the app. Named after the process,
  so the application picks its own and never another running Fogata's, which goes away
  with that one. `fogataSilentInput` answers true only once the server has made the
  node (a sync after the request, 2 s at most); a PipeWire without the null sink factory
  answers false at once.
  WebRTC records from its input whenever any channel of sound sends; while the
  microphone is off and a shared screen's sound goes out, the application points WebRTC
  here so the microphone stays closed.
- Both connect to PipeWire's own socket, which a Flatpak gets only when given it
  (`--filesystem=xdg-run/pipewire-0`, in tools/beta/flatpak.sh): with only PulseAudio's,
  the screen's sound falls back to the monitor capture above and the silent input is not
  made.

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

## Sound of a shared window (Windows)

The package's `ApplicationLoopbackCapturer` takes, for a shared window (the id it is listed
by is its window handle), the sound of the window's process and its children only
(`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`). Asked for in the render endpoint's
floating-point format, that capture started and never got a packet, so a shared window
went with no sound at all.

- `windows/application_loopback_capturer.cc`: one process's capture asks for plain 16-bit
  PCM at 48 kHz, two channels; a whole screen's keeps the endpoint's format.
- `windows/loopback_capturer_factory.cc`: a window whose process cannot be found (closed
  meanwhile, an id that is no window) gets no sound at all; the package took the whole
  system's sound then.
- `windows/test/loopback_test.cc` and `run_tests.ps1` (new): build with Visual Studio's
  tools against the libwebrtc a Windows build unpacks, find a window by its title, check
  the plugin lists it by its handle, and take its sound while another program plays
  (none may come) and once its own program plays too (it must); whoever runs it starts
  those programs, playing into a virtual output.

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
  downloaded first). Fogata's archives are not published: the build takes them from this
  computer's store, `<home>/fogata-libwebrtc/<version>/` (or `FOGATA_LIBWEBRTC_DIR`), where
  `tools/webrtc` puts what it builds, or from `FOGATA_LIBWEBRTC_ARCHIVE`, naming one archive.
  The ini's `download_url`, once they are published, is the last place tried.
- The archive carries `NOTICE.md`, the licences of everything built into the library, and
  the wrapper's `LICENSE`. `tools/third_party_notices.py` puts them into THIRD-PARTY-NOTICES
  from the copies kept in `tools/notices/libwebrtc` (so it also runs on a Mac, which never
  unpacks the archive), and fails when the unpacked archive's files differ from the copies:
  copy the new ones over when libwebrtc changes.
- macOS (`macos/flutter_webrtc.podspec`): WebRTC-SDK 156.8078.5, Fogata's own build of the
  same WebRTC (`tools/webrtc/MACOS.md`), which the app's Podfile takes from
  `app/macos/WebRTC-SDK.podspec` (this computer's store, checked by SHA-256). iOS
  (`ios/flutter_webrtc.podspec`) and `macos/flutter_webrtc/Package.swift` (Swift Package
  Manager, which the app does not use) still name webrtc-sdk's 150.7871.04.

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
- `FlutterRTCVideoRenderer.m` (the three copies, identical; macOS): `renderFrame:` drops
  such frames and tells the first of a track, and `setSize:` makes no pixel buffer for
  such a size. With Fogata's own WebRTC framework (`tools/webrtc/MACOS.md`), whose VP8
  decoder refuses such keyframes before decoding them, a stand-in's buffer gives the size
  refused (`refusedSize`, read by name so WebRTC-SDK's builds, which lack it, still work):
  it is taken as the frame's size, so the stand-in is dropped and told the same way.

## The renderer's picture buffer (Linux, Windows)

The renderer made its `FlutterDesktopPixelBuffer` again at the first frame of every track it
was given, on the thread frames come on and outside its lock, while the raster thread could
be reading the one `CopyPixelBuffer` had just returned (a stream shown in place of another).

- `common/cpp/src/flutter_video_renderer.cc`: `initialize` makes it once; it lives as long
  as the renderer, and only `CopyPixelBuffer` changes it, under the lock.
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
  sink appears again); the system's default leaves streams where they open.
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
- `common/cpp/src/flutter_media_stream.cc`: a camera refused through the portal, or no
  portal to reach any (and none opened directly), fails `getUserMedia` with
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

- `lib/src/native/event_errors.dart` (new, exported): `onWebRtcEventError`, set once by the
  application (`lib/main.dart`, which notes it in the day's log), gets every such error with
  what it was about; without it the error is printed.
- `RTCPeerConnectionNative.errorListener`: reports the error, and the first one makes the
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
own errors now go on to its listeners, each of which logs them. Tested by
`app/test/webrtc_events_test.dart`.
