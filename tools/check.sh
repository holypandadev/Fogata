#!/usr/bin/env bash
# Checks that must pass before every commit:
#
#     tools/check.sh [--no-advisories] [--no-notices] [--no-codegen]
#
# Formatting, clippy with warnings as errors, every test, licences, bans and sources of the
# crates, known vulnerabilities against the RustSec database on this computer (fetched by
# hand with `cargo deny fetch db`, never online here, so the checks run offline; a database
# whose last advisory is more than 14 days old fails, since it misses what came after), the
# tests of the release scripts and of these checks, of the commit marker and of the CMake
# file that bundles the library, of the copy of libwebrtc's notices, the Linux desktop
# entry, the Linux player's tone mapping and row packing (on Linux also built with ASan and
# UBSan, with the players' other C++ tests), and on a Mac of the Podfile's
# check of WebRTC-SDK and of the framework's codecs, the patches of fuzz/Cargo.toml the same
# as the workspace's and the crates of fuzz/Cargo.lock at versions Cargo.lock has (cargo deny
# runs over the fuzz workspace too, with the advisories), the dependencies between the
# workspace's crates within their layers (tools/crate_layers.py), the app's limits of what a
# person types the same as the Rust ones (tools/limits.py), THIRD-PARTY-NOTICES up to date, and the bridge code
# (flutter_rust_bridge) the same as the API it is generated from. Each --no-... skips one of
# the last checks, for a computer that cannot run it; without it, a check that cannot run
# fails.
set -euo pipefail
cd "$(dirname "$0")/.."

advisories=1
notices=1
codegen=1
for arg in "$@"; do
    case "$arg" in
        --no-advisories) advisories=0 ;;
        --no-notices) notices=0 ;;
        --no-codegen) codegen=0 ;;
        *) echo "usage: tools/check.sh [--no-advisories] [--no-notices] [--no-codegen]" >&2; exit 2 ;;
    esac
done

# What cannot run fails before the long steps.
if [[ $advisories == 1 ]]; then
    newest=
    for db in "${CARGO_HOME:-$HOME/.cargo}"/advisory-dbs/advisory-db-*/; do
        [[ -d $db/crates ]] || continue
        # When its last advisory came, as the database's own history says.
        when=$(git -C "$db" log -1 --format=%ct 2> /dev/null || true)
        if [[ -n $when && ( -z $newest || $when -gt $newest ) ]]; then
            newest=$when
        fi
    done
    if [[ -z $newest ]]; then
        echo "no RustSec database here: fetch it with 'cargo deny fetch db', or skip the check" \
            "of known vulnerabilities with --no-advisories" >&2
        exit 1
    fi
    age=$(( ($(date +%s) - newest) / 86400 ))
    day=$(date -u -d "@$newest" +%Y-%m-%d 2> /dev/null || date -u -r "$newest" +%Y-%m-%d)
    echo "RustSec database of $day ($age days old)"
    if (( age > 14 )); then
        echo "the RustSec database is $age days old: fetch it again with 'cargo deny fetch db'," \
            "or skip the check of known vulnerabilities with --no-advisories" >&2
        exit 1
    fi
fi
if [[ $codegen == 1 ]] && ! command -v flutter_rust_bridge_codegen > /dev/null; then
    echo "flutter_rust_bridge_codegen is not on the PATH: install it, or skip the check of the" \
        "bridge code with --no-codegen" >&2
    exit 1
fi

# The Rust rust-toolchain.toml names, not one an override (RUSTUP_TOOLCHAIN) put in its place.
pinned=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
if [[ "$(rustc -V)" != "rustc $pinned "* ]]; then
    echo "$(rustc -V) is not the Rust rust-toolchain.toml names ($pinned)" >&2
    exit 1
fi
# fuzz/ is a workspace of its own, so its [patch.crates-io] must name every patch of the
# main workspace's (the vendored forks), or its targets fuzz other code than the app ships
# (or do not build at all).
patches() {
    awk '/^\[/ { in_patch = ($0 == "[patch.crates-io]"); next }
        in_patch && NF && $1 !~ /^#/ { gsub(/\.\.\//, ""); gsub(/[ \t]+/, " "); print }' "$1" |
        LC_ALL=C sort
}
if [[ "$(patches Cargo.toml)" != "$(patches fuzz/Cargo.toml)" ]]; then
    echo "[patch.crates-io] of fuzz/Cargo.toml differs from the one of Cargo.toml:" >&2
    diff <(patches Cargo.toml) <(patches fuzz/Cargo.toml) >&2 || true
    exit 1
