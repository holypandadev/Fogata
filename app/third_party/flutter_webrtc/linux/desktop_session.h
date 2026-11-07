#ifndef FLUTTER_WEBRTC_DESKTOP_SESSION_H_
#define FLUTTER_WEBRTC_DESKTOP_SESSION_H_

#include <map>
#include <string>

namespace flutter_webrtc_plugin {

// A Wayland session, where the screen is shared through the system's own dialog
// (PortalScreenCapturer) and libwebrtc is asked for no screen or window list: GTK draws on
// a Wayland display. The environment can say otherwise (sway, or variables imported by
// systemd, name both displays and no session type), and a list of X11's screens would then
// only show black. Without a GTK display (tests), the environment decides
// (WaylandEnvironment). call.dart asks this through the "fogataWaylandSession" method.
bool WaylandSession();

// A Wayland session as the environment tells it: a Wayland display is named (an empty name
// counts, as for WebRTC, whose own test is XDG_SESSION_TYPE "wayland" and WAYLAND_DISPLAY
// set), and either the session says it is Wayland or no X display is named. A program
// started from a service may be given only WAYLAND_DISPLAY: GTK draws on that display, and
// with no X display there is nothing a list could show. call.dart's `waylandSession` is the
// same rule.
bool WaylandEnvironment(const std::map<std::string, std::string>& environment);

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_DESKTOP_SESSION_H_
