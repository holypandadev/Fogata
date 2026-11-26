#ifndef FOGATA_PORTAL_CAMERA_H_
#define FOGATA_PORTAL_CAMERA_H_

// Fogata: the error for a camera reached through the Camera portal (Linux, in a Flatpak)
// that did not start. Its capture starts before PipeWire links the camera to it, so the
// plugin waits for the first picture (libwebrtc's RTCVideoCapturer::WaitForStart,
// tools/webrtc/patches/wrapper/0028) and then tells what PipeWire's stream said. A V4L2
// camera another application streams refuses PipeWire's VIDIOC_S_FMT with EBUSY, and
// WirePlumber sends the stream the failed link's words: "error set output format: -16
// (Device or resource busy)", the system's text in the sound server's language, so the
// number is what is looked for. The sandbox has no /dev/video* to look at the camera
// itself, as the plugin does for cameras reached directly. Header only, with no
// libwebrtc, so that its test (test/portal_camera_test.cc) builds anywhere.

#include <string>

namespace flutter_webrtc_plugin {

// Whether what PipeWire's stream was told says that another process holds the camera:
// a failed link's EBUSY (-16, as PipeWire writes an error) or the system's English words
// for it.
inline bool PortalCameraBusy(const std::string& why) {
  return why.find(": -16 (") != std::string::npos ||
         why.find("Device or resource busy") != std::string::npos;
}

// The error for a camera through the portal whose PipeWire stream was told it failed
// ([why], its words): busy when they say so, else that the camera did not start. The
// words after the colon go to the day's log. A stream with no picture yet when the wait
// ends is kept, as before the wait, and needs no error.
inline std::string PortalCameraError(const std::string& why) {
  if (PortalCameraBusy(why)) {
    return "NotReadableError: another application is using the camera "
           "(fogataCameraBusy): " +
           why;
  }
  return "NotReadableError: the camera did not start: " + why;
}

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_PORTAL_CAMERA_H_
