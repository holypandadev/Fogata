# Tests of the Podfile's check of the WebRTC-SDK pod as installed (fogata_tree_digest and
# fogata_check_webrtc_sdk), with plain Ruby: the two are read from the Podfile itself and
# run on folders made here, without CocoaPods. Written for the Ruby macOS comes with (2.6)
# as much as for a newer one.
#
#     ruby app/macos/test/webrtc_sdk_test.rb
require 'minitest/autorun'
require 'tmpdir'
require 'fileutils'
require 'pathname'

MACOS = File.expand_path('..', __dir__)
podfile = File.read(File.join(MACOS, 'Podfile'))
checks = podfile[/^require 'digest'\n.*?(?=^post_install do)/m]
raise 'the Podfile has no check of WebRTC-SDK' if checks.nil?

# The Podfile's own functions, defined as the Podfile defines them, with the list of
# digests looked for next to a (blank) Podfile in the test's folder.
module PodfileChecks
  def self.load(source, podfile_path)
    eval(source, TOPLEVEL_BINDING, podfile_path)
  end
end

class WebrtcSdkCheck < Minitest::Test
  Spec = Struct.new(:version)
  Target = Struct.new(:pod_name, :root_spec)
  Sandbox = Struct.new(:dir) do
    def pod_dir(_name)
      Pathname(dir)
    end
  end
  Installer = Struct.new(:pod_targets, :sandbox)

  def setup
    @tmp = Dir.mktmpdir('fogata-podfile')
    @pod = File.join(@tmp, 'Pods/WebRTC-SDK')
    FileUtils.mkdir_p(File.join(@pod, 'WebRTC.xcframework/macos'))
    File.write(File.join(@pod, 'WebRTC.xcframework/Info.plist'), 'plist')
    File.write(File.join(@pod, 'WebRTC.xcframework/macos/WebRTC'), 'code')
    File.symlink('macos/WebRTC', File.join(@pod, 'WebRTC.xcframework/Current'))
    File.write(File.join(@tmp, 'Podfile'), '')
    PodfileChecks.load(CHECKS, File.join(@tmp, 'Podfile'))
  end

  def teardown
    FileUtils.rm_rf(@tmp)
  end

  def installer(version = '1.2.3')
    target = Target.new('WebRTC-SDK', Spec.new(version))
    Installer.new([target], Sandbox.new(@pod))
  end

  def record(line)
    File.write(File.join(@tmp, 'WebRTC-SDK.sha256'), "#{line}\n")
  end

  def test_the_pod_recorded_installs
    record("1.2.3 #{fogata_tree_digest(@pod)}")
    fogata_check_webrtc_sdk(installer)
  end

  def test_a_changed_file_stops_the_install
    record("1.2.3 #{fogata_tree_digest(@pod)}")
    File.write(File.join(@pod, 'WebRTC.xcframework/macos/WebRTC'), 'other code')
    error = assert_raises(RuntimeError) { fogata_check_webrtc_sdk(installer) }
    assert_match(/is not the one recorded/, error.message)
  end

  def test_a_link_pointed_elsewhere_or_a_new_file_stops_the_install
    record("1.2.3 #{fogata_tree_digest(@pod)}")
    link = File.join(@pod, 'WebRTC.xcframework/Current')
    File.delete(link)
    File.symlink('Info.plist', link)
    assert_raises(RuntimeError) { fogata_check_webrtc_sdk(installer) }
    File.delete(link)
    File.symlink('macos/WebRTC', link)
    fogata_check_webrtc_sdk(installer)
    File.write(File.join(@pod, '.hidden'), 'planted')
    assert_raises(RuntimeError) { fogata_check_webrtc_sdk(installer) }
  end

  def test_a_version_not_recorded_stops_the_install_and_says_the_line
    record("1.2.3 #{fogata_tree_digest(@pod)}")
    error = assert_raises(RuntimeError) { fogata_check_webrtc_sdk(installer('1.2.4')) }
    assert_match(/1\.2\.4 #{fogata_tree_digest(@pod)}/, error.message)
  end

  def test_a_project_without_the_pod_is_not_checked
    fogata_check_webrtc_sdk(Installer.new([], Sandbox.new(@pod)))
  end

  # The version the podspec builds has its digest recorded beside the Podfile.
  def test_the_podspecs_version_is_recorded
    podspec = File.read(File.join(MACOS, 'WebRTC-SDK.podspec'))
    version = podspec[/s\.version = '([^']+)'/, 1]
    recorded = File.readlines(File.join(MACOS, 'WebRTC-SDK.sha256')).map(&:split)
    assert(recorded.any? { |v, digest| v == version && digest =~ /\A\h{64}\z/ }, version)
  end
end

CHECKS = checks
