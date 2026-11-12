#!/usr/bin/env bash
# Builds and runs the native tests of the plugin's shared C++ code (Linux): the video
# renderer's, a connection's received tracks' and the retired textures' with
# ThreadSanitizer, the check of the noise suppression table, which outputs a shared
# screen's sound takes, the portal's frame conversion, and libwebrtc
# taking a local offer with a section rejected as call.dart rejects one, its voice engine
# stopping with the last connection unless one is kept (voice_engine_test.cc), screen
# sharing's choice of desktop and libwebrtc's screen and window lists (on an Xvfb display of its own
# when Xvfb is there), and the way the cameras are reached (camera_portal_test.cc; its part
# through the Camera portal runs in tools/beta/test-flatpak-camera.sh). Needs
# libwebrtc's headers and library, which the first Linux build of the app unpacks into
# third_party/libwebrtc (or give LIBWEBRTC=<folder with include/ and lib/>), Flutter's
# Linux engine headers (from the Flutter on PATH) and GTK 3's.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
webrtc=${LIBWEBRTC:-$root/third_party/libwebrtc}
[ -e "$webrtc/include/libwebrtc.h" ] || {
  echo "no libwebrtc in $webrtc: build the app for Linux once first" >&2
  exit 1
}
out=$(mktemp -d)
trap 'rm -rf "${out:?}"' EXIT
lib=$(dirname "$(find "$webrtc/lib" -name libwebrtc.so | head -n 1)")
engine=$(dirname "$(readlink -f "$(command -v flutter)")")/cache/artifacts/engine/linux-x64
"${CXX:-clang++}" -std=c++17 -g -O1 -fsanitize=thread \
  -I"$root/common/cpp/include" -I"$root/linux/flutter/include" -I"$webrtc/include" \
  -I"$engine" $(pkg-config --cflags gtk+-3.0) -DRTC_DESKTOP_DEVICE \
  "$here/video_renderer_test.cc" "$root/common/cpp/src/flutter_video_renderer.cc" \
  -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread -o "$out/video_renderer_test"
TSAN_OPTIONS="halt_on_error=1 exitcode=66" "$out/video_renderer_test"
# A connection's received streams and tracks, added on libwebrtc's signalling thread while
# the platform thread looks them up (fogata_remote_tracks.h): ThreadSanitizer, plain C++.
"${CXX:-clang++}" -std=c++17 -g -O1 -Wall -Werror -fsanitize=thread -I"$root/common/cpp/include" \
  "$here/remote_tracks_test.cc" -lpthread -o "$out/remote_tracks_test"
TSAN_OPTIONS="halt_on_error=1 exitcode=66" "$out/remote_tracks_test"
# Unregistered textures kept until the engine is done with them, while a raster thread
# copies (linux/flutter/retired_textures.h): ThreadSanitizer, plain C++.
"${CXX:-clang++}" -std=c++17 -g -O1 -Wall -Werror -fsanitize=thread -I"$root/linux/flutter" \
  "$here/retired_textures_test.cc" -lpthread -o "$out/retired_textures_test"
TSAN_OPTIONS="halt_on_error=1 exitcode=66" "$out/retired_textures_test"
"${CXX:-clang++}" -std=c++17 -g -O1 -I"$root/common/cpp/include" -I"$webrtc/include" \
  "$here/denoise_table_test.cc" -ldl -o "$out/denoise_table_test"
"$out/denoise_table_test"
# What a track's id stands for by its owner (fogata_track_owner.h): plain C++.
"${CXX:-clang++}" -std=c++17 -g -O1 -Wall -Werror -I"$root/common/cpp/include" \
  "$here/track_owner_test.cc" -o "$out/track_owner_test"
"$out/track_owner_test"
# Which outputs a shared screen's sound takes (linux/screen_sound_node.h): plain C++.
"${CXX:-clang++}" -std=c++17 -g -O1 -Wall -Werror -I"$root/linux" \
  "$here/screen_sound_node_test.cc" -o "$out/screen_sound_node_test"
"$out/screen_sound_node_test"
# The portal's frames turned into I420 (linux/portal_i420.h), byte for byte as the loop it
# replaced, built as a release build is; `portal_i420_test 300` also times it.
"${CXX:-clang++}" -std=c++17 -O2 -Wall -Werror -I"$root/linux" \
  "$here/portal_i420_test.cc" -o "$out/portal_i420_test"
"$out/portal_i420_test"
"${CXX:-clang++}" -std=c++17 -g -O1 -I"$webrtc/include" "$here/rejected_section_test.cc" \
  -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread -o "$out/rejected_section_test"
# The audio module libwebrtc makes at start must reach the sound server (it stops the
# program otherwise), but no track is added: nothing is played or recorded, and no camera
# is opened.
"$out/rejected_section_test"
# The voice engine stops with the last connection, and the audio device's lists with it,
# unless an idle connection is kept (KeepVoiceEngine): the same sound server, no track.
"${CXX:-clang++}" -std=c++17 -g -O1 -I"$webrtc/include" "$here/voice_engine_test.cc" \
  -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread -o "$out/voice_engine_test"
"$out/voice_engine_test"
# Which desktop is a Wayland session, and libwebrtc's screen and window lists where it can
# make no capturer and on an X display of its own (Xvfb, when there is one): the same sound
# server as above.
"${CXX:-clang++}" -std=c++17 -g -O1 -I"$root/linux" -I"$webrtc/include" -DRTC_DESKTOP_DEVICE \
  $(pkg-config --cflags gtk+-3.0) \
  "$here/desktop_session_test.cc" "$root/linux/desktop_session.cc" \
  -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread $(pkg-config --libs gtk+-3.0) \
  -o "$out/desktop_session_test"
if command -v Xvfb > /dev/null; then
  Xvfb -displayfd 3 -screen 0 1024x768x24 -nolisten tcp 3> "$out/display" 2> /dev/null &
  xvfb=$!
  # Up to 20 s: on a busy computer (a CI runner building other things) Xvfb takes seconds.
  for _ in $(seq 200); do [ -s "$out/display" ] && break; sleep 0.1; done
  display=$(tr -dc 0-9 < "$out/display")
  if [ -z "$display" ]; then
    kill "$xvfb" 2> /dev/null
    echo "Xvfb did not start: no X display for the desktop session test" >&2
    exit 1
  fi
  FOGATA_TEST_X_DISPLAY=":$display" "$out/desktop_session_test" || {
    kill "$xvfb"
    exit 1
  }
  kill "$xvfb"
else
  "$out/desktop_session_test"
fi
# Which way the cameras are reached, and cameras opened directly asking nothing; no camera
# is listed or opened.
"${CXX:-clang++}" -std=c++17 -g -O1 -I"$root/linux" -I"$webrtc/include" -DRTC_DESKTOP_DEVICE \
  $(pkg-config --cflags glib-2.0) "$here/camera_portal_test.cc" "$root/linux/camera_access.cc" \
  -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread $(pkg-config --libs glib-2.0) \
  -o "$out/camera_portal_test"
env -u FOGATA_TEST_CAMERA_PORTAL "$out/camera_portal_test"
