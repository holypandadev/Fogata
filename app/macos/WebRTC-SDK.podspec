# WebRTC for macOS built by Fogata (tools/webrtc/build-macos.sh, see tools/webrtc/MACOS.md):
# webrtc-sdk's Objective-C SDK on WebRTC M158 (branch-head 8127), VP8 and Opus as the only
# codecs, VP8 pictures larger than 8K refused before they are decoded. It takes the
# place of webrtc-sdk's WebRTC-SDK pod of the same name once the Podfile names this file.
#
# The archive comes from this computer's store of what tools/webrtc built,
# <home>/fogata-libwebrtc/<version>/WebRTC.xcframework.zip (FOGATA_LIBWEBRTC_DIR names
# another folder), or from FOGATA_WEBRTC_XCFRAMEWORK, naming one archive; without either it is
# downloaded from the releases of holypandadev/fogata-libwebrtc, where the Linux and Windows
# archives are too. Wherever it comes from, it must have the SHA-256 below.
Pod::Spec.new do |s|
  s.name = 'WebRTC-SDK'
  s.version = '158.8127.3'
  s.summary = 'WebRTC M158 for macOS, built by Fogata.'
  s.homepage = 'https://webrtc.googlesource.com/src'
  s.license = { :type => 'BSD-3-Clause', :file => 'WebRTC.xcframework/LICENSE' }
  s.author = 'The WebRTC project authors'

  sha256 = 'f2c4a03bca28b309ce672f2afa322ffbfce5189ff7abbd62455b408771342655'
  local = ENV['FOGATA_WEBRTC_XCFRAMEWORK'].to_s
  if local.empty?
    store = ENV['FOGATA_LIBWEBRTC_DIR'].to_s
    store = File.join(Dir.home, 'fogata-libwebrtc') if store.empty?
    local = File.join(store, 'fogata-libwebrtc.m158.8127.3', 'WebRTC.xcframework.zip')
  end
  published = 'https://github.com/holypandadev/fogata-libwebrtc/releases/download/' \
              'fogata-libwebrtc.m158.8127.3/WebRTC.xcframework.zip'
  url = File.exist?(File.expand_path(local)) ? "file://#{File.expand_path(local)}" : published
  s.source = { :http => url, :sha256 => sha256 }

  s.osx.deployment_target = '12.0'
  s.vendored_frameworks = 'WebRTC.xcframework'
  s.preserve_paths = 'WebRTC.xcframework/NOTICE.md'
end
