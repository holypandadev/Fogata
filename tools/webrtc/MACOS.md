# WebRTC for macOS

The macOS plugin is written against webrtc-sdk's Objective-C SDK (the AVAudioEngine audio
device module, desktop capture, audio processing...), not the C++ wrapper.
`patches/apple` carries it to WebRTC M156:

1. `0001-webrtc-sdk-objc-sdk`: webrtc-sdk/webrtc's m150_release (2a0b277) moved to
   branch-head 8078 (on top of `patches/webrtc`, which the Mac's build applies first: its
   voice engine keeps webrtc-sdk's own rule for a muted sender, beside `patches/webrtc/0008`), without Android and the tvOS/visionOS build fork, and without what
   Fogata never uses: the frame and data packet cryptor, H.265, its changes to the
   VideoToolbox H.264 encoder and decoder, and its OpenGL view.
2. `0002-vp8-size-gate`: every VP8 decoder of the SDK refuses keyframes larger than 8K before
   decoding them (the wrapper's `vp8_size_gate`, copied in as `//libwebrtc` by
   `prepare.sh`), and `RTCI420Buffer.refusedSize` tells the size a stand-in stands for, which
   the plugin's renderer reads.
3. `0003-m156-build-fixes`: what 0001 needed to build on a Mac: M156's media targets, no
   `HAVE_NO_MEDIA`, and the network thread in place of webrtc-sdk's separate worker thread,
   which M156 no longer has.
4. `0004-opus-and-vp8-only`: the codecs calls use and nothing else, as in the Linux and
   Windows library. The audio codec factories make Opus alone (no G.711, G.722, iLBC, L16
   or multichannel Opus); the default video codec factories, which the plugin's own
   (`FlutterWebRTCPlugin.m`, see the plugin's FOGATA.md) build on, VP8 alone; the
   framework holds no VideoToolbox H.264 encoder or decoder, and VP9, AV1 and H.265 are
   not built (`args/common.gn`). `RTCPeerConnectionFactory`'s plain `init` and the default
   builder get no video codec factory: a factory for video is given VP8's, as the plugin
   does.
5. `0005-desktop-capture-only-with-a-capturer`: the desktop media list and capturer
   start, list and capture only with a capturer WebRTC made, as the C++ wrapper's do
   (`patches/wrapper/0007`): WebRTC makes none where it cannot capture that kind of source,
   and both used the missing one, which ended the program. `RTCDesktopCapturer`'s
   `startCapture` and `startCaptureWithFPS:` answer whether the capture started, and its
   delegate hears that it did only once it has.
6. `0006-capture-desktop-frames-at-the-rate-asked`: the desktop capturer captures at the
   rate asked, as the C++ wrapper's does (`patches/wrapper/0008`): the next capture is due
   one interval after the last one began, not after it ended, screens are not compared
   frame by frame for an updated region nobody reads, and WebRTC's log says every 10 s what
   a capture cost.
7. `0007-encode-a-shared-screen-once`: one encoder for every connection that sends a shared
   screen at one size, as the C++ wrapper's (`patches/wrapper/0017`, built in from
   `//libwebrtc` as `fogata_shared_encoder`, like the size check): the encoder factory given
   to `RTCPeerConnectionFactory` (`initWithAudioDeviceModuleType:...`, which the plugin
   uses, and `initWithEncoderFactory:decoderFactory:audioDevice:`) is wrapped in
   `SharedEncoderFactory`, `ObjCVideoTrackSource` tags the frames of a source marked with
   `RTCVideoSource`'s `enableFogataSharedEncode` (the plugin marks its ScreenCaptureKit
   source), and the field trial `Fogata-SharedScreenEncode/Enabled/` turns it on. Written
   without a Mac at hand: it applies after 0001-0006, but its first build is on a Mac (see
   "Checking patch 7" below).

The macOS framework is all these patches are for: the iOS framework and WebRTC's own
Objective-C tests, which still name the VideoToolbox codecs, are not built from them.

