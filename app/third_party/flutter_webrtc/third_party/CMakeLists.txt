include(ExternalProject)

# Flutter adds plugins to example builds through .plugin_symlinks. Windows
# CMake refuses to extract archives through that symlink path, so resolve the
# real third_party directory before downloading or extracting libwebrtc.
get_filename_component(LIBWEBRTC_THIRD_PARTY_DIR "${CMAKE_CURRENT_LIST_DIR}" REALPATH)

# Load binary version + download URL template from libwebrtc_version.ini so
# bumping the prebuilt release does not require editing CMake.
set(LIBWEBRTC_VERSION_INI "${LIBWEBRTC_THIRD_PARTY_DIR}/libwebrtc_version.ini")
if(NOT EXISTS "${LIBWEBRTC_VERSION_INI}")
  message(FATAL_ERROR "libwebrtc: missing version manifest at ${LIBWEBRTC_VERSION_INI}")
endif()

set(LIBWEBRTC_BINARY_VERSION "")
set(LIBWEBRTC_DOWNLOAD_URL_BASE "")
file(STRINGS "${LIBWEBRTC_VERSION_INI}" _libwebrtc_ini_lines)
foreach(_line IN LISTS _libwebrtc_ini_lines)
  string(STRIP "${_line}" _line)
  # Skip blanks, comments (# or ;) and section headers ([name]).
  if(_line STREQUAL "" OR _line MATCHES "^[#;]" OR _line MATCHES "^\\[.*\\]$")
    continue()
  endif()
  if(NOT _line MATCHES "^([^=]+)=(.*)$")
    continue()
  endif()
  string(STRIP "${CMAKE_MATCH_1}" _key)
  string(STRIP "${CMAKE_MATCH_2}" _value)
  if(_key STREQUAL "binary_version")
    set(LIBWEBRTC_BINARY_VERSION "${_value}")
  elseif(_key STREQUAL "download_url")
    set(LIBWEBRTC_DOWNLOAD_URL_BASE "${_value}")
  elseif(_key MATCHES "^sha256_(.+)$")
    set("LIBWEBRTC_SHA256_${CMAKE_MATCH_1}" "${_value}")
  endif()
endforeach()

if(LIBWEBRTC_BINARY_VERSION STREQUAL "")
  message(FATAL_ERROR "libwebrtc: ${LIBWEBRTC_VERSION_INI} must define binary_version")
endif()

# Strip any trailing slashes so we control the separator when composing the URL.
string(REGEX REPLACE "/+$" "" LIBWEBRTC_DOWNLOAD_URL_BASE "${LIBWEBRTC_DOWNLOAD_URL_BASE}")

# Resolve OS field for the release asset name (win / linux)
if(CMAKE_SYSTEM_NAME STREQUAL "Windows")
  set(LIBWEBRTC_OS "win")
elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  set(LIBWEBRTC_OS "linux")
else()
  message(FATAL_ERROR "libwebrtc: unsupported target OS '${CMAKE_SYSTEM_NAME}'")
endif()

# Resolve arch field for the release asset name (x64 / arm64)
if(CMAKE_SYSTEM_NAME STREQUAL "Windows")
  # Visual Studio generator exposes the selected arch via CMAKE_GENERATOR_PLATFORM
  string(TOLOWER "${CMAKE_GENERATOR_PLATFORM}" _libwebrtc_plat)
  if(_libwebrtc_plat STREQUAL "arm64")
    set(LIBWEBRTC_ARCH "arm64")
  elseif(_libwebrtc_plat STREQUAL "x64" OR _libwebrtc_plat STREQUAL "")
    # Empty platform usually means Ninja/default x64 host build
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
      set(LIBWEBRTC_ARCH "x64")
    else()
      message(FATAL_ERROR "libwebrtc: 32-bit Windows is not supported")
    endif()
  else()
    message(FATAL_ERROR "libwebrtc: unsupported Windows arch '${CMAKE_GENERATOR_PLATFORM}'")
  endif()
