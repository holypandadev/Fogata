# WebRTC for macOS built by Fogata (tools/webrtc/build-macos.sh, see tools/webrtc/MACOS.md):
# webrtc-sdk's Objective-C SDK on WebRTC M156 (branch-head 8078), VP8 and Opus as the only
# codecs, VP8 pictures larger than 8K refused before they are decoded. It takes the
# place of webrtc-sdk's WebRTC-SDK pod of the same name once the Podfile names this file.
#
# It is not published: the archive comes from this computer's store of what tools/webrtc
# built, <home>/fogata-libwebrtc/<version>/WebRTC.xcframework.zip (FOGATA_LIBWEBRTC_DIR names
# another folder), or from FOGATA_WEBRTC_XCFRAMEWORK, naming one archive. It must have the
# SHA-256 below.
Pod::Spec.new do |s|
  s.name = 'WebRTC-SDK'
  s.version = '156.8078.5'
  s.summary = 'WebRTC M156 for macOS, built by Fogata.'
  s.homepage = 'https://webrtc.googlesource.com/src'
  s.license = { :type => 'BSD-3-Clause', :file => 'WebRTC.xcframework/LICENSE' }
  s.author = 'The WebRTC project authors'

  sha256 = '72235368ce8addf9ef36a93dedaec16fd64bc5bec382686175b1d9218db96961'
  local = ENV['FOGATA_WEBRTC_XCFRAMEWORK'].to_s
  if local.empty?
    store = ENV['FOGATA_LIBWEBRTC_DIR'].to_s
    store = File.join(Dir.home, 'fogata-libwebrtc') if store.empty?
    local = File.join(store, 'fogata-libwebrtc.m156.8078.5', 'WebRTC.xcframework.zip')
  end
  unless File.exist?(File.expand_path(local))
    raise "WebRTC-SDK: #{local} does not exist. Build it with tools/webrtc/build-macos.sh " \
          "(tools/webrtc/MACOS.md), or copy it there from a computer that did."
  end
  s.source = { :http => "file://#{File.expand_path(local)}", :sha256 => sha256 }

  s.osx.deployment_target = '12.0'
  s.vendored_frameworks = 'WebRTC.xcframework'
  s.preserve_paths = 'WebRTC.xcframework/NOTICE.md'
end
