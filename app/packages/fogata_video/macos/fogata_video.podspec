Pod::Spec.new do |s|
  s.name             = 'fogata_video'
  s.version          = '0.1.0'
  s.summary          = 'Videos with AVFoundation.'
  s.description      = 'Reads a video first picture and plays videos into Flutter textures.'
  s.homepage         = 'https://github.com/holypandadev/fogata'
  s.license          = { :type => 'AGPL-3.0-only' }
  s.author           = 'Holy Panda'
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'
  s.frameworks       = 'AVFoundation', 'CoreMedia', 'CoreVideo', 'UniformTypeIdentifiers'
  s.platform         = :osx, '12.0'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version    = '5.0'
end
