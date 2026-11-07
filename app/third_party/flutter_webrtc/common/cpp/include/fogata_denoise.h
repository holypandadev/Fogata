#ifndef FOGATA_DENOISE_H
#define FOGATA_DENOISE_H

// Fogata: the microphone's noise suppression (RNNoise, in the application's Rust library)
// as libwebrtc's capture post-processing: it runs on WebRTC's audio thread on every 10 ms
// of microphone sound, after echo cancelling and gain control, and the switch only flips
// a flag it reads, so a change applies from the next 10 ms without opening the
// microphone again.

#include <atomic>
#include <cstddef>
#include <cstdint>

#include "rtc_audio_processing.h"

namespace flutter_webrtc_plugin {

// The table of functions the application's Rust library hands over (fogata-ffi's
// denoise.rs, `DenoiseApi`).
struct FogataDenoiseApi {
  uint32_t version;
  void* (*create)();
  void (*process)(void* state, bool on, int32_t rate, float* samples,
                  size_t frames);
};

constexpr uint32_t kFogataDenoiseApiVersion = 1;

class FogataDenoiser : public libwebrtc::RTCAudioProcessing::CustomProcessing {
 public:
  // 10 ms at 48 kHz, the highest rate WebRTC processes at.
  static constexpr int kMaxFrames = 480;

  explicit FogataDenoiser(const FogataDenoiseApi* api)
      : api_(api), state_(api->create()) {}
  ~FogataDenoiser() override = default;

  void SetOn(bool on) { on_.store(on, std::memory_order_relaxed); }

  void Initialize(int, int) override {}

  // The first channel, its whole band (not split into bands), 10 ms at the rate WebRTC
  // processes at (16, 32 or 48 kHz), samples as 16-bit values in floats. Called with
  // libwebrtc's lock held, one call at a time. A frame longer than the buffer it says it
  // has, or longer than 10 ms at 48 kHz, goes on untouched.
  void Process(int, int num_frames, int buffer_size, float* buffer) override {
    if (buffer == nullptr || num_frames <= 0 || num_frames > kMaxFrames ||
        buffer_size < num_frames) {
      return;
    }
    api_->process(state_, on_.load(std::memory_order_relaxed), num_frames * 100,
                  buffer, static_cast<size_t>(num_frames));
  }

  // The rate is read from each frame.
  void Reset(int) override {}

  void Release() override {}

 private:
  const FogataDenoiseApi* api_;
  void* state_;
  std::atomic<bool> on_{false};
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_DENOISE_H
