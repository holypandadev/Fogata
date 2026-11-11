cmake_minimum_required(VERSION 3.10)
set(PROJECT_NAME "flutter_webrtc")
project(${PROJECT_NAME} LANGUAGES CXX)

if (CMAKE_VERSION VERSION_GREATER_EQUAL "3.24.0")
  cmake_policy(SET CMP0135 NEW)
endif()

# Add the libwebrtc dependency
include("${CMAKE_CURRENT_SOURCE_DIR}/../third_party/CMakeLists.txt")

set(PLUGIN_NAME "${PROJECT_NAME}_plugin")
set (CMAKE_EXPORT_COMPILE_COMMANDS ON )
set(CMAKE_CXX_STANDARD 17)

add_definitions(-DRTC_DESKTOP_DEVICE)

add_library(${PLUGIN_NAME} SHARED
  "../common/cpp/src/flutter_data_channel.cc"
  "../common/cpp/src/flutter_media_stream.cc"
  "../common/cpp/src/flutter_utf8_sanitize.cc"
  "../common/cpp/src/flutter_peerconnection.cc"
  "../common/cpp/src/flutter_frame_capturer.cc"
  "../common/cpp/src/flutter_video_renderer.cc"
  "../common/cpp/src/flutter_screen_capture.cc"
  "../common/cpp/src/flutter_webrtc.cc"
  "../common/cpp/src/flutter_webrtc_base.cc"
  "../common/cpp/src/flutter_common.cc"
  "loopback_capturer_factory.cc"
  "portal_screen_capturer.cc"
  "desktop_session.cc"
  "pipewire_apps_capturer.cc"
  "silent_input.cc"
  "input_watcher.cc"
  "input_name.cc"
  "output_keeper.cc"
  "test_camera.cc"
  "camera_access.cc"
  "flutter_webrtc_plugin.cc"
  "flutter/core_implementations.cc"
  "flutter/standard_codec.cc"
  "flutter/plugin_registrar.cc"
  "task_runner_linux.cc"
)

include_directories(
  "${CMAKE_CURRENT_SOURCE_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}/flutter/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/../common/cpp/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/libwebrtc/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/svpng"
)

apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}/../common/cpp/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/libwebrtc/include"
)
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter)
target_link_libraries(${PLUGIN_NAME} PRIVATE PkgConfig::GTK)

# GCC 14+ (and notably GCC 16) tightened transitive header inclusion, so the
# bundled libwebrtc headers (e.g. third_party/libwebrtc/include/rtc_types.h)
# now fail to compile because they use fixed-width integer types such as
# uint32_t without including <cstdint>. Force-include <cstdint> for every
# translation unit of this target so those headers build under strict
# toolchains, instead of editing the downloaded third-party headers in place.
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
  target_compile_options(${PLUGIN_NAME} PRIVATE -include cstdint)
endif()

# ---------------------------------------------------------------------------
# libpulse: the sound server's inputs and outputs (input_watcher.cc,
# output_keeper.cc, input_name.cc). A shared screen's sound is PipeWire's only
# (loopback_capturer_factory.cc).
# On Manjaro/.41 the dev headers come from the base `libpulse` package.
# ---------------------------------------------------------------------------
find_package(PkgConfig)
if(PkgConfig_FOUND)
  pkg_check_modules(PULSE IMPORTED_TARGET libpulse libpulse-simple)
endif()
if(TARGET PkgConfig::PULSE)
  message(STATUS "flutter_webrtc: libpulse found -- watching the sound server.")
  target_compile_definitions(${PLUGIN_NAME} PRIVATE HAVE_LIBPULSE)
  target_link_libraries(${PLUGIN_NAME} PRIVATE PkgConfig::PULSE)
elseif(CMAKE_BUILD_TYPE STREQUAL "Debug")
  message(WARNING "flutter_webrtc: libpulse NOT found -- the sound server is not watched (debug build only). Install the libpulse dev package.")
else()
  # Fogata: without libpulse the application could not tell that no sound server
  # answers (input_watcher.cc), and WebRTC's voice engine ends the process then.
  message(FATAL_ERROR "flutter_webrtc: libpulse NOT found. Fogata's release and profile builds need its dev package (libpulse-dev): without it a call on a computer with no sound server would end the application.")
endif()


# Fogata: screen capture through the xdg-desktop-portal and PipeWire, used under
# Wayland; a shared screen's sound and the silent input that keeps the microphone closed
# while it is off come with it. Without the PipeWire headers a debug build keeps
# libwebrtc's own capture; a release or profile build stops, as for libpulse above: it
# would pass every other check and go out without all three.
if(PkgConfig_FOUND)
  pkg_check_modules(PORTAL_CAPTURE IMPORTED_TARGET libpipewire-0.3 gio-2.0 gio-unix-2.0)
endif()
if(TARGET PkgConfig::PORTAL_CAPTURE)
  target_compile_definitions(${PLUGIN_NAME} PRIVATE HAVE_PIPEWIRE)
  target_link_libraries(${PLUGIN_NAME} PRIVATE PkgConfig::PORTAL_CAPTURE)
elseif(CMAKE_BUILD_TYPE STREQUAL "Debug")
  message(WARNING "flutter_webrtc: libpipewire-0.3 NOT found -- screen capture under Wayland uses libwebrtc's own, with no screen sound and no silent input (debug build only). Install the PipeWire dev package.")
else()
  message(FATAL_ERROR "flutter_webrtc: libpipewire-0.3 NOT found. Fogata's release and profile builds need its dev package (libpipewire-0.3-dev): without it the application has no screen capture under Wayland, no screen sound and no silent input.")
endif()

target_link_libraries(${PLUGIN_NAME} PRIVATE 
  "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/libwebrtc/lib/libwebrtc.so"
)

# List of absolute paths to libraries that should be bundled with the plugin
set(flutter_webrtc_bundled_libraries
  "${CMAKE_CURRENT_SOURCE_DIR}/../third_party/libwebrtc/lib/libwebrtc.so"
  PARENT_SCOPE
)

# Add $ORIGIN to RPATH so that lib/libflutter_webrtc_plugin.so can find lib/libwebrtc.so at runtime
set_property(
    TARGET ${PLUGIN_NAME}
    PROPERTY BUILD_RPATH
    "\$ORIGIN"
)
