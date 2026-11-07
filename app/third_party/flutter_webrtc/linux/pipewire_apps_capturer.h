#ifndef FLUTTER_WEBRTC_PIPEWIRE_APPS_CAPTURER_H_
#define FLUTTER_WEBRTC_PIPEWIRE_APPS_CAPTURER_H_

#include <memory>

#include "loopback_capturer.h"

namespace flutter_webrtc_plugin {

// The sound of every other application, for a shared screen: a PipeWire input
// stream linked to each application's output, except this application's own
// (its calls and its sounds), so the others never hear themselves back.
class PipeWireAppsCapturer : public LoopbackCapturer {
 public:
  PipeWireAppsCapturer();
  ~PipeWireAppsCapturer() override;

  bool Start(scoped_refptr<RTCAudioSource> source) override;
  void Stop() override;

  struct State;

 private:
  std::unique_ptr<State> state_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_PIPEWIRE_APPS_CAPTURER_H_