fi
cargo fmt --all --check
# Every crate but fogata-ffi forbids unsafe code outright (a forbid cannot be allowed again
# further down); the workspace denies it elsewhere, and clippy denies an unsafe block without
# a SAFETY comment. fogata-ffi, which calls the system or is the generated bridge, is allowed
# it on the smallest items. Those are pinned here, per file: how many lines name the
# unsafe_code lint (allow, expect, cfg_attr...) and how many lines use the unsafe keyword
# (blocks, functions, impls, extern blocks), so a new unsafe block inside a module or function
# that already has an allowance changes a number too. The generated bridge code
# (frb_generated.rs) is left out: the check of the bridge code below pins it.
for manifest in crates/*/Cargo.toml; do
    crate=$(dirname "$manifest")
    [[ $crate == crates/fogata-ffi ]] && continue
    for root in "$crate"/src/lib.rs "$crate"/src/main.rs "$crate"/build.rs; do
        if [[ -f $root ]] && ! grep -q '^#!\[forbid(unsafe_code)\]' "$root"; then
            echo "$root does not forbid unsafe code (#![forbid(unsafe_code)])" >&2
            exit 1
        fi
    done
done
unsafe_uses=$(find crates -name '*.rs' ! -name frb_generated.rs -print0 | LC_ALL=C sort -z |
    xargs -0 awk '
        { sub(/\/\/.*/, "") }
        /(^|[^A-Za-z0-9_])unsafe([^A-Za-z0-9_]|$)/ { keyword[FILENAME]++ }
        /unsafe_code/ && !/forbid\(unsafe_code\)/ { lint[FILENAME]++ }
        END {
            for (f in keyword) print "unsafe", keyword[f], f
            for (f in lint) print "unsafe_code", lint[f], f
        }' | LC_ALL=C sort -k3 -k1)
expected_unsafe="unsafe 1 crates/fogata-ffi/src/api/app.rs
unsafe_code 1 crates/fogata-ffi/src/api/app.rs
unsafe 7 crates/fogata-ffi/src/denoise.rs
unsafe_code 2 crates/fogata-ffi/src/denoise.rs
unsafe 2 crates/fogata-ffi/src/disk.rs
unsafe_code 2 crates/fogata-ffi/src/disk.rs
unsafe 3 crates/fogata-ffi/src/idle.rs
unsafe_code 3 crates/fogata-ffi/src/idle.rs
unsafe 15 crates/fogata-ffi/src/idle/x11.rs
unsafe_code 1 crates/fogata-ffi/src/lib.rs
unsafe 1 crates/fogata-ffi/src/logs.rs
unsafe_code 1 crates/fogata-ffi/src/logs.rs"
if [[ $unsafe_uses != "$expected_unsafe" ]]; then
    echo "unsafe code is used or allowed somewhere new (or no longer where it was):" >&2
    echo "$unsafe_uses" >&2
    echo "review it, give each unsafe block a SAFETY comment, and update tools/check.sh" >&2
    exit 1
fi
# The judgment of the level before, which the state tests compare with this one, is
# fogata-state as it was at level one, unchanged (see crates/fogata-state/before/Cargo.toml):
# its sources must hash as they did then (every file's SHA-256, by path, hashed again).
before_digest=$(cd crates/fogata-state/before/src && find . -type f | LC_ALL=C sort |
    xargs shasum -a 256 | shasum -a 256 | cut -d' ' -f1)
if [[ $before_digest != 2a6bcb1c0bbe0484617f1bd3780ec2509ce6cdb721df036c9814276f30282e47 ]]; then
    echo "crates/fogata-state/before/src is not fogata-state as it was at level one" >&2
    exit 1
fi
cargo clippy --workspace --all-targets -- -D warnings
# The command-line program's lab feature (the network lab's tools and test switches, left out
# of the program that ships, and the core's lab code it turns on) is linted and tested too.
cargo clippy -p fogata --all-targets --features lab -- -D warnings
cargo clippy -p fogata-core --all-targets --features lab -- -D warnings
cargo test --workspace --quiet
cargo test -p fogata --features lab --quiet
# The vendored libp2p-mdns, libp2p-identify, libp2p-quic, turn and spake2 are no workspace
# members: their own tests run here. The vendored libp2p-stream has none of its own; fogata-net
# tests its change (tests/bursts.rs).
cargo test -p libp2p-mdns --lib --quiet
cargo test -p libp2p-identify --lib --quiet
cargo test -p libp2p-quic --lib --quiet
cargo test -p turn --lib --quiet
cargo test -p spake2 --lib --quiet
# Warnings are printed (two versions of one crate, an allowance no crate needs), not hidden;
# only errors fail. One line per crate, without the tree of what pulls it in.
cargo deny --offline check --hide-inclusion-graph licenses bans sources
# fuzz/Cargo.lock must hold the crates it shares with Cargo.lock at versions Cargo.lock has,
# so the fuzz targets fuzz what the app ships.
python3 -I -B tools/fuzz_lock.py
# The workspace's crates use only the crates their layer allows (docs/architecture.md), and
# the app's limits of what a person types are the Rust ones.
python3 -I -B tools/crate_layers.py
python3 -I -B tools/limits.py
if [[ $advisories == 1 ]]; then
    cargo deny --offline check --hide-inclusion-graph advisories
    # The fuzz workspace's own lock: its sources and its known vulnerabilities and yanked
    # versions (not its licences: libfuzzer-sys is NCSA, and none of it ships). Here only, on
    # the computer that builds the fuzz targets and so has their crates.
    cargo deny --offline --manifest-path fuzz/Cargo.toml check --hide-inclusion-graph \
        sources advisories
