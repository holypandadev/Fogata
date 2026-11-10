#ifndef FLUTTER_WEBRTC_PORTAL_SCREEN_CAPTURER_H_
#define FLUTTER_WEBRTC_PORTAL_SCREEN_CAPTURER_H_

#include <cstdint>
#include <functional>
#include <memory>
#include <string>

namespace flutter_webrtc_plugin {

// Captures a screen or a window chosen in the system's own dialog (the
// xdg-desktop-portal ScreenCast interface) and receives its frames from
// PipeWire. Under Wayland applications cannot look at the screen themselves;
// this asks once, lets the person pick a window as well as a screen, and only
// ever runs one capture.
class PortalScreenCapturer {
 public:
  // A frame as I420 planes.
  using FrameCallback = std::function<void(int width,
                                           int height,
                                           const uint8_t* y,
                                           int stride_y,
                                           const uint8_t* u,
                                           int stride_u,
                                           const uint8_t* v,
                                           int stride_v)>;
  // The person chose something (true) or the dialog failed or was cancelled.
  using StartedCallback =
      std::function<void(bool started, const std::string& error)>;
  // The capture ended from outside: the system's own "stop sharing", or what was
  // shared went away.
  using EndedCallback = std::function<void()>;

  // A Wayland session (WaylandSession in desktop_session.h), and this build has
  // PipeWire.
  static bool Available();

  PortalScreenCapturer();
  ~PortalScreenCapturer();

  // Shows the system's dialog. [started] runs on the GLib main thread when the
  // person has chosen or cancelled; frames then go to [on_frame] from
  // PipeWire's thread, at most [fps] a second, and [ended] runs on the GLib main
  // thread if the system ends the capture.
  void Start(bool cursor,
             int fps,
             FrameCallback on_frame,
             StartedCallback started,
             EndedCallback ended);

  // From now on, at most [fps] frames a second go to [on_frame]; no more than the
  // rate the capture was started at, which the system was asked for.
  void SetFps(int fps);

  // Once started: what was chosen is a whole screen, as far as the portal tells.
  // A window, or a choice the portal does not tell (one older than version 3
  // that offered windows too), is not.
  bool SharesScreen() const;

  // Ends the capture and the portal's session. Safe to call more than once.
  void Stop();

  struct State;

 private:
  std::shared_ptr<State> state_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_PORTAL_SCREEN_CAPTURER_H_