## Build (on a Mac, about 30 GB free, Xcode with the macOS SDK)

    git -C ~/Projects/Fogata-test log -1                # a checkout with tools/webrtc
    tools/webrtc/fetch.sh ~/webrtc-work mac             # about 20 GB
    JOBS=8 tools/webrtc/build-macos.sh ~/webrtc-work    # arm64, then x64

It applies `patches/webrtc` and `patches/apple`, builds `mac_framework_bundle` for arm64 and
x64 (`args/common.gn`, `args/mac.gn`, macOS 12 and later), joins them with `lipo`, checks
that the framework exports no class of a codec other than Opus and VP8 and names no source
file of one (nor of the frame cryptor), makes
`WebRTC.xcframework` with WebRTC's `LICENSE` and the licences of everything built in
(`NOTICE.md`), and writes `~/webrtc-work/dist/fogata-libwebrtc.m156.8078.6/WebRTC.xcframework.zip`
with its SHA-256.

To change the port, fix it in `~/webrtc-work/webrtc/src`, build again with `--no-prepare`,
and save the fix as a patch of `patches/apple` (`git -C ~/webrtc-work/webrtc/src diff` against
the tree `prepare.sh` made).

## Checking patch 7

Patch 7 was written on Linux against the tree the other patches make, and never built
there. On the first Mac build, check:

- that `sdk/BUILD.gn`'s `fogata_shared_encoder` compiles `//libwebrtc/src/internal/shared_encoder.cc`
  with the deps it lists (as `fogata_vp8_size_gate` does), and that `native_video` and
  `peerconnectionfactory_base_objc` find `src/internal/shared_encoder.h` through their
  `include_dirs`;
- that the framework still exports no other codec (`build-macos.sh` checks it);
- with `FOGATA_SHARED_SCREEN_ENCODE=on` in the Mac copy's environment (on macOS the
  plugin leaves the trial out without it, until this has been checked once), in a call
  where the Mac shares its screen to two or three watchers: each watcher's
  `outbound-rtp` names its encoder `... (shared)` (`encoderImplementation`; on macOS the
  real encoder is the `SimulcastEncoderAdapter` the plugin's `VideoEncoderFactorySimulcast`
  makes over libvpx, so the name before " (shared)" may be that adapter's), one
  `SharedScreenEnc` thread runs for each size, every watcher sees the screen, and
  without the variable each connection has its encoder again;
- that a camera is never shared (its connections keep encoders of their own).

## The app on it

The app takes it already: the Podfile names `app/macos/WebRTC-SDK.podspec` in place of
webrtc-sdk's pod, and the plugin's podspec asks for its version. After a new build:

1. `app/macos/WebRTC-SDK.podspec`: its `version` and the archive's SHA-256 (`sha256`).
2. `app/third_party/flutter_webrtc/macos/flutter_webrtc.podspec`: the same version in
   `s.dependency 'WebRTC-SDK', ...`.
3. In `app/macos`: `pod update WebRTC-SDK flutter_webrtc --no-repo-update`. The podspec
   takes the archive from the store `build-macos.sh` copied it into,
   `~/fogata-libwebrtc/<version>/` (`FOGATA_WEBRTC_XCFRAMEWORK` names another one); every Mac
   that builds the app needs it there. The Podfile's own check stops the install once and
   prints the line for `app/macos/WebRTC-SDK.sha256` (the installed files' tree digest):
   record it in place of the old one.
4. Build and test: `mac-build.sh` / `mac-call.sh` of the rigs; a call with Linux (picture
   both ways, sound, screen and its sound), the noise suppression switch, advanced ducking
   in the log, and the giant picture test (`FOGATA_TEST_HOSTILE=giant` with copy b on Linux
   sending 8192 by 8192): the Mac keeps b out at its first such picture, with
   `Dropping video frames of 8192x8192` in its log and no jump in memory.
