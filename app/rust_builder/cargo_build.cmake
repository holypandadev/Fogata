# Builds the bridge library with cargo, for the desktop builds (linux/ and windows/
# CMakeLists.txt): cmake -DFOGATA_CARGO=<cargo> -DFOGATA_ROOT=<workspace> -DFOGATA_PROFILE=<dev
# or release> -DFOGATA_TARGET_DIR=<cargo's target folder> [-DFOGATA_CMAKE=<cmake> -DFOGATA_NINJA_DIR=<folder>] -P cargo_build.cmake
#
# It runs while the app is built, so it sees the build's own environment, and leaves out of
# cargo's every variable that would change the code cargo makes, or which compiler or
# toolchain makes it, and says which it left out: RUSTFLAGS would replace the flags of
# .cargo/config.toml (Control Flow Guard on Windows), CARGO_TARGET_<target>_RUSTFLAGS add to
# them, CARGO_PROFILE_RELEASE_* change the release profile, RUSTUP_TOOLCHAIN the Rust that
# rust-toolchain.toml names, and the compiler, flag and CMake variables (CC, CFLAGS,
# CFLAGS_<target>, TARGET_CFLAGS, CL, LINK, CMAKE_TOOLCHAIN_FILE...) reach libopus and AWS-LC,
# which are built with them, or the linker. Only the number of jobs is kept.
set(FOGATA_LEFT_OUT "^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC_BOOTSTRAP|RUSTUP_TOOLCHAIN|CARGO_BUILD_.*|CARGO_TARGET_.*|CARGO_PROFILE_RELEASE_.*|CARGO_UNSTABLE_.*|CARGO_HOST_.*|(TARGET_|HOST_)?(CC|CXX|AR|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|ARFLAGS|ASMFLAGS)(_.*)?|CRATE_CC_.*|CL|_CL_|LINK|_LINK_|ML|_ML_|ML64|CMAKE|CMAKE_.*|AWS_LC_SYS_.*|OPUS_.*)$")

execute_process(COMMAND "${CMAKE_COMMAND}" -E environment OUTPUT_VARIABLE FOGATA_ENVIRONMENT)
# No name holds these, and in a value they would split or join CMake's list items.
foreach(FOGATA_CHARACTER ";" "[" "]")
  string(REPLACE "${FOGATA_CHARACTER}" " " FOGATA_ENVIRONMENT "${FOGATA_ENVIRONMENT}")
endforeach()
string(REGEX MATCHALL "(^|\n)[^=\n]+=" FOGATA_NAMES "${FOGATA_ENVIRONMENT}")
set(FOGATA_CLEARED "")
foreach(FOGATA_ENTRY IN LISTS FOGATA_NAMES)
  string(REGEX REPLACE "^\n?(.*)=$" "\\1" FOGATA_NAME "${FOGATA_ENTRY}")
  # Windows does not tell names apart by case.
  string(TOUPPER "${FOGATA_NAME}" FOGATA_UPPER)
  if(FOGATA_UPPER MATCHES "${FOGATA_LEFT_OUT}" AND NOT FOGATA_UPPER STREQUAL "CARGO_BUILD_JOBS")
    unset(ENV{${FOGATA_NAME}})
    list(APPEND FOGATA_CLEARED "${FOGATA_NAME}")
  endif()
endforeach()
if(FOGATA_CLEARED)
  string(REPLACE ";" ", " FOGATA_CLEARED "${FOGATA_CLEARED}")
  message(STATUS "cargo builds the Fogata library without: ${FOGATA_CLEARED}")
endif()

# On Windows, the CMake that builds the app and the Ninja beside it build libopus.
if(FOGATA_CMAKE)
  set(ENV{CMAKE} "${FOGATA_CMAKE}")
  set(ENV{CMAKE_GENERATOR} "Ninja")
endif()
if(FOGATA_NINJA_DIR)
  file(TO_NATIVE_PATH "${FOGATA_NINJA_DIR}" FOGATA_NINJA_DIR)
  set(ENV{PATH} "${FOGATA_NINJA_DIR};$ENV{PATH}")
endif()

execute_process(
  COMMAND "${FOGATA_CARGO}" build -p fogata-ffi --locked --profile "${FOGATA_PROFILE}"
          --target-dir "${FOGATA_TARGET_DIR}"
  WORKING_DIRECTORY "${FOGATA_ROOT}"
  RESULT_VARIABLE FOGATA_RESULT
)
if(NOT FOGATA_RESULT EQUAL 0)
  message(FATAL_ERROR "cargo build of the Fogata library failed (${FOGATA_RESULT})")
endif()
