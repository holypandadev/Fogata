#!/usr/bin/env bash
# Puts the WebRTC tree of a work folder (made by fetch.sh) back to the pinned commit and
# applies Fogata's patches: patches/webrtc to WebRTC, then webrtc-sdk's C++ wrapper copied
# into it as libwebrtc/ with patches/wrapper on top. Any change made by hand in that
# tree is lost.
#
#     tools/webrtc/prepare.sh <work> [extra patch folder ...]
#
# Extra folders (patches/apple for macOS) are applied to WebRTC after patches/webrtc.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.sh"

work=${1:?usage: prepare.sh <work> [extra patch folder ...]}
shift
src=$work/webrtc/src
[[ -d $src/.git && -d $work/wrapper/.git ]] ||
    { echo "$work has no WebRTC tree: run fetch.sh first" >&2; exit 1; }

git -C "$src" -c advice.detachedHead=false checkout -q -f --detach "$WEBRTC_COMMIT"
# Removes what the patches added; the dependencies gclient put there are ignored by git
# and stay.
git -C "$src" clean -q -fd
rm -rf "${src:?}/libwebrtc"

apply_all() {
    local dir=$1 into=$2
    local patch
    for patch in "$dir"/*.patch; do
        [[ -e $patch ]] || continue
        git -C "$into" apply --whitespace=nowarn "$patch" ||
            { echo "$(basename "$patch") does not apply" >&2; exit 1; }
    done
}

apply_all "$here/patches/webrtc" "$src"
for extra in "$@"; do
    apply_all "$extra" "$src"
done

mkdir "$src/libwebrtc"
git -C "$work/wrapper" archive "$WRAPPER_COMMIT" | tar -x -C "$src/libwebrtc"
git -C "$work/wrapper" -c advice.detachedHead=false checkout -q -f --detach "$WRAPPER_COMMIT"
for patch in "$here"/patches/wrapper/*.patch; do
    [[ -e $patch ]] || continue
    git -C "$src" apply --whitespace=nowarn --directory=libwebrtc "$patch" ||
        { echo "$(basename "$patch") does not apply" >&2; exit 1; }
done
echo "prepared $src"
