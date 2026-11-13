# Fogata's own build of WebRTC

The Linux and Windows app links `libwebrtc`: WebRTC with webrtc-sdk's C++ wrapper
(github.com/webrtc-sdk/libwebrtc), the API the plugin in `app/third_party/flutter_webrtc` is
written against. These scripts build it from source, at the commits `pins.sh` names, with
the patches in `patches/`:

- **WebRTC branch-head 8078**, the branch of Chrome 156. webrtc-sdk's prebuilt releases
  were six milestones behind (m150), without the security fixes Chrome made since.
- **Only the codecs calls use**: VP8 (with its retransmission codec) and Opus. No H.264,
  H.265, VP9, AV1 (encoder or decoder), G.722, PCM or L16 is built in, so none can be
  negotiated and no decoder of one is there for what others send. The app's SDP filters
  (`onlyCallCodecs`, `callCodecs` in `app/lib/src/call.dart`) keep only those two anyway.
- **VP8 pictures larger than 8K are refused before they are decoded** (no side over 8192
  pixels, at most 7680 by 4320 of them). libvpx makes room for a keyframe's size before
  anything else sees it: a peer sending 16383 by 16383 keyframes made each copy take about a
  gigabyte. The size is read from the keyframe's header; the decoder hands on a black 2 by 2
  picture that tells the size instead, and the plugin's renderer tells the app
  (`fogataFrameTooLarge`), which keeps the sender out of the call. Interframes after a
  refused keyframe are refused too until the next keyframe let through.
- **Only the wrapper's API is exported** from the Linux library, as in webrtc-sdk's
  builds (WebRTC M156 has parts in Rust whose bridge would otherwise be exported).
- **Cameras through the Camera portal and PipeWire on Linux**, when asked (the plugin asks
  in a Flatpak): the wrapper opened cameras through V4L2 alone (`/dev/video*`), which a
  Flatpak has only with every device of the computer. It asks xdg-desktop-portal's Camera
  interface once, lists and opens the cameras of the PipeWire connection it gives as they
  come and go, tells whether they were granted, refused or cannot be reached, and keeps the
  id V4L2 gives a camera.
- **A sound server that restarts is connected to again** (Linux): PulseAudio's device kept
  a dead connection after PipeWire's PulseAudio service or PulseAudio restarted (as after
  a system update), listing only the default and playing and recording nothing until the
  application restarted. It now tries again (0.5 s after, then after 1, 2 and 4 s, then
  every 5 s), lists no device meanwhile, takes the chosen devices again by name and starts
  again what was running, and tells the wrapper's `OnDeviceChange` both ways. Connecting and
  every question to the server are given up after 5 s (the first connection waited for
  ever on a server that never answered), and the voice engine stays up without a device
  instead of ending the process.
- **The microphone is processed in one channel**, whatever the other channels of sound
  send: a shared screen's sound starting to go out in stereo made WebRTC process the
  microphone in two channels, and its echo canceller and gain control started over each
  time it started and stopped.
- **A shared screen or window is captured at the rate asked** (Linux X11, Windows and macOS): the
  wrapper waited the whole interval after each capture ended, so a capture taking 20 ms made
  30 frames a second into 19, and it compared every frame with the last for a changed region
  nobody reads. With `FOGATA_WEBRTC_LOG=info` it says every 10 s how many captures it made
  and what one cost.

## Building on Linux (x64)

A work folder outside the repository holds everything; the Linux build takes about 16 GB
for the sources and toolchains and 5 GB for the build, and on 8 cores about 40 minutes the
first time.

    tools/webrtc/fetch.sh ~/webrtc-work linux
    JOBS=8 tools/webrtc/build-linux.sh ~/webrtc-work

`fetch.sh` downloads depot_tools, WebRTC and its dependencies (gclient, with the clang,
Rust and sysroot its hooks fetch) and the wrapper, each at its pinned commit.
`build-linux.sh` resets the tree (`prepare.sh`), applies the patches, builds, runs the
wrapper's tests of the size check and of the microphone's channels, and writes into
`<work>/dist`:

    fogata-libwebrtc.m156.8078.5/libwebrtc-linux-x64-release.zip   (+ .sha256)

laid out like webrtc-sdk's archives (`linux-x64-release/{lib,include,LICENSE,NOTICE.md}`).
`NOTICE.md` holds the licence of WebRTC and of every library built into it, as WebRTC's
own `tools_webrtc/libs/generate_licenses.py` lists them for the target. The same sources
give the same archive, byte for byte (see "Checking an archive").

