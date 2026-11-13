# Everything the WebRTC build is made from, pinned by commit. Sourced by the other scripts.

# Chromium's tools: gclient, gn, ninja and the toolchains they fetch.
DEPOT_TOOLS_URL=https://chromium.googlesource.com/chromium/tools/depot_tools.git
DEPOT_TOOLS_COMMIT=8defb316a8fccb3645e5bcf3ed078b757106c857

# WebRTC branch-head 8078, the branch of Chrome 156: the commit of 156.0.8078.82, released
# on the Stable channel for Mac on 2026-11-01 (chromiumdash, fetch_releases; 157 and 158 were
# on that channel too).
# gclient fetches its dependencies (build, third_party, clang, sysroots) at the commits
# its DEPS file names.
WEBRTC_URL=https://webrtc.googlesource.com/src.git
WEBRTC_BRANCH=refs/branch-heads/8078
WEBRTC_COMMIT=19576e37870bfe5a0a910a32f98ff411f722f43a

# webrtc-sdk's C++ wrapper (libwebrtc.m150.7871.07), the API the Linux and Windows
# plugin is written against.
WRAPPER_URL=https://github.com/webrtc-sdk/libwebrtc.git
WRAPPER_COMMIT=4515f9ccdca75e2454e204f326f1d7129cad678e

# webrtc-sdk's WebRTC branch the patches in patches/ were taken from (m150_release), for
# reference: the scripts never fetch it.
SDK_URL=https://github.com/webrtc-sdk/webrtc.git
SDK_COMMIT=2a0b27762f1e05d7ac86c1c36fc2b51cca7fe229

# The name of what this builds, as libwebrtc_version.ini of the plugin names it. Raise the
# last number with every change of the patches or the arguments.
LIBWEBRTC_VERSION=fogata-libwebrtc.m156.8078.6
