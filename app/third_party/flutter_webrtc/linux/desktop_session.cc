#include "desktop_session.h"

#include <gdk/gdk.h>
#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif

#include <cstdlib>

namespace flutter_webrtc_plugin {

bool WaylandEnvironment(const std::map<std::string, std::string>& environment) {
  auto value = [&](const char* name) -> const std::string* {
    auto it = environment.find(name);
    return it == environment.end() ? nullptr : &it->second;
  };
  const std::string* type = value("XDG_SESSION_TYPE");
  const std::string* x = value("DISPLAY");
  const bool says_wayland = type != nullptr && type->rfind("wayland", 0) == 0;
  return value("WAYLAND_DISPLAY") != nullptr && (says_wayland || x == nullptr || x->empty());
}

bool WaylandSession() {
  GdkDisplay* display = gdk_display_get_default();
  if (display != nullptr) {
#ifdef GDK_WINDOWING_WAYLAND
    return GDK_IS_WAYLAND_DISPLAY(display);
#else
    return false;
#endif
  }
  std::map<std::string, std::string> environment;
  for (const char* name : {"XDG_SESSION_TYPE", "WAYLAND_DISPLAY", "DISPLAY"}) {
    if (const char* value = std::getenv(name)) environment[name] = value;
  }
  return WaylandEnvironment(environment);
}

}  // namespace flutter_webrtc_plugin