else()
  # Flutter Linux / eLinux pass the target as e.g. linux-x64 or linux-arm64
  if(DEFINED FLUTTER_TARGET_PLATFORM AND FLUTTER_TARGET_PLATFORM MATCHES "arm64")
    set(LIBWEBRTC_ARCH "arm64")
  elseif(DEFINED FLUTTER_TARGET_PLATFORM AND FLUTTER_TARGET_PLATFORM MATCHES "x64")
    set(LIBWEBRTC_ARCH "x64")
  elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|arm64")
    set(LIBWEBRTC_ARCH "arm64")
  else()
    set(LIBWEBRTC_ARCH "x64")
  endif()
endif()

set(LIBWEBRTC_ASSET "libwebrtc-${LIBWEBRTC_OS}-${LIBWEBRTC_ARCH}-release")
# Named after the version, so a new version is downloaded rather than an old archive reused.
set(ZIPFILE "${LIBWEBRTC_THIRD_PARTY_DIR}/downloads/${LIBWEBRTC_BINARY_VERSION}-${LIBWEBRTC_ASSET}.zip")
set(LIBWEBRTC_SHA256 "${LIBWEBRTC_SHA256_${LIBWEBRTC_OS}-${LIBWEBRTC_ARCH}}")
if(LIBWEBRTC_SHA256 STREQUAL "")
  message(FATAL_ERROR "libwebrtc: ${LIBWEBRTC_VERSION_INI} has no sha256_${LIBWEBRTC_OS}-${LIBWEBRTC_ARCH}: "
    "build that archive with tools/webrtc (see its README) and record its SHA-256 there")
endif()
# Where the archive comes from, first found: FOGATA_LIBWEBRTC_ARCHIVE, naming one archive;
# this computer's store of the archives tools/webrtc built (FOGATA_LIBWEBRTC_DIR, by default
# fogata-libwebrtc in the home folder), as <store>/<version>/<asset>.zip; the copy downloaded
# before; and last the download from download_url, when the ini names one. Every one must
# have the SHA-256 above.
set(_libwebrtc_local "$ENV{FOGATA_LIBWEBRTC_ARCHIVE}")
if(_libwebrtc_local STREQUAL "")
  set(_libwebrtc_store "$ENV{FOGATA_LIBWEBRTC_DIR}")
  if(_libwebrtc_store STREQUAL "" AND CMAKE_HOST_WIN32)
    set(_libwebrtc_store "$ENV{USERPROFILE}/fogata-libwebrtc")
  elseif(_libwebrtc_store STREQUAL "")
    set(_libwebrtc_store "$ENV{HOME}/fogata-libwebrtc")
  endif()
  file(TO_CMAKE_PATH "${_libwebrtc_store}" _libwebrtc_store)
  set(_libwebrtc_stored "${_libwebrtc_store}/${LIBWEBRTC_BINARY_VERSION}/${LIBWEBRTC_ASSET}.zip")
  if(EXISTS "${_libwebrtc_stored}")
    set(_libwebrtc_local "${_libwebrtc_stored}")
  endif()
elseif(NOT EXISTS "${_libwebrtc_local}")
  message(FATAL_ERROR "libwebrtc: FOGATA_LIBWEBRTC_ARCHIVE names ${_libwebrtc_local}, which does not exist")
endif()
if(NOT _libwebrtc_local STREQUAL "")
  file(SHA256 "${_libwebrtc_local}" _libwebrtc_have)
  if(NOT _libwebrtc_have STREQUAL LIBWEBRTC_SHA256)
    message(FATAL_ERROR "libwebrtc: ${_libwebrtc_local} has the SHA-256 ${_libwebrtc_have}, "
      "not the ${LIBWEBRTC_SHA256} that ${LIBWEBRTC_VERSION_INI} names")
  endif()
  set(ZIPFILE "${_libwebrtc_local}")
endif()
# What was extracted last, so a different version or archive is extracted again.
set(LIBWEBRTC_STAMP "${LIBWEBRTC_THIRD_PARTY_DIR}/libwebrtc/.fogata-archive")

# Final URL = <base>/<binary_version>/<asset>.zip
set(DOWNLOAD_URL "${LIBWEBRTC_DOWNLOAD_URL_BASE}/${LIBWEBRTC_BINARY_VERSION}/${LIBWEBRTC_ASSET}.zip")

