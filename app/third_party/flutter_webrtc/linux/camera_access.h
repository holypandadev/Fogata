#ifndef FLUTTER_WEBRTC_CAMERA_ACCESS_H_
#define FLUTTER_WEBRTC_CAMERA_ACCESS_H_

#include <string>

#include "rtc_video_device.h"

namespace flutter_webrtc_plugin {

// How the cameras are reached on Linux. In a Flatpak, through the Camera portal
// and PipeWire, which ask the person once: the sandbox has no /dev/video*, and
// asks for no device to have them. Elsewhere libwebrtc opens /dev/video* itself
// (V4L2), as before: it needs no portal, no permission and no PipeWire camera
// support, and keeps the ids of the cameras chosen. FOGATA_CAMERA=portal takes
// the portal outside a Flatpak too, FOGATA_CAMERA=direct V4L2 inside one. Through
// the portal, V4L2 stands in while the portal is unavailable (a system without
// it, or without its Camera interface), never when it was refused.
struct CameraWay {
  bool portal = false;
  bool direct_fallback = false;
};

// [flatpak]: whether this process runs in a Flatpak; [setting]: FOGATA_CAMERA's
// value (empty when unset).
CameraWay ChooseCameraWay(bool flatpak, const std::string& setting);

// The way for this process.
CameraWay CameraWayHere();

// How a camera access is named to Dart ("fogataCameraAccess").
const char* CameraAccessName(libwebrtc::RTCVideoDevice::CameraAccess access);

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_CAMERA_ACCESS_H_
