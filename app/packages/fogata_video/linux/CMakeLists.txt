cmake_minimum_required(VERSION 3.10)
set(PROJECT_NAME "fogata_video")
project(${PROJECT_NAME} LANGUAGES CXX)

set(PLUGIN_NAME "fogata_video_plugin")

add_library(${PLUGIN_NAME} SHARED
  "fogata_video_plugin.cc"
)
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
# GTK brings GLib, GObject and gdk-pixbuf. GStreamer is opened at run time (dlopen), so
# nothing of it is linked or needed to build.
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter PkgConfig::GTK ${CMAKE_DL_LIBS})

set(fogata_video_bundled_libraries
  ""
  PARENT_SCOPE
)