function(_libwebrtc_extract_and_normalize zip_path dest_dir)
  # Extract into an isolated staging dir so we can normalize the top-level
  # layout regardless of what folder name the archive uses (e.g.
  # libwebrtc-linux-x64-release/ vs. libwebrtc/ vs. files at the root).
  set(_staging "${dest_dir}/.libwebrtc_extract")
  file(REMOVE_RECURSE "${_staging}")
  file(MAKE_DIRECTORY "${_staging}")
  file(ARCHIVE_EXTRACT INPUT "${zip_path}" DESTINATION "${_staging}")

  if(EXISTS "${dest_dir}/libwebrtc")
    file(REMOVE_RECURSE "${dest_dir}/libwebrtc")
  endif()

  file(GLOB _entries "${_staging}/*")
  list(LENGTH _entries _entry_count)
  set(_promoted FALSE)
  if(_entry_count EQUAL 1)
    list(GET _entries 0 _only)
    if(IS_DIRECTORY "${_only}")
      file(RENAME "${_only}" "${dest_dir}/libwebrtc")
      set(_promoted TRUE)
    endif()
  endif()
  if(NOT _promoted)
    # Archive root holds the files directly — wrap them in libwebrtc/.
    file(MAKE_DIRECTORY "${dest_dir}/libwebrtc")
    foreach(_entry IN LISTS _entries)
      get_filename_component(_name "${_entry}" NAME)
      file(RENAME "${_entry}" "${dest_dir}/libwebrtc/${_name}")
    endforeach()
  endif()

  file(REMOVE_RECURSE "${_staging}")

  # Fogata: every file of an archive carries one fixed date (the same sources give the
  # same archive), so another archive unpacked over the last one looked unchanged to the
  # build: objects compiled against the old headers were kept and linked with the new
  # library. What is unpacked gets the time of its unpacking.
  file(GLOB_RECURSE _unpacked LIST_DIRECTORIES false "${dest_dir}/libwebrtc/*")
  if(_unpacked)
    file(TOUCH_NOCREATE ${_unpacked})
  endif()
endfunction()

if(EXISTS "${ZIPFILE}")
  file(SHA256 "${ZIPFILE}" _libwebrtc_have)
  if(NOT _libwebrtc_have STREQUAL LIBWEBRTC_SHA256)
    message(NOTICE "libwebrtc: ${ZIPFILE} does not match its SHA-256; downloading it again")
    file(REMOVE "${ZIPFILE}")
  endif()
endif()

if(NOT EXISTS "${ZIPFILE}" AND LIBWEBRTC_DOWNLOAD_URL_BASE STREQUAL "")
  message(FATAL_ERROR "libwebrtc: ${LIBWEBRTC_BINARY_VERSION} is not published, and this computer "
    "does not have ${_libwebrtc_stored}: build it with tools/webrtc (see its README), or copy "
    "it there from a computer that did")
endif()

if(NOT EXISTS "${ZIPFILE}")
  message(NOTICE "download: ${DOWNLOAD_URL}")
  file(MAKE_DIRECTORY "${LIBWEBRTC_THIRD_PARTY_DIR}/downloads")
  file(DOWNLOAD "${DOWNLOAD_URL}"
      ${ZIPFILE}
      EXPECTED_HASH SHA256=${LIBWEBRTC_SHA256}
      STATUS download_status
      LOG download_log)

  if(NOT download_status EQUAL 0)
    file(REMOVE "${ZIPFILE}")
    message(FATAL_ERROR "Failed to download dependency: ${download_log}\n"
      "An archive built with tools/webrtc can be put in ${_libwebrtc_store}/${LIBWEBRTC_BINARY_VERSION}/.")
  endif()
endif()

set(_libwebrtc_stamp_want "${LIBWEBRTC_BINARY_VERSION} ${LIBWEBRTC_SHA256}")
set(_libwebrtc_stamp_have "")
if(EXISTS "${LIBWEBRTC_STAMP}")
  file(READ "${LIBWEBRTC_STAMP}" _libwebrtc_stamp_have)
endif()
if(NOT _libwebrtc_stamp_have STREQUAL _libwebrtc_stamp_want)
  _libwebrtc_extract_and_normalize("${ZIPFILE}" "${LIBWEBRTC_THIRD_PARTY_DIR}")
  file(WRITE "${LIBWEBRTC_STAMP}" "${_libwebrtc_stamp_want}")
else()
  message(TRACE "libwebrtc already extracted.")
endif()
