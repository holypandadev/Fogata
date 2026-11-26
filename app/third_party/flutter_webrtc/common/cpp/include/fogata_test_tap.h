#ifndef FOGATA_TEST_TAP_H
#define FOGATA_TEST_TAP_H

// Fogata, for tests only: with FOGATA_TEST_TAP_DIR=<folder>, the sound at two points of a
// call goes to files there, to find where a word loses its level between the microphone and
// what the other side plays (tools of the A6 "after the APM" measurement):
//   tap-apm-<role>      the microphone's sound at the end of WebRTC's processing (echo
//                       cancelling, gain control), before RNNoise (FogataDenoiser::Process),
//                       at the processing's rate;
//   tap-neteq-<role>[-N] each received voice as its jitter buffer (NetEq) gives it out, before
//                       the volume and the mixing (a sink on the received track; N counts
//                       the tracks after the first).
// <role> is FOGATA_ROLE (the call lab's a, b...), else the process id. Without the variable
// nothing is made, written or read, and nothing else changes.
//
// Each tap is <name>.raw (16-bit little-endian mono; several channels are averaged),
// <name>.start (the epoch ms of its first sample, as the call lab's recordings have) and
// <name>.times, which places every sample on the wall clock: the sound comes in frames of
// 10 ms, in bursts and with gaps (a recording that restarts gives none for a while), so the
// samples are cut into segments of one rate with no gap, each with its own start:
//   seg  <first sample> <start ms> <rate>   a new segment (a gap of over 40 ms, or another rate)
//   mark <first sample> <start ms> <rate>   once a second: the segment's start, refined
// A segment's start is the earliest its frames allow (a frame is handed over no sooner than
// its last sample is there, and later in a burst): the last line of a segment holds its best
// start. Times are the wall clock's (epoch ms, with fractions).

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <mutex>
#include <string>
#include <vector>

#ifndef _WIN32
#include <unistd.h>
#else
#include <process.h>
#endif

namespace flutter_webrtc_plugin {

// FOGATA_TEST_TAP_DIR, read once; empty without it.
inline const std::string& FogataTapDir() {
  static const std::string dir = [] {
#ifdef _WIN32
    char* value = nullptr;
    size_t length = 0;
    if (_dupenv_s(&value, &length, "FOGATA_TEST_TAP_DIR") != 0 || value == nullptr) {
      return std::string();
    }
    std::string result(value);
    free(value);
    return result;
#else
    const char* value = std::getenv("FOGATA_TEST_TAP_DIR");
    return std::string(value ? value : "");
#endif
  }();
  return dir;
}

inline std::string FogataTapRole() {
#ifdef _WIN32
  char* value = nullptr;
  size_t length = 0;
  std::string role;
  if (_dupenv_s(&value, &length, "FOGATA_ROLE") == 0 && value != nullptr) {
    role = value;
    free(value);
  }
  if (role.empty()) role = "pid" + std::to_string(_getpid());
#else
  const char* value = std::getenv("FOGATA_ROLE");
  std::string role = value ? value : "";
  if (role.empty()) role = "pid" + std::to_string(getpid());
#endif
  return role;
}

class FogataTapWriter {
 public:
  // <dir>/<name>.raw, .start and .times, opened with the first frame.
  explicit FogataTapWriter(std::string name) : name_(std::move(name)) {}

  ~FogataTapWriter() {
    if (raw_) std::fclose(raw_);
    if (times_) std::fclose(times_);
  }

  FogataTapWriter(const FogataTapWriter&) = delete;
  FogataTapWriter& operator=(const FogataTapWriter&) = delete;

  // One frame of mono samples in the 16-bit range (floats, as the audio processing has them).
  void WriteFloat(const float* samples, size_t frames, int rate) {
    if (frames == 0 || rate <= 0) return;
    scratch_.resize(frames);
    for (size_t i = 0; i < frames; i++) {
      const float v = std::round(samples[i]);
      scratch_[i] = static_cast<int16_t>(std::min(32767.0f, std::max(-32768.0f, v)));
    }
    Write(scratch_.data(), frames, rate);
  }

