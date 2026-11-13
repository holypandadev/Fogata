# Native components and their security releases

Fogata's Rust crates are checked with `cargo deny` (`tools/check.sh`: licences, bans, sources
and, once the RustSec advisory database is on the computer, advisories). The parts below are
native code that no Rust tool sees. Each one parses what other people send (media, pictures,
videos), so its security releases matter. Check them before every release and whenever a
browser or system security update is announced.

| Component | Where it comes from | Version now | How to check |
|---|---|---|---|
| libwebrtc (Linux, Windows) | Fogata's own build (`tools/webrtc`, see its README): WebRTC at a branch-head pinned by commit in `tools/webrtc/pins.sh`, with webrtc-sdk's C++ wrapper and the patches in `tools/webrtc/patches`; the archive pinned by SHA-256 in `app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini` | M156, branch-head 8078 (`fogata-libwebrtc.m156.8078.6`) | Compare the milestone with Chrome's stable releases and their security fixes (Chrome Releases blog, "Stable Channel Update for Desktop"): a WebRTC fix there is in a milestone (M156, M157...), and the branch-heads of WebRTC take its merges (`git log` of `refs/branch-heads/<n>` at webrtc.googlesource.com). Move `WEBRTC_COMMIT` to the branch-head's newest commit for fixes in the same milestone, and to the next branch-head when Chrome moves to a new milestone; then build again and record the new SHA-256. Anyone can check that an archive comes from those sources and patches by building it again (`tools/webrtc/README.md`, "Checking an archive"): the same layout gives the same bytes. Only VP8 and Opus are built in, so advisories about other codecs (H.264, H.265, VP9, AV1, G.711, G.722, iLBC...) do not apply to it. |
| WebRTC-SDK (macOS) | Fogata's own build (`tools/webrtc`, see `MACOS.md`): the same WebRTC as libwebrtc (`tools/webrtc/pins.sh`), with webrtc-sdk's Objective-C SDK moved to it (`tools/webrtc/patches/apple`); CocoaPods takes it from `app/macos/WebRTC-SDK.podspec`, which checks the archive's SHA-256, and the Podfile checks the installed files against `app/macos/WebRTC-SDK.sha256` after every `pod install` | M156, branch-head 8078 (156.8078.6) | Kept with libwebrtc above: the same `WEBRTC_COMMIT`, built again on a Mac with `tools/webrtc/build-macos.sh`; then record the archive's SHA-256 in the podspec and the line the Podfile prints in `WebRTC-SDK.sha256`. As on Linux and Windows, Opus and VP8 are the only codecs built in (`tools/webrtc/patches/apple/0004`): the framework has no G.711, G.722, iLBC, L16, H.264 (VideoToolbox's included), H.265, VP9 or AV1 encoder or decoder, so advisories about those codecs do not apply to it either. Advisories about Apple's own media frameworks are in the macOS rows below. |
| Flutter engine (Skia/Impeller, picture codecs, text) | The Flutter SDK used to build | 3.47.8 stable | Flutter's release notes and security advisories (the flutter/flutter GitHub security page); Chrome security fixes in Skia or the image codecs often apply too. Build releases with the newest stable patch release. |
| GStreamer (Linux video player) | The system's packages, opened at run time | Whatever the distribution ships | The distribution's security notices for `gstreamer1.0-*` (for example Ubuntu USN, Debian DSA) and GStreamer's own security page. People update it with their system; the app needs no change. |
| Media Foundation (Windows video player) | Windows | Windows' own | Microsoft's monthly security updates (Patch Tuesday); people update it with Windows. |
| AVFoundation and system codecs (macOS) | macOS | macOS' own | Apple security releases; people update it with macOS. |

What the app does to keep this code's exposure small is described in
`docs/architecture.md`, 3.10 (for example, a picture is decoded only if its header promises a
bounded size, and an animation only up to 2048 by 2048).

## Screen capture on Windows

libwebrtc captures a shared screen with DXGI desktop duplication, and with GDI where that
cannot start (a computer or virtual machine without a GPU that duplicates: a virtual
machine's display-only adapter, a VirtIO one say, is one, where a 1280x800 capture costs 25 to 80 ms);
a shared window with Windows.Graphics.Capture. Windows.Graphics.Capture for screens
(`allow_wgc_screen_capturer`) was considered and left off, as WebRTC leaves it by default:
on some Windows 10 builds it draws a yellow border around the screen, and it needs a
recent Windows.
