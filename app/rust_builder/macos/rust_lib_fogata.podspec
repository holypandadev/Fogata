#
# The Fogata library (crates/fogata-ffi) for the macOS app, built by build_rust.sh.
#
Pod::Spec.new do |s|
  s.name             = 'rust_lib_fogata'
  s.version          = '0.1.0'
  s.summary          = 'The Fogata library, built from the Rust workspace.'
  s.description      = <<-DESC
The Rust core of Fogata as the Flutter interface sees it (crates/fogata-ffi), as a static
library linked into the application.
                       DESC
  s.homepage         = 'https://holypanda.dev'
  s.license          = { :type => 'AGPL-3.0-only' }
  s.author           = { 'Holy Panda' => '239265931+holypandadev@users.noreply.github.com' }

  # This will ensure the source files in Classes/ are included in the native
  # builds of apps using this FFI plugin. Podspec does not support relative
  # paths, so Classes contains a forwarder C file that relatively imports
  # `../src/*` so that the C sources can be shared among all target platforms.
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '12.0'
  # System frameworks and libraries the Rust library uses: network settings and changes
  # (libp2p, hickory-resolver), sound output (cpal) and the Objective-C runtime (objc2).
  s.frameworks = 'SystemConfiguration', 'CoreAudio', 'AudioToolbox', 'CoreFoundation', 'Foundation'
  s.libraries = 'objc'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version = '5.0'

  s.script_phase = {
    :name => 'Build Rust library',
    :script => 'sh "$PODS_TARGET_SRCROOT/build_rust.sh"',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/rust_phony'],
    # Let XCode know that the static library referenced in -force_load below is
    # created by this build step.
    :output_files => ["${BUILT_PRODUCTS_DIR}/libfogata_ffi.a"],
  }
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    # Flutter.framework does not contain a i386 slice.
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'OTHER_LDFLAGS' => '-force_load ${BUILT_PRODUCTS_DIR}/libfogata_ffi.a',
  }
end