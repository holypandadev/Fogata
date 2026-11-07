#include "loopback_capturer.h"

// Linux factory for LoopbackCapturer.
//
// When the plugin build found libpulse (HAVE_LIBPULSE, set in CMakeLists.txt)
// this returns the PulseAudio monitor-capture implementation so that
// getDisplayMedia({audio: true}) streams system audio. When libpulse is
// absent the build still succeeds and this returns nullptr, matching the old
// stub behaviour (getDisplayMedia continues without an audio track).
//
// The declaration in loopback_capturer.h is non-inline for __linux__, so this
// translation unit must always define the symbol regardless of HAVE_LIBPULSE.

#if defined(__linux__) && defined(HAVE_PIPEWIRE)
#include "pipewire_apps_capturer.h"
#endif
#if defined(__linux__) && defined(HAVE_LIBPULSE)
#include "pulse_loopback_capturer.h"
#endif

namespace flutter_webrtc_plugin {

#if defined(__linux__) && defined(HAVE_PIPEWIRE) && defined(HAVE_LIBPULSE)

namespace {

// Fogata: PipeWire's capture first; on a computer whose sound server is PulseAudio
// itself (PipeWire cannot connect), what the default output plays.
class PipeWireOrPulse : public LoopbackCapturer {
 public:
  explicit PipeWireOrPulse(std::string source_id)
      : source_id_(std::move(source_id)) {}

  bool Start(scoped_refptr<RTCAudioSource> source) override {
    used_.reset(new PipeWireAppsCapturer());
    if (used_->Start(source)) return true;
    used_.reset(new PulseLoopbackCapturer(source_id_));
    if (used_->Start(source)) return true;
    used_.reset();
    return false;
  }

  void Stop() override {
    if (used_) used_->Stop();
  }

 private:
  std::string source_id_;
  std::unique_ptr<LoopbackCapturer> used_;
};

}  // namespace

// Fogata: every other application's sound, without this one's own (its calls),
// instead of everything the default output plays.
std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& source_id,
    bool /*window*/) {
  return std::unique_ptr<LoopbackCapturer>(new PipeWireOrPulse(source_id));
}

#elif defined(__linux__) && defined(HAVE_PIPEWIRE)

// Fogata: every other application's sound, without this one's own (its calls),
// instead of everything the default output plays.
std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& /*source_id*/,
    bool /*window*/) {
  return std::unique_ptr<LoopbackCapturer>(new PipeWireAppsCapturer());
}

#elif defined(__linux__) && defined(HAVE_LIBPULSE)

std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& source_id,
    bool /*window*/) {
  return std::unique_ptr<LoopbackCapturer>(
      new PulseLoopbackCapturer(source_id));
}

#else  // libpulse not available: preserve the old nullptr behaviour.

std::unique_ptr<LoopbackCapturer> CreateLoopbackCapturer(
    const std::string& /*source_id*/,
    bool /*window*/) {
  return nullptr;
}

#endif

}  // namespace flutter_webrtc_plugin
