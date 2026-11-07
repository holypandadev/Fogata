cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "fogata_video")
project(${PROJECT_NAME} LANGUAGES CXX)
cmake_policy(VERSION 3.14...3.25)

set(PLUGIN_NAME "fogata_video_plugin")

add_library(${PLUGIN_NAME} SHARED
  "include/fogata_video/fogata_video_plugin_c_api.h"
  "fogata_video_plugin_c_api.cpp"
  "fogata_video_plugin.cpp"
  "fogata_video_plugin.h"
  "video_probe.cpp"
  "video_probe.h"
)
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
# Media Foundation and WIC come with Windows.
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin
  mfplat mfreadwrite mfuuid windowscodecs shlwapi ole32 oleaut32)

set(fogata_video_bundled_libraries
  ""
  PARENT_SCOPE
)
