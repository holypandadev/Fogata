// A camera reached through the Camera portal whose stream failed (fogata_portal_camera.h):
// which of PipeWire's words say that another application holds it, in any language of the
// sound server, and the error the plugin answers with. Plain C++; the wait itself
// (libwebrtc's RTCVideoCapturer::WaitForStart) needs PipeWire and a camera
// (camportal/PLAN.md in the agents' notes).

#include <cstdio>
#include <string>

#include "fogata_portal_camera.h"

using namespace flutter_webrtc_plugin;

namespace {

int failures = 0;

void check(bool ok, const char* what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what);
    failures++;
  }
}

bool Has(const std::string& text, const char* part) {
  return text.find(part) != std::string::npos;
}

}  // namespace

int main() {
  // What WirePlumber 0.5 sends the stream for a V4L2 camera whose VIDIOC_S_FMT answers
  // EBUSY (PipeWire's impl-link.c: "error set %s format: %d (%s)").
  const std::string busy = "error set output format: -16 (Device or resource busy)";
  check(PortalCameraBusy(busy), "EBUSY in English");
  check(PortalCameraBusy("error set output format: -16 (Dispositivo o recurso ocupado)"),
        "EBUSY in the sound server's Spanish");
  check(PortalCameraBusy("Device or resource busy"), "the English words alone");
  // Other failures are not taken for a busy camera.
  check(!PortalCameraBusy("Format negotiation failed"), "a format no side takes");
  check(!PortalCameraBusy("Buffer allocation failed"), "buffers");
  check(!PortalCameraBusy("error set output format: -22 (Invalid argument)"), "EINVAL");
  check(!PortalCameraBusy("error set output format: -160 (x)"), "another number");
  check(!PortalCameraBusy("PipeWire stream paused"), "a state");
  check(!PortalCameraBusy(""), "nothing said");

  const std::string held = PortalCameraError(busy);
  check(Has(held, "(fogataCameraBusy): "), "busy: the app's mark");
  check(held.rfind("NotReadableError:", 0) == 0, "busy: named as a browser names it");
  check(Has(held, busy.c_str()), "busy: PipeWire's words, for the log");

  const std::string failed = PortalCameraError("Format negotiation failed");
  check(!Has(failed, "fogataCamera"), "failed: no mark of the app's own");
  check(failed == "NotReadableError: the camera did not start: Format negotiation failed",
        "failed: the words");

  if (failures == 0) std::printf("portal_camera_test: passed\n");
  return failures == 0 ? 0 : 1;
}
