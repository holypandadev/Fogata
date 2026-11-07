#include "camera_access.h"

#include <unistd.h>

#include <cstdlib>

namespace flutter_webrtc_plugin {

CameraWay ChooseCameraWay(bool flatpak, const std::string& setting) {
  CameraWay way;
  if (setting == "portal") {
    way.portal = true;
  } else if (setting == "direct") {
    way.portal = false;
  } else {
    way.portal = flatpak;
  }
  way.direct_fallback = way.portal;
  return way;
}

CameraWay CameraWayHere() {
  // Flatpak puts this file at the root of every sandbox it makes.
  const bool flatpak = access("/.flatpak-info", F_OK) == 0;
  const char* setting = getenv("FOGATA_CAMERA");
  return ChooseCameraWay(flatpak, setting ? setting : "");
}

const char* CameraAccessName(libwebrtc::RTCVideoDevice::CameraAccess access) {
  switch (access) {
    case libwebrtc::RTCVideoDevice::kCameraAccessDirect:
      return "direct";
    case libwebrtc::RTCVideoDevice::kCameraAccessNotAsked:
      return "notAsked";
    case libwebrtc::RTCVideoDevice::kCameraAccessAsking:
      return "asking";
    case libwebrtc::RTCVideoDevice::kCameraAccessGranted:
      return "granted";
    case libwebrtc::RTCVideoDevice::kCameraAccessDenied:
      return "denied";
    case libwebrtc::RTCVideoDevice::kCameraAccessUnavailable:
      return "unavailable";
  }
  return "unavailable";
}

}  // namespace flutter_webrtc_plugin
