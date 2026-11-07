#!/bin/sh
# Builds the bridge library for the macOS app (the podspec's script phase, in Xcode's build
# environment) with cargo straight from the workspace, as on Linux and Windows: with the Rust
# of rust-toolchain.toml and the crates Cargo.lock names (--locked), and nothing else fetched
# or run for the build. One static library per architecture Xcode builds (ARCHS: both for a
# release), joined with lipo into $BUILT_PRODUCTS_DIR/libfogata_ffi.a.
#
# Xcode's environment need not have the user's PATH: cargo is looked for in rustup's place
# first, and CMake (opusic-sys builds libopus with it) in Homebrew's and CMake.app's when it
# is not on the PATH; CMAKE in the environment names it otherwise.
set -eu

# Physical paths: this folder is reached through Flutter's link to the plugin.
HERE=$(cd "$(dirname "$0")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
if [ ! -f "$ROOT/crates/fogata-ffi/Cargo.toml" ]; then
  echo "error: no Fogata workspace (crates/fogata-ffi) at $ROOT" >&2
  exit 1
fi

# Xcode's own tools (iOS linkers among them) are not for cargo's build scripts.
PATH=$(printf '%s' "$PATH" | tr ':' '\n' | grep -v 'Contents/Developer/' | paste -s -d ':' -)
export PATH

CARGO="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
if [ ! -x "$CARGO" ]; then
  CARGO=$(command -v cargo || true)
fi
if [ -z "$CARGO" ]; then
  echo "error: cargo not found: install Rust with rustup (https://rustup.rs)" >&2
  exit 1
fi

if [ -z "${CMAKE:-}" ]; then
  for candidate in "$(command -v cmake || true)" /opt/homebrew/bin/cmake /usr/local/bin/cmake \
      /Applications/CMake.app/Contents/bin/cmake; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      CMAKE=$candidate
      break
    fi
  done
fi
if [ -z "${CMAKE:-}" ]; then
  echo "error: CMake not found (it builds libopus): brew install cmake, or set CMAKE" >&2
  exit 1
fi
export CMAKE

case "${CONFIGURATION:-Release}" in
  Debug*) PROFILE=dev; OUT=debug ;;
  *) PROFILE=release; OUT=release ;;
esac

# What would change the code cargo makes (RUSTFLAGS, and the C flags that would reach
# libopus) is left out of its environment.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER CFLAGS CXXFLAGS

# From the workspace, so its .cargo/config.toml and rust-toolchain.toml apply.
cd "$ROOT"
# Where cargo builds: CARGO_TARGET_DIR when it is set (a relative one counts from the
# workspace), else target/ there. It is handed to cargo as --target-dir, so no configuration
# file or variable sends the libraries anywhere else, and the ones joined below are the ones
# these runs of cargo made.
case "${CARGO_TARGET_DIR:-}" in
  "") TARGET="$ROOT/target" ;;
  /*) TARGET="$CARGO_TARGET_DIR" ;;
  *) TARGET="$ROOT/$CARGO_TARGET_DIR" ;;
esac
set --
for arch in ${ARCHS:-$(uname -m)}; do
  case "$arch" in
    arm64) triple=aarch64-apple-darwin ;;
    x86_64) triple=x86_64-apple-darwin ;;
    *) echo "error: no Rust target for architecture $arch" >&2; exit 1 ;;
  esac
  if command -v rustup > /dev/null 2>&1 || [ -x "$HOME/.cargo/bin/rustup" ]; then
    rustup=$(command -v rustup || echo "$HOME/.cargo/bin/rustup")
    if ! "$rustup" target list --installed | grep -qx "$triple"; then
      # The toolchain rust-toolchain.toml pins, which a plain `rustup target add` run
      # outside the workspace would miss; a release builds both Mac architectures.
      toolchain=$("$rustup" show active-toolchain | cut -d' ' -f1)
      echo "error: the Rust target $triple is not installed: rustup target add" \
        "aarch64-apple-darwin x86_64-apple-darwin --toolchain $toolchain" >&2
      exit 1
    fi
  fi
  # An old library at the end of the path is removed first, so a cargo that built somewhere
  # else anyway (build.target in a configuration file) makes the lipo below fail instead of
  # joining the old one.
  rm -f "$TARGET/$triple/$OUT/libfogata_ffi.a"
  "$CARGO" build -p fogata-ffi --locked --target-dir "$TARGET" --profile "$PROFILE" --target "$triple"
  set -- "$@" "$TARGET/$triple/$OUT/libfogata_ffi.a"
done

mkdir -p "${BUILT_PRODUCTS_DIR:?}"
lipo -create "$@" -output "$BUILT_PRODUCTS_DIR/libfogata_ffi.a"

# Inputs that change on every build, so Xcode runs this phase every time and cargo decides
# what changed.
ln -fs "${OBJROOT:-}/XCBuildData/build.db" "$BUILT_PRODUCTS_DIR/rust_phony"
ln -fs "$BUILT_PRODUCTS_DIR/${EXECUTABLE_PATH:-}" "$BUILT_PRODUCTS_DIR/rust_phony_out"