The plugin's CMake takes the archive named in
`app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini` (`binary_version` and
`sha256_<os>-<arch>`). Every build script copies what it built into this computer's store,
`~/fogata-libwebrtc/<version>/` (`FOGATA_LIBWEBRTC_DIR` names another folder; on Windows
the store is `%USERPROFILE%\fogata-libwebrtc`), and the app's build takes it from there;
`FOGATA_LIBWEBRTC_ARCHIVE` names one archive instead. A computer without it downloads the
published one: each archive is an asset of a release of
[holypandadev/fogata-libwebrtc](https://github.com/holypandadev/fogata-libwebrtc) whose tag
is the version, at `<download_url>/<version>/<archive name>` (the ini's `download_url`). A
new version is published there, with both archives, before the ini names it.

Whatever the place, the archive must have the SHA-256 the ini names.

## Checking an archive

Anyone can check that an archive comes from these sources and patches: build it again
and compare SHA-256s. The compilers, the sysroot and every library come from the commits
`pins.sh` and WebRTC's `DEPS` name, and the archive's files have fixed dates and order, so
the same commit of this repository gives the same archive, byte for byte, in any folder
(and, nothing coming from the computer itself, on any other), with one condition: the
work folder's layout. The library keeps the
names of its source files as the build folder sees them (in its log and its failed
checks), and GN resolves links, so the build folder must see the sources as
`../../webrtc/src`, as `fetch.sh` lays them out: `<work>/webrtc/src` and
`<work>/out/<system>`, real folders, not links. The build scripts stop otherwise. For the
Linux archive:

    git checkout <commit>                    # the one whose libwebrtc_version.ini to check
    tools/webrtc/fetch.sh ~/webrtc-check linux
    FOGATA_LIBWEBRTC_DIR=~/webrtc-check/store tools/webrtc/build-linux.sh ~/webrtc-check
    cat ~/webrtc-check/dist/*/libwebrtc-linux-x64-release.zip.sha256
    grep sha256_linux-x64 app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini

(`FOGATA_LIBWEBRTC_DIR` keeps the copy out of this computer's store.) `check-archive.sh`
does all of it for one system and fails unless the bytes are the same:

    tools/webrtc/check-archive.sh linux ~/webrtc-check     # or macos on a Mac
    tools/webrtc/check-archive.sh windows ~/webrtc-check-win <folder with the toolchain zip>

A check is worth most on another computer than the one that built the archive. The Windows archive
is checked the same way with `build-windows.sh` (the toolchain folder is checked before anything is
fetched; the sources are fetched for Linux, and `build-windows.sh` adds Windows' with that
toolchain, which gclient would otherwise ask Google's storage for); it also depends on the Visual Studio
toolchain packed for it (`WINDOWS.md`), which must be the same Visual Studio and Windows
SDK. The macOS framework is built with the Mac's own Xcode and SDK, which `pins.sh` does
not pin: it is the same, in any folder, only from the same Xcode version (`xcodebuild
-version`).

## Building for Windows (x64)

Cross-compiled on Linux with a Visual Studio toolchain packed once on Windows:
`WINDOWS.md`, `package-win-toolchain.ps1`, `build-windows.sh`.

## Building for macOS

On a Mac, with the Objective-C SDK of webrtc-sdk moved to M156 (`patches/apple`):
`MACOS.md`, `build-macos.sh`, and `app/macos/WebRTC-SDK.podspec` for CocoaPods.

## Changing what is built

- Which milestone a branch-head is: in the fetched tree, `git -C <work>/webrtc/src log -1`
  shows the pinned commit, a merge into that branch titled `[M156] ...` and ending in
  `Cr-Commit-Position: refs/branch-heads/8078@{#n}` and `Cr-Branched-From:` (the commit of
  main the branch started from). Which milestone Chrome's stable channel
  is on is in Chrome's release notes.
- A new WebRTC branch: change `WEBRTC_BRANCH`/`WEBRTC_COMMIT` in `pins.sh`, run `fetch.sh`,
  and port the patches (`prepare.sh` says which one does not apply). Raise
  `LIBWEBRTC_VERSION`, build, and put the new SHA-256 in `libwebrtc_version.ini`.
- A patch: make the change in `<work>/webrtc/src` (or its `libwebrtc/`), build with
  `build-linux.sh <work> --no-prepare`, and save it with `git diff` as the next patch in
  `patches/webrtc` (or, made in the wrapper's own checkout, in `patches/wrapper`).

## The patches

`patches/webrtc` (to WebRTC, every system):

1. `0001-libwebrtc-target`: the wrapper in the default build; the Wayland capture built
   with an older libstdc++ (webrtc-sdk/libwebrtc's own patches).
2. `0002-custom-audio-transport`: the factory takes the `AudioTransport` the wrapper uses
   for custom audio sources (webrtc-sdk/libwebrtc `custom_audio_source_m150.patch`,
   moved to M156).
3. `0003-audio-device-observer`: from webrtc-sdk/webrtc, the audio device module tells
   an observer when devices come and go (the wrapper's `OnDeviceChange`, used on
   Windows), and PulseAudio and ALSA start and stop recording more safely.
4. `0004-windows-shared-mode-rates`: Windows audio devices at 176.4 and 192 kHz
   (webrtc-sdk/libwebrtc's patch for its Windows builds).
5. `0005-pipewire-cameras-coming-and-going`: PipeWire's cameras (the Camera portal's
   path, Linux only) read with PipeWire's loop locked while its thread adds and removes
   them, listed once their formats are known, with the V4L2 bus of those PipeWire reaches
   through V4L2; without PipeWire's library the session stops instead of using it.
6. `0006-pulseaudio-connect-again-to-a-sound-server-that-went-away` (Linux only): the
   PulseAudio device connects again, from WebRTC's worker thread, to a sound server that
   went away (0.5 s after, then after 1, 2 and 4 s, then every 5 s; a server that does not
   answer within 5 s is tried again), lists no device at all meanwhile, takes the chosen
   output and input again by the server's names for them where they are still listed,
   starts playout and recording again as they were, and tells its observer both ways;
   connecting and every question to the server (the mixer's too) are given up after 5 s,
   as a stream starting already was after 9 s.
7. `0007-keep-the-voice-engine-up-without-its-audio-device` (every system):
   `adm_helpers::Init` logs a device that does not initialize instead of ending the
   process.

`patches/apple` (macOS only, after `patches/webrtc`): see `MACOS.md`.

`patches/wrapper` (to webrtc-sdk/libwebrtc):

1. builds against M156 (moved headers and targets, header extension ids);
2. VP8 and Opus only;
3. the VP8 size check, with its tests (`test/vp8_size_gate.test.cc`), and
   `RTCVideoFrame::TooLarge`;
4. the Linux library's exports;
5. its tests built only where WebRTC builds its own;
6. no frame or data packet cryptor;
7. the desktop media list and the desktop capturer start, list and capture only with a
   capturer WebRTC made: it makes none where it cannot capture that kind of source (on
   Linux, windows in a Wayland session or anything when the X display does not open),
   and both used the missing one, which ended the program;
8. the desktop capturer captures at the rate asked: the next capture is due one interval
   after the last one began (it waited the whole interval after the last one ended, so a
   capture taking 20 ms made 30 frames a second into 19), screens are not compared frame by
   frame for an updated region nobody reads, and every 10 s WebRTC's log says at the info
   level what a capture cost (`FOGATA_WEBRTC_LOG=info`);
9. the video device lists and opens cameras through the Camera portal and PipeWire on
   Linux when asked (`SetCameraPortal`, `RequestCameraAccess`, `GetCameraAccess`), V4L2
   standing in only while the portal is unavailable; a camera keeps the id V4L2 gives it
   (its bus), PipeWire is asked for one of the formats the camera offers (the nearest to
   the size and rate wanted), and a device gone since it was counted has no name.
   Elsewhere, and on Linux by default, nothing changes.
10. the audio device's lists are counted on the worker thread, as every other call of the
    audio device module, its device change listener is set and called under a lock (a
    null one stops it), and it is taken off the module as it goes;
11. the microphone is processed in one channel whatever the other channels of sound send
    (`CustomAudioTransportImpl::UpdateAudioSenders`), with its test
    (`test/custom_audio_transport.test.cc`).

webrtc-sdk/webrtc's m150 branch carries much more (Apple audio engine, Android, H.265,
per-track audio processing options, the frame cryptor for end-to-end encryption of
frames); what the C++ wrapper does not need is left out of Linux and Windows, and what
calls never use is left out everywhere.
