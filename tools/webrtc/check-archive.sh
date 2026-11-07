#!/usr/bin/env bash
# Builds the WebRTC archive of one system again from this commit's sources and patches, in
# a work folder of its own, and checks that it is, byte for byte, the archive this commit
# names (README.md, "Checking an archive"). Nothing is copied into this computer's store.
#
#     tools/webrtc/check-archive.sh linux|macos <work>
#     tools/webrtc/check-archive.sh windows <work> <folder with the toolchain's <hash>.zip>
#     tools/webrtc/check-archive.sh linux|windows|macos --named   (only the SHA-256 named)
#
# linux and windows run on Linux (windows needs the Visual Studio toolchain packed once on
# Windows, WINDOWS.md, checked before anything is fetched); macos runs on a Mac, and gives
# the same bytes only with the Xcode version the archive was built with. <work> must be new
# or one made by fetch.sh for that system: a build takes tens of gigabytes there.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
. "$here/pins.sh"

usage="usage: check-archive.sh linux|macos <work>, or windows <work> <toolchain folder>"
system=${1:?$usage}
work=${2:?$usage}
[[ $work == --named ]] && only_named=1 || only_named=0
build_args=()

case $system in
    linux)
        archive=libwebrtc-linux-x64-release.zip
        key=sha256_linux-x64
        ;;
    windows)
        archive=libwebrtc-win-x64-release.zip
        key=sha256_win-x64
        ;;
    macos)
        archive=WebRTC.xcframework.zip
        ;;
    *)
        echo "$usage" >&2
        exit 2
        ;;
esac

# What this commit names.
if [[ $system == macos ]]; then
    named=$(sed -n "s/^ *sha256 *= *'\([0-9a-f]\{64\}\)'.*/\1/p" "$repo/app/macos/WebRTC-SDK.podspec")
else
    named=$(sed -n "s/^$key *= *\([0-9a-f]\{64\}\).*/\1/p" \
        "$repo/app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini")
fi
if [[ ! $named =~ ^[0-9a-f]{64}$ ]]; then
    echo "this commit names no SHA-256 for the $system archive" >&2
    exit 1
fi
if [[ $only_named == 1 ]]; then
    echo "$named"
    exit 0
fi

# The toolchain the Windows build needs, checked before tens of gigabytes are fetched.
if [[ $system == windows ]]; then
    toolchain=${3:-}
    if [[ -z $toolchain || ! -d $toolchain ]]; then
        echo "$usage" >&2
        exit 2
    fi
    toolchain=$(cd "$toolchain" && pwd)
    zips=("$toolchain"/*.zip)
    if [[ ${#zips[@]} -ne 1 || ! -f ${zips[0]} ]]; then
        echo "$toolchain must hold exactly one toolchain archive (<hash>.zip, WINDOWS.md)" >&2
        exit 1
    fi
    build_args=("$toolchain")
fi

mkdir -p "$work"
work=$(cd "$work" && pwd)
case $system in
    linux) fetch_systems=(linux) ;;
    # Linux's only: build-windows.sh adds Windows' (fetch.sh linux win) with the toolchain
    # from the folder given, which gclient's hook would otherwise ask Google's storage for
    # and be refused.
    windows) fetch_systems=(linux) ;;
    macos) fetch_systems=(mac) ;;
esac
"$here/fetch.sh" "$work" "${fetch_systems[@]}"
# Built into the work folder's own store, never this computer's.
export FOGATA_LIBWEBRTC_DIR=$work/store
"$here/build-$system.sh" "$work" ${build_args[@]+"${build_args[@]}"}

built=$work/dist/$LIBWEBRTC_VERSION/$archive
if [[ ! -f $built ]]; then
    echo "the build made no $built" >&2
    exit 1
fi
if command -v sha256sum > /dev/null; then
    got=$(sha256sum "$built" | cut -c1-64)
else
    got=$(shasum -a 256 "$built" | cut -c1-64)
fi
echo "named by this commit: $named"
echo "built again:          $got"
if [[ $got != "$named" ]]; then
    echo "the $system archive built again is not the one this commit names" >&2
    exit 1
fi
echo "the $system archive is the one this commit names"