fi
python3 -I -B tools/beta/test_release.py
python3 -I -B tools/beta/test_ci_builds.py
python3 -I -B tools/test_ci.py
python3 -I -B tools/test_native_build.py
python3 -I -B tools/test_third_party_notices.py
python3 -I -B tools/test_desktop_entry.py
python3 -I -B tools/test_repo_checks.py
tools/webrtc/test-check-archive.sh
# The Linux player's HDR tone mapping is plain C++: tested on every system.
app/packages/fogata_video/linux/test/run_tests.sh
# The video players' check of a file's first bytes (Linux and Windows share it).
head_test=$(mktemp)
c++ -std=c++17 -Wall -Wextra -Werror app/packages/fogata_video/common/test/video_head_test.cpp \
    -o "$head_test"
"$head_test" > /dev/null
rm -f "${head_test:?}"
if [[ $(uname) == Linux ]]; then
    # The Linux player's opening of a file: once, a plain file only, nothing left open.
    open_test=$(mktemp)
    c++ -std=c++17 -Wall -Wextra -Werror app/packages/fogata_video/linux/test/open_video_test.cc \
        -o "$open_test"
    "$open_test" app/integration_test/clip.mp4 > /dev/null
    rm -f "${open_test:?}"
    # The same C++ tests again with AddressSanitizer and UndefinedBehaviorSanitizer, any
    # finding fatal: a read past a buffer or an overflow that happens to give the right answer
    # above fails here. (The video renderer's runs under ThreadSanitizer in tools/ci.sh.)
    app/packages/fogata_video/linux/test/run_tests.sh --sanitize
    sanitize=(-g -O1 -fno-omit-frame-pointer -fsanitize=address,undefined -fno-sanitize-recover=all)
    c++ -std=c++17 -Wall -Wextra -Werror "${sanitize[@]}" \
        app/packages/fogata_video/common/test/video_head_test.cpp -o "$head_test"
    "$head_test" > /dev/null
    c++ -std=c++17 -Wall -Wextra -Werror "${sanitize[@]}" \
        app/packages/fogata_video/linux/test/open_video_test.cc -o "$open_test"
    "$open_test" app/integration_test/clip.mp4 > /dev/null
    rm -f "${head_test:?}" "${open_test:?}"
fi
if [[ $(uname) == Darwin ]]; then
    tools/beta/test-macos-updater.sh
    # The Podfile's check of the WebRTC-SDK pod as installed (Ruby comes with CocoaPods).
    ruby app/macos/test/webrtc_sdk_test.rb
    tools/webrtc/test-macos-codecs.sh
    # macOS's video loader: the file opened once, read in bounded answers.
    app/packages/fogata_video/macos/test/run_tests.sh app/integration_test/clip.mp4 > /dev/null
    capture_test=$(mktemp)
    cc -Wall -Werror -I app/third_party/flutter_webrtc/macos/flutter_webrtc/Sources/flutter_webrtc \
        app/third_party/flutter_webrtc/macos/test/capture_size_test.c -o "$capture_test"
    "$capture_test"
    rm -f "${capture_test:?}"
fi
if [[ $notices == 1 ]]; then
    python3 -I -B tools/third_party_notices.py --check ||
        { echo "(--no-notices skips this check)" >&2; exit 1; }
fi
if [[ $codegen == 1 ]]; then
    generated() {
        find crates/fogata-ffi/src/frb_generated.rs app/lib/src/rust -type f -print0 |
            sort -z | xargs -0 sha256sum
    }
    before=$(generated)
    (cd app && flutter_rust_bridge_codegen generate > /dev/null)
    if [[ "$(generated)" != "$before" ]]; then
        echo "the bridge code was out of date with crates/fogata-ffi/src/api: it is generated" \
            "again now; commit it" >&2
        exit 1
    fi
fi
echo "All checks passed."
