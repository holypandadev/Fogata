// Cameras on Linux, as libwebrtc's video device reaches them (tools/webrtc/patches/wrapper/
// 0009 and patches/webrtc/0005) and as the plugin chooses the way (linux/camera_access.h).
//
// Always: which way a process takes (a Flatpak the Camera portal, anything else V4L2,
// FOGATA_CAMERA over both), and that cameras opened directly ask nothing.
//
// With FOGATA_TEST_CAMERA_PORTAL set, the cameras through xdg-desktop-portal's Camera
// interface and PipeWire, in a desktop session where the portal answers (for example
// tools/smoke-wayland/session.sh with a camera of tools/smoke-wayland/camera_source.c):
//   granted      access is granted; the camera FOGATA_TEST_CAMERA_NAME is listed, with the
//                id FOGATA_TEST_CAMERA_ID; it gives at least 20 pictures of 1280x720 in
//                3 s; with FOGATA_TEST_CAMERA_PLUG (a folder) a second camera
//                ("Second test camera") plugged in meanwhile is listed and goes when it
//                is unplugged, the first still giving pictures: the test writes
//                <folder>/plug, the session starts that camera, then <folder>/unplug, the
//                session ends it (from outside a sandbox, as a camera is plugged in)
//   denied       access is refused: no camera is listed, none opens
//   unavailable  no portal answers: no camera is listed (V4L2 does not stand in)
// The GLib main loop runs while it waits, as GTK's does in the application: the portal
// answers there. FOGATA_WEBRTC_LOG=verbose (or info) shows WebRTC's own log.

#include <glib.h>

#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <functional>
#include <string>
#include <thread>

#include "camera_access.h"
#include "libwebrtc.h"
#include "rtc_logging.h"
#include "rtc_mediaconstraints.h"
#include "rtc_peerconnection_factory.h"
#include "rtc_video_device.h"
#include "rtc_video_frame.h"
#include "rtc_video_renderer.h"
#include "rtc_video_source.h"
#include "rtc_video_track.h"

using namespace flutter_webrtc_plugin;
using namespace libwebrtc;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  std::printf("%s: %s\n", ok ? "ok" : "FAIL", what.c_str());
  if (!ok) failures++;
}

std::string Env(const char* name) {
  const char* value = getenv(name);
  return value ? value : "";
}

// Runs GLib's main loop until [done] or [seconds] pass; whether [done].
bool Wait(const std::function<bool()>& done, double seconds) {
  const auto end = std::chrono::steady_clock::now() +
                   std::chrono::milliseconds(static_cast<int>(seconds * 1000));
  while (!done()) {
    if (std::chrono::steady_clock::now() > end) return false;
    g_main_context_iteration(nullptr, FALSE);
    std::this_thread::sleep_for(std::chrono::milliseconds(10));
  }
  return true;
}

struct Camera {
  std::string name;
  std::string id;
};

std::vector<Camera> Cameras(scoped_refptr<RTCVideoDevice> device) {
  std::vector<Camera> cameras;
  const uint32_t n = device->NumberOfDevices();
  for (uint32_t i = 0; i < n; i++) {
    char name[256] = {0};
    char id[256] = {0};
    if (device->GetDeviceName(i, name, sizeof(name), id, sizeof(id)) != 0) continue;
    cameras.push_back({name, id});
  }
  return cameras;
}

int Find(const std::vector<Camera>& cameras, const std::string& name) {
  for (size_t i = 0; i < cameras.size(); i++) {
    if (cameras[i].name == name) return static_cast<int>(i);
  }
  return -1;
}

class Counter : public RTCVideoRenderer<scoped_refptr<RTCVideoFrame>> {
 public:
  void OnFrame(scoped_refptr<RTCVideoFrame> frame) override {
    if (frame->width() == 1280 && frame->height() == 720) right++;
    frames++;
  }
  std::atomic<int> frames{0};
  std::atomic<int> right{0};
};

void Ways() {
  struct {
    bool flatpak;
    const char* setting;
    bool portal;
  } cases[] = {
      {true, "", true},           {false, "", false},
      {false, "portal", true},    {true, "direct", false},
      {true, "nonsense", true},   {false, "nonsense", false},
  };
  for (const auto& c : cases) {
    const CameraWay way = ChooseCameraWay(c.flatpak, c.setting);
    check(way.portal == c.portal && way.direct_fallback == c.portal,
          std::string(c.flatpak ? "a Flatpak" : "outside a Flatpak") +
              " with FOGATA_CAMERA='" + c.setting + "' takes " +
              (c.portal ? "the portal, V4L2 standing in" : "V4L2"));
  }
}

void Direct(scoped_refptr<RTCVideoDevice> device) {
  check(device->GetCameraAccess() == RTCVideoDevice::kCameraAccessDirect,
        "cameras are opened directly unless told otherwise");
  int told = -1;
  device->RequestCameraAccess(
      [&told](RTCVideoDevice::CameraAccess access) { told = access; });
  check(told == RTCVideoDevice::kCameraAccessDirect,
        "asking for cameras opened directly answers at once");
}