  // One frame of 16-bit interleaved samples, its channels averaged.
  void WriteInterleaved(const int16_t* samples, size_t channels, size_t frames, int rate) {
    if (frames == 0 || rate <= 0 || channels == 0) return;
    scratch_.resize(frames);
    for (size_t i = 0; i < frames; i++) {
      int sum = 0;
      for (size_t c = 0; c < channels; c++) sum += samples[i * channels + c];
      scratch_[i] = static_cast<int16_t>(sum / static_cast<int>(channels));
    }
    Write(scratch_.data(), frames, rate);
  }

 private:
  static double NowMs() {
    return std::chrono::duration<double, std::milli>(
               std::chrono::system_clock::now().time_since_epoch())
        .count();
  }

  bool Open() {
    if (failed_) return false;
    const std::string base = FogataTapDir() + "/" + name_;
#ifdef _WIN32
    if (fopen_s(&raw_, (base + ".raw").c_str(), "wb") != 0) raw_ = nullptr;
    if (fopen_s(&times_, (base + ".times").c_str(), "w") != 0) times_ = nullptr;
#else
    raw_ = std::fopen((base + ".raw").c_str(), "wb");
    times_ = std::fopen((base + ".times").c_str(), "w");
#endif
    if (raw_ == nullptr || times_ == nullptr) {
      std::fprintf(stderr, "Fogata: tap %s cannot be written\n", base.c_str());
      failed_ = true;
      return false;
    }
    return true;
  }

  void Write(const int16_t* samples, size_t frames, int rate) {
    std::lock_guard<std::mutex> guard(mutex_);
    const double now = NowMs();
    if (raw_ == nullptr && !Open()) return;
    // The frame's first sample was there at the latest one frame before now.
    const double first = now - frames * 1000.0 / rate;
    if (written_ == 0) {
      const std::string start = FogataTapDir() + "/" + name_ + ".start";
      std::FILE* f = nullptr;
#ifdef _WIN32
      if (fopen_s(&f, start.c_str(), "w") != 0) f = nullptr;
#else
      f = std::fopen(start.c_str(), "w");
#endif
      if (f) {
        std::fprintf(f, "%lld\n", static_cast<long long>(std::llround(first)));
        std::fclose(f);
      }
    }
    // Where the segment so far puts this frame's first sample.
    const double candidate = first - segment_samples_ * 1000.0 / rate;
    if (rate != rate_ || segment_samples_ == 0 || candidate - segment_start_ > 40.0) {
      rate_ = rate;
      segment_start_ = first;
      segment_samples_ = 0;
      segment_first_ = written_;
      std::fprintf(times_, "seg %llu %.3f %d\n",
                   static_cast<unsigned long long>(segment_first_), segment_start_, rate_);
      next_mark_ = written_ + static_cast<uint64_t>(rate);
    } else {
      segment_start_ = std::min(segment_start_, candidate);
    }
    std::fwrite(samples, sizeof(int16_t), frames, raw_);
    written_ += frames;
    segment_samples_ += frames;
    if (written_ >= next_mark_) {
      std::fprintf(times_, "mark %llu %.3f %d\n",
                   static_cast<unsigned long long>(segment_first_), segment_start_, rate_);
      next_mark_ = written_ + static_cast<uint64_t>(rate);
      // At most a second lost if the process is killed.
      std::fflush(raw_);
      std::fflush(times_);
    }
  }

  const std::string name_;
  std::mutex mutex_;
  std::FILE* raw_ = nullptr;
  std::FILE* times_ = nullptr;
  bool failed_ = false;
  std::vector<int16_t> scratch_;
  uint64_t written_ = 0;
  uint64_t next_mark_ = 0;
  uint64_t segment_first_ = 0;
  uint64_t segment_samples_ = 0;
  double segment_start_ = 0;
  int rate_ = 0;
};

// A writer for the tap of that kind ("apm", "neteq"): <kind>-<role>, then -2, -3... for
// the next of the same kind in this process. Null without FOGATA_TEST_TAP_DIR. Never
// freed: the threads that write may outlive whatever made them.
inline FogataTapWriter* FogataNewTap(const std::string& kind) {
  if (FogataTapDir().empty()) return nullptr;
  static std::mutex* mutex = new std::mutex();
  static std::map<std::string, int>* counts = new std::map<std::string, int>();
  std::lock_guard<std::mutex> guard(*mutex);
  const int n = ++(*counts)[kind];
  std::string name = "tap-" + kind + "-" + FogataTapRole();
  if (n > 1) name += "-" + std::to_string(n);
  return new FogataTapWriter(name);
}

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_TEST_TAP_H
