# The Flutter tooling requires that developers have CMake 3.10 or later
# installed. You should not increase this version, as doing so will cause
# the plugin to fail to compile for some customers of the plugin.
cmake_minimum_required(VERSION 3.10)

# Project-level configuration.
set(PROJECT_NAME "rust_lib_fogata")
project(${PROJECT_NAME} LANGUAGES CXX)

# The bridge library is built with cargo straight from the workspace, as on Windows and
# macOS: with the crates Cargo.lock names (--locked) and nothing else fetched or run for the
# build. Rust from the distribution's packages works too.
get_filename_component(FOGATA_HERE "${CMAKE_CURRENT_SOURCE_DIR}" REALPATH)
get_filename_component(FOGATA_ROOT "${FOGATA_HERE}/../../.." ABSOLUTE)

# Where cargo builds: CARGO_TARGET_DIR when it is set (a relative one counts from the workspace,
# where cargo runs), else target/ in the workspace. It is handed to cargo as --target-dir, so
# no configuration file or variable sends the library anywhere else, and the library copied
# below is the one this run of cargo made. An old one at the end of that path is removed
# first: if cargo still built somewhere else (build.target in a configuration file), the copy
# fails instead of taking the old library.
if(DEFINED ENV{CARGO_TARGET_DIR} AND NOT "$ENV{CARGO_TARGET_DIR}" STREQUAL "")
  get_filename_component(FOGATA_TARGET "$ENV{CARGO_TARGET_DIR}" ABSOLUTE BASE_DIR "${FOGATA_ROOT}")
else()
  set(FOGATA_TARGET "${FOGATA_ROOT}/target")
endif()
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(FOGATA_CARGO_PROFILE "dev")
  set(FOGATA_CARGO_OUT "${FOGATA_TARGET}/debug")
else()
  set(FOGATA_CARGO_PROFILE "release")
  set(FOGATA_CARGO_OUT "${FOGATA_TARGET}/release")
endif()

# cargo runs without the variables that would change the code it makes (cargo_build.cmake).
set(FOGATA_FFI_LIB "${CMAKE_CURRENT_BINARY_DIR}/libfogata_ffi.so")
add_custom_command(
  OUTPUT "${FOGATA_FFI_LIB}" "${CMAKE_CURRENT_BINARY_DIR}/_always_"
  COMMAND ${CMAKE_COMMAND} -E remove -f "${FOGATA_CARGO_OUT}/libfogata_ffi.so"
  COMMAND ${CMAKE_COMMAND} -DFOGATA_CARGO=cargo "-DFOGATA_ROOT=${FOGATA_ROOT}"
          "-DFOGATA_TARGET_DIR=${FOGATA_TARGET}"
          -DFOGATA_PROFILE=${FOGATA_CARGO_PROFILE}
          -P "${FOGATA_ROOT}/app/rust_builder/cargo_build.cmake"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "${FOGATA_CARGO_OUT}/libfogata_ffi.so" "${FOGATA_FFI_LIB}"
  WORKING_DIRECTORY "${FOGATA_ROOT}"
  VERBATIM
)
add_custom_target(${PROJECT_NAME}_cargo ALL DEPENDS "${FOGATA_FFI_LIB}")

# List of absolute paths to libraries that should be bundled with the plugin.
set(rust_lib_fogata_bundled_libraries
  "${FOGATA_FFI_LIB}"
  PARENT_SCOPE
)
