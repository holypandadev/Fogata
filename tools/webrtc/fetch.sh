#!/usr/bin/env bash
# Fetches what the WebRTC build needs into a work folder outside the repository, at the
# commits pins.sh names: depot_tools, WebRTC with its dependencies (gclient sync, with the
# toolchains and sysroots its hooks fetch) and webrtc-sdk's C++ wrapper. Run again, it
# brings an existing work folder to the pinned commits.
#
#     tools/webrtc/fetch.sh <work> [linux|win|mac ...]
#
# The systems named (default: linux) are those gclient fetches dependencies for. About
# 16 GB for Linux; Windows and macOS add their own.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.sh"

work=${1:?usage: fetch.sh <work> [linux|win|mac ...]}
shift
oses=("$@")
[[ ${#oses[@]} -gt 0 ]] || oses=(linux)
mkdir -p "$work"
work=$(cd "$work" && pwd)

# Fetches one commit of a repository into <dir> and checks it out, detached.
at_commit() {
    local dir=$1 url=$2 commit=$3
    [[ -d $dir/.git ]] || git init -q "$dir"
    if ! git -C "$dir" cat-file -e "$commit^{commit}" 2>/dev/null; then
        git -C "$dir" fetch -q --depth=1 "$url" "$commit"
    fi
    git -C "$dir" -c advice.detachedHead=false checkout -q -f --detach "$commit"
}

at_commit "$work/depot_tools" "$DEPOT_TOOLS_URL" "$DEPOT_TOOLS_COMMIT"
export DEPOT_TOOLS_UPDATE=0 DEPOT_TOOLS_METRICS=0 PATH="$work/depot_tools:$PATH"
# gn's launcher needs the Python depot_tools bootstraps; its failure to find luci-auth
# (only for Google's own builds) is harmless.
(cd "$work/depot_tools" && ./ensure_bootstrap >/dev/null 2>&1) || true
[[ -f $work/depot_tools/python3_bin_reldir.txt ]] ||
    { echo "depot_tools did not bootstrap" >&2; exit 1; }

mkdir -p "$work/webrtc"
targets=$(printf '"%s", ' "${oses[@]}")
cat > "$work/webrtc/.gclient" <<EOF
solutions = [
  {
    "name": "src",
    "url": "$WEBRTC_URL",
    "deps_file": "DEPS",
    "managed": False,
    "custom_deps": {},
    "custom_vars": {},
  },
]
target_os = [${targets%, }]
EOF
at_commit "$work/webrtc/src" "$WEBRTC_URL" "$WEBRTC_COMMIT"
(cd "$work/webrtc" && gclient sync --no-history --shallow --jobs 8 --force)

at_commit "$work/wrapper" "$WRAPPER_URL" "$WRAPPER_COMMIT"
echo "fetched into $work"
