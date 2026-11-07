# The Flutter tooling requires that developers have a version of Visual Studio
# installed that includes CMake 3.14 or later. You should not increase this
# version, as doing so will cause the plugin to fail to compile for some
# customers of the plugin.
cmake_minimum_required(VERSION 3.14)

# Project-level configuration.
set(PROJECT_NAME "rust_lib_fogata")
project(${PROJECT_NAME} LANGUAGES CXX)

# The bridge library is built with cargo straight from the workspace, as on Linux and macOS:
# with the crates Cargo.lock names (--locked), and nothing else fetched or run for the build.
#
# opusic-sys builds libopus with CMake. Visual Studio's CMake, which builds this and need not
# be on the PATH, is given to it (CMAKE), with the Ninja beside it (on the PATH, and as the
# generator): a Visual Studio project built from inside this build, which is one too, fails
# (MSB4018 in CMake's compiler checks), and Ninja runs the compiler directly.
if(CMAKE_VERSION VERSION_LESS "3.25")
  message(FATAL_ERROR "Building the Fogata library needs CMake 3.25 or later (Visual Studio 2022 has it)")
endif()
get_filename_component(FOGATA_CMAKE_BIN "${CMAKE_COMMAND}" DIRECTORY)
find_program(FOGATA_NINJA ninja HINTS "${FOGATA_CMAKE_BIN}/../../Ninja")
if(NOT FOGATA_NINJA)
  message(FATAL_ERROR "Ninja not found beside ${CMAKE_COMMAND} nor on the PATH: install Visual Studio's \"C++ CMake tools for Windows\"")
endif()
get_filename_component(FOGATA_NINJA_DIR "${FOGATA_NINJA}" DIRECTORY)

# This folder is reached through Flutter's link to the plugin
# (windows/flutter/ephemeral/.plugin_symlinks/rust_lib_fogata/windows), or as itself.
set(FOGATA_ROOT "")
foreach(FOGATA_CANDIDATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../../../../.."
    "${CMAKE_CURRENT_SOURCE_DIR}/../../..")
  get_filename_component(FOGATA_CANDIDATE "${FOGATA_CANDIDATE}" ABSOLUTE)
  if(NOT FOGATA_ROOT AND EXISTS "${FOGATA_CANDIDATE}/crates/fogata-ffi/Cargo.toml")
    set(FOGATA_ROOT "${FOGATA_CANDIDATE}")
  endif()
endforeach()
if(NOT FOGATA_ROOT)
  message(FATAL_ERROR "No Fogata workspace (crates/fogata-ffi) above ${CMAKE_CURRENT_SOURCE_DIR}")
endif()

find_program(FOGATA_CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{USERPROFILE}/.cargo/bin")
if(NOT FOGATA_CARGO)
  message(FATAL_ERROR "cargo not found: install Rust with rustup (https://rustup.rs)")
endif()

# Debug builds take cargo's debug profile; Profile and Release builds its release profile.
set(FOGATA_CARGO_PROFILE "$<IF:$<CONFIG:Debug>,dev,release>")
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
set(FOGATA_CARGO_OUT "${FOGATA_TARGET}/$<IF:$<CONFIG:Debug>,debug,release>")
set(FOGATA_FFI_LIB "${CMAKE_CURRENT_BINARY_DIR}/$<CONFIG>/fogata_ffi.dll")
# cargo runs without the variables that would change the code it makes (cargo_build.cmake).
add_custom_command(
  OUTPUT "${FOGATA_FFI_LIB}" "${CMAKE_CURRENT_BINARY_DIR}/_always_"
  COMMAND ${CMAKE_COMMAND} -E remove -f "${FOGATA_CARGO_OUT}/fogata_ffi.dll"
  COMMAND ${CMAKE_COMMAND} "-DFOGATA_CARGO=${FOGATA_CARGO}" "-DFOGATA_ROOT=${FOGATA_ROOT}"
          "-DFOGATA_TARGET_DIR=${FOGATA_TARGET}"
          "-DFOGATA_PROFILE=${FOGATA_CARGO_PROFILE}" "-DFOGATA_CMAKE=${CMAKE_COMMAND}"
          "-DFOGATA_NINJA_DIR=${FOGATA_NINJA_DIR}" -P "${FOGATA_ROOT}/app/rust_builder/cargo_build.cmake"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "${FOGATA_CARGO_OUT}/fogata_ffi.dll" "${FOGATA_FFI_LIB}"
  WORKING_DIRECTORY "${FOGATA_ROOT}"
  VERBATIM
)
# Never made, so cargo runs on every build and decides itself what changed.
set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/_always_" PROPERTIES SYMBOLIC TRUE)
add_custom_target(${PROJECT_NAME}_cargo ALL DEPENDS "${FOGATA_FFI_LIB}")

# List of absolute paths to libraries that should be bundled with the plugin.
set(rust_lib_fogata_bundled_libraries
  "${FOGATA_FFI_LIB}"
  PARENT_SCOPE
)