void Portal(scoped_refptr<RTCPeerConnectionFactory> factory,
            scoped_refptr<RTCVideoDevice> device, const std::string& expected) {
  device->SetCameraPortal(true, false);
  check(device->GetCameraAccess() == RTCVideoDevice::kCameraAccessNotAsked,
        "through the portal, nothing is asked before it is asked for");
  check(device->NumberOfDevices() == 0, "nothing is listed before it is asked for");

  std::atomic<int> told{-1};
  std::atomic<int> times{0};
  device->RequestCameraAccess([&](RTCVideoDevice::CameraAccess access) {
    told = access;
    times++;
  });
  // Asked again while the first is under way: both are told.
  std::atomic<int> second{-1};
  device->RequestCameraAccess(
      [&](RTCVideoDevice::CameraAccess access) { second = access; });
  check(Wait([&] { return told != -1 && second != -1; }, 60),
        "the portal answers within 60 s");
  const char* outcome =
      told == RTCVideoDevice::kCameraAccessGranted  ? "granted"
      : told == RTCVideoDevice::kCameraAccessDenied ? "denied"
      : told == RTCVideoDevice::kCameraAccessUnavailable ? "unavailable"
                                                         : "something else";
  check(expected == outcome, "the portal's answer is " + expected + " (" + outcome + ")");
  check(second == told && times == 1, "a second request while asking is told the same");
  check(device->GetCameraAccess() == told, "the device keeps the answer");

  const auto cameras = Cameras(device);
  if (expected != "granted") {
    check(cameras.empty(), "no camera is listed");
    auto capturer = device->Create("", 0, 1280, 720, 30);
    check(capturer.get() == nullptr, "no camera opens");
    return;
  }

  const std::string name = Env("FOGATA_TEST_CAMERA_NAME");
  const int index = Find(cameras, name);
  check(index >= 0, "the camera '" + name + "' is listed");
  if (index < 0) return;
  const std::string id = Env("FOGATA_TEST_CAMERA_ID");
  if (!id.empty()) {
    check(cameras[index].id == id,
          "its id is the one V4L2 gives it ('" + cameras[index].id + "')");
  }

  auto capturer = device->Create(name.c_str(), index, 1280, 720, 30);
  check(capturer.get() != nullptr, "it opens");
  if (!capturer.get()) return;
  check(capturer->StartCapture(), "it starts");
  auto source = factory->CreateVideoSource(capturer, "camera", RTCMediaConstraints::Create());
  auto track = factory->CreateVideoTrack(source, "camera");
  Counter counter;
  track->AddRenderer(&counter);
  Wait([] { return false; }, 3);
  std::printf("pictures in 3 s: %d, of 1280x720: %d\n", counter.frames.load(),
              counter.right.load());
  check(counter.right >= 20, "it gives pictures of the size asked");

  const std::string plug = Env("FOGATA_TEST_CAMERA_PLUG");
  if (!plug.empty()) {
    // Another camera plugged in, then out.
    const char* second_name = "Second test camera";
    std::ofstream(plug + "/plug") << "plug\n";
    check(Wait([&] { return Find(Cameras(device), second_name) >= 0; }, 15),
          "a camera plugged in meanwhile is listed");
    std::ofstream(plug + "/unplug") << "unplug\n";
    check(Wait([&] { return Find(Cameras(device), second_name) < 0; }, 15),
          "a camera unplugged goes from the list");
    check(Find(Cameras(device), name) >= 0, "the first camera is still listed");
    const int before = counter.frames;
    Wait([] { return false; }, 1);
    check(counter.frames > before, "the first camera still gives pictures");
  }

  track->RemoveRenderer(&counter);
  capturer->StopCapture();
}

}  // namespace

void LogToStderr(const libwebrtc::string& message) {
  std::fprintf(stderr, "webrtc: %s", message.std_string().c_str());
}

int main() {
  // Each line as it comes, whatever ends the test.
  setvbuf(stdout, nullptr, _IOLBF, 0);
  Ways();

  const std::string log = Env("FOGATA_WEBRTC_LOG");
  if (!log.empty()) {
    LibWebRTCLogging::setLogSink(log == "verbose" ? Verbose : Info, &LogToStderr);
  }

  LibWebRTC::Initialize();
  auto factory = LibWebRTC::CreateRTCPeerConnectionFactory();
  if (factory.get() == nullptr || !factory->Initialize()) {
    std::printf("FAIL: no peer connection factory\n");
    return 1;
  }
  auto device = factory->GetVideoDevice();
  Direct(device);
  const std::string expected = Env("FOGATA_TEST_CAMERA_PORTAL");
  if (!expected.empty()) Portal(factory, device, expected);

  device = nullptr;
  factory->Terminate();
  factory = nullptr;
  LibWebRTC::Terminate();
  std::printf(failures ? "camera_portal_test: %d failed\n" : "camera_portal_test: passed\n",
              failures);
  return failures ? 1 : 0;
}
