# Everything the WebRTC build is made from, pinned by commit. Sourced by the other scripts.

# Chromium's tools: gclient, gn, ninja and the toolchains they fetch. Newer than the
# depot_tools WebRTC's DEPS names (b1c2e867 on branch-head 8127), which gclient fetches
# into the tree for the hooks; this one only has to read that DEPS.
DEPOT_TOOLS_URL=https://chromium.googlesource.com/chromium/tools/depot_tools.git
DEPOT_TOOLS_COMMIT=8defb316a8fccb3645e5bcf3ed078b757106c857

# WebRTC branch-head 8127, the branch of Chrome 158: the commit of 158.0.8127.37, released
# on the Stable channel for Mac and Windows on 2026-11-12 (chromiumdash, fetch_releases;
# Linux was still on 157 on 2026-11-16).
# gclient fetches its dependencies (build, third_party, clang, sysroots) at the commits
# its DEPS file names.
WEBRTC_URL=https://webrtc.googlesource.com/src.git
WEBRTC_BRANCH=refs/branch-heads/8127
WEBRTC_COMMIT=fb1c41a93d92d4188f068f437d80ef2c4b50c3a9

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
LIBWEBRTC_VERSION=fogata-libwebrtc.m158.8127.2
