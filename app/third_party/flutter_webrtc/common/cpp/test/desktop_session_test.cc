// Screen sharing in the environments the application meets. Started from a user service,
// it had only WAYLAND_DISPLAY (no session type, no X display): picking what to share asked
// libwebrtc for its lists, which started a capturer WebRTC had not made and ended the
// application. Now that environment is a Wayland session for the plugin (the system's own
// dialog shares the screen), and libwebrtc itself (tools/webrtc/patches/wrapper/0007)
// gives empty lists, and a capturer that does not start, where it can make no capturer.
// On an X display (FOGATA_TEST_X_DISPLAY, which run_tests.sh starts with Xvfb) the lists
// are made as before.

#include <gdk/gdk.h>

#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>

#include "desktop_session.h"
#include "libwebrtc.h"
#include "rtc_desktop_capturer.h"
#include "rtc_desktop_device.h"
#include "rtc_desktop_media_list.h"
#include "rtc_peerconnection_factory.h"

using namespace flutter_webrtc_plugin;
using namespace libwebrtc;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// Sets the three variables the decisions read; nullptr leaves one unset.
void Environment(const char* session_type, const char* wayland, const char* x) {
  auto set = [](const char* name, const char* value) {
    if (value == nullptr) {
      unsetenv(name);
    } else {
      setenv(name, value, 1);
    }
  };
  set("XDG_SESSION_TYPE", session_type);
  set("WAYLAND_DISPLAY", wayland);
  set("DISPLAY", x);
}

// Lists the screens and windows through a factory of its own (each caches its lists),
// checking how many screens it finds; the first screen, if any.
scoped_refptr<MediaSource> List(int screens, const std::string& where) {
  scoped_refptr<RTCPeerConnectionFactory> factory =
      LibWebRTC::CreateRTCPeerConnectionFactory();
  factory->Initialize();
  scoped_refptr<MediaSource> first;
  for (DesktopType type : {kScreen, kWindow}) {
    scoped_refptr<RTCDesktopMediaList> list =
        factory->GetDesktopDevice()->GetDesktopMediaList(type);
    // Thumbnails only where there is nothing to capture: that path must not use the
    // missing capturer either.
    list->UpdateSourceList(true, screens == 0);
    if (type == kScreen) {
      check(list->GetSourceCount() == screens,
            where + ": " + std::to_string(screens) + " screens listed, not " +
                std::to_string(list->GetSourceCount()));
      if (list->GetSourceCount() > 0) first = list->GetSource(0);
    } else if (screens == 0) {
      check(list->GetSourceCount() == 0, where + ": no windows listed");
    }
  }
  factory->Terminate();
  return first;
}

}  // namespace

int main() {
  const char* x_display = std::getenv("FOGATA_TEST_X_DISPLAY");

  // The environment's rule, where GTK's display is not known.
  using Env = std::map<std::string, std::string>;
  check(WaylandEnvironment(Env{{"WAYLAND_DISPLAY", "wayland-0"}}),
        "only WAYLAND_DISPLAY is a Wayland session");
  check(WaylandEnvironment(Env{{"XDG_SESSION_TYPE", "wayland"},
                               {"WAYLAND_DISPLAY", "wayland-0"},
                               {"DISPLAY", ":0"}}),
        "a Wayland desktop session is one");
  check(!WaylandEnvironment(Env{{"WAYLAND_DISPLAY", "wayland-0"}, {"DISPLAY", ":0"}}),
        "an X display named next to a Wayland one is X11");
  check(!WaylandEnvironment(Env{{"XDG_SESSION_TYPE", "x11"}, {"DISPLAY", ":0"}}),
        "an X11 session is not Wayland");
  check(!WaylandEnvironment(Env{{"XDG_SESSION_TYPE", "wayland"}, {"DISPLAY", ":0"}}),
        "no Wayland display, no Wayland session");
  check(WaylandEnvironment(Env{{"XDG_SESSION_TYPE", "wayland"}, {"WAYLAND_DISPLAY", ""}}),
        "an empty Wayland display name counts, as for WebRTC");
  check(WaylandEnvironment(Env{{"WAYLAND_DISPLAY", ""}, {"DISPLAY", ""}}),
        "an empty Wayland display name and no X display is Wayland");
  // Without a GTK display, WaylandSession goes by the environment.
  Environment(nullptr, "wayland-0", nullptr);
  check(WaylandSession(), "no GTK display: the environment decides");
  Environment("x11", nullptr, ":0");
  check(!WaylandSession(), "no GTK display: an X11 environment is X11");

  LibWebRTC::Initialize();

  // The environment that ended the application: WebRTC makes no capturer there.
  Environment(nullptr, "wayland-0", nullptr);
  List(0, "only WAYLAND_DISPLAY");
  // A Wayland session as WebRTC sees one: it makes no window capturer there.
  Environment("wayland", "wayland-0", nullptr);
  {
    scoped_refptr<RTCPeerConnectionFactory> factory =
        LibWebRTC::CreateRTCPeerConnectionFactory();
    factory->Initialize();
    scoped_refptr<RTCDesktopMediaList> windows =
        factory->GetDesktopDevice()->GetDesktopMediaList(kWindow);
    windows->UpdateSourceList(true, true);
    check(windows->GetSourceCount() == 0, "no windows listed in a Wayland session");
    factory->Terminate();
  }

  if (x_display == nullptr) {
    std::printf("no X display given: X11's lists not made\n");
  } else {
    // GTK on X11 decides, whatever the environment says: here it names a Wayland
    // session, and GTK still draws on the X display it opened.
    Environment("x11", nullptr, x_display);
    setenv("GDK_BACKEND", "x11", 1);
    const bool gdk = gdk_init_check(nullptr, nullptr);
    check(gdk, "GTK opens the X display");
    if (gdk) {
      Environment("wayland", "wayland-0", x_display);
      check(!WaylandSession(), "GTK on X11 is X11 though the environment says Wayland");
    }
    Environment("x11", nullptr, x_display);
    scoped_refptr<MediaSource> screen = List(1, "Xvfb");
    // The X display gone, a capturer of the screen listed before cannot be made.
    Environment(nullptr, "wayland-0", nullptr);
    if (screen.get() != nullptr) {
      scoped_refptr<RTCPeerConnectionFactory> factory =
          LibWebRTC::CreateRTCPeerConnectionFactory();
      factory->Initialize();
      scoped_refptr<RTCDesktopCapturer> capturer =
          factory->GetDesktopDevice()->CreateDesktopCapturer(screen, true);
      check(capturer.get() != nullptr &&
                capturer->Start(30) == RTCDesktopCapturer::CS_FAILED,
            "a capturer WebRTC could not make does not start");
      capturer = nullptr;
      factory->Terminate();
    }
  }
  LibWebRTC::Terminate();

  if (failures > 0) {
    std::printf("%d failed\n", failures);
    return 1;
  }
  std::printf("desktop session: all passed\n");
  return 0;
}
