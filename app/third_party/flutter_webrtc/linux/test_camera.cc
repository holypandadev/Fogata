#include "test_camera.h"

#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "rtc_video_frame.h"

namespace flutter_webrtc_plugin {

namespace {

// How far the background sways, in pixels, around its middle.
constexpr int kSwayX = 24;
constexpr int kSwayY = 8;

uint8_t Clamp(int value) {
  return static_cast<uint8_t>(value < 0 ? 0 : (value > 255 ? 255 : value));
}

uint32_t Hash(uint32_t x, uint32_t y) {
  uint32_t h = x * 374761393u + y * 668265263u;
  h = (h ^ (h >> 13)) * 1274126177u;
  return h ^ (h >> 16);
}

}  // namespace

bool TestCamera::Wanted() {
#ifdef NDEBUG
  // Never in a release build.
  return false;
#else
  const char* value = std::getenv("FOGATA_TEST_CAMERA");
  return value != nullptr && std::strcmp(value, "1") == 0;
#endif
}

TestCamera::TestCamera(scoped_refptr<RTCVideoSource> source, int width,
                       int height, int fps)
    : source_(source),
      width_(width > 0 ? width & ~1 : 1280),
      height_(height > 0 ? height & ~1 : 720),
      fps_(fps > 0 ? fps : 30) {
#ifndef NDEBUG
  const char* giant = std::getenv("FOGATA_TEST_CAMERA_GIANT");
  int w = 0;
  int h = 0;
  if (giant != nullptr && std::sscanf(giant, "%dx%d", &w, &h) == 2 && w > 0 &&
      h > 0 && w <= 16383 && h <= 16383) {
    giant_width_ = w & ~1;
    giant_height_ = h & ~1;
  }
#endif
}

TestCamera::~TestCamera() { StopCapture(); }

bool TestCamera::StartCapture() {
  if (running_.exchange(true)) return true;
  thread_ = std::thread([this] { Run(); });
  return true;
}

bool TestCamera::CaptureStarted() { return running_; }

void TestCamera::StopCapture() {
  running_ = false;
  if (thread_.joinable()) thread_.join();
}

void TestCamera::Run() {
  const int w = width_;
  const int h = height_;
  const int cw = w / 2;
  const int ch = h / 2;
  // The background, larger than the picture by the sway: smooth shapes with
  // fine detail on top, as a room behind someone has.
  const int bw = w + 2 * kSwayX;
  const int bh = h + 2 * kSwayY;
  std::vector<uint8_t> back(static_cast<size_t>(bw) * bh);
  for (int y = 0; y < bh; y++) {
    for (int x = 0; x < bw; x++) {
      const double smooth = 50.0 * std::sin(x * 0.011) * std::cos(y * 0.017) +
                            25.0 * std::sin((x + y) * 0.031);
      const int detail = static_cast<int>(Hash(static_cast<uint32_t>(x / 3),
                                               static_cast<uint32_t>(y / 3)) &
                                          31u) -
                         16;
      back[static_cast<size_t>(y) * bw + x] =
          Clamp(120 + static_cast<int>(smooth) + detail);
    }
  }
  std::vector<uint8_t> y_plane(static_cast<size_t>(w) * h);
  std::vector<uint8_t> u_plane(static_cast<size_t>(cw) * ch);
  std::vector<uint8_t> v_plane(static_cast<size_t>(cw) * ch);
  // A flat grey picture: the encoder makes it small, whatever its size.
  std::vector<uint8_t> giant_y;
  std::vector<uint8_t> giant_uv;
  if (giant_width_ > 0) {
    giant_y.assign(static_cast<size_t>(giant_width_) * giant_height_, 128);
    giant_uv.assign(static_cast<size_t>(giant_width_ / 2) * (giant_height_ / 2),
                    128);
  }
  uint32_t noise = 2463534242u;
  const auto period = std::chrono::microseconds(1000000 / fps_);
  auto next = std::chrono::steady_clock::now();
  for (int64_t frame = 0; running_; frame++) {
    const double t = static_cast<double>(frame) / fps_;
    const int dx = kSwayX + static_cast<int>(kSwayX * std::sin(t * 0.5));
    const int dy = kSwayY + static_cast<int>(kSwayY * std::sin(t * 0.37));
    // Someone moving in front of it.
    const double cx = w / 2.0 + w / 4.0 * std::sin(t * 0.8);
    const double cy = h / 2.0 + h / 8.0 * std::sin(t * 1.3);
    const double rx = w / 9.0;
    const double ry = h / 4.0;
    for (int y = 0; y < h; y++) {
      const uint8_t* from = &back[static_cast<size_t>(y + dy) * bw + dx];
      uint8_t* to = &y_plane[static_cast<size_t>(y) * w];
      for (int x = 0; x < w; x += 4) {
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        for (int i = 0; i < 4 && x + i < w; i++) {
          const int grain = static_cast<int>((noise >> (8 * i)) & 3u) - 1;
          to[x + i] = Clamp(from[x + i] + grain);
        }
      }
      const double ny = (y - cy) / ry;
      if (ny * ny >= 1.0) continue;
      const double half = rx * std::sqrt(1.0 - ny * ny);
      const int x0 = static_cast<int>(cx - half) < 0 ? 0 : static_cast<int>(cx - half);
      const int x1 = static_cast<int>(cx + half) > w ? w : static_cast<int>(cx + half);
      for (int x = x0; x < x1; x++) {
        const int stripe = ((x + y + static_cast<int>(frame)) >> 3) & 1;
        to[x] = Clamp(170 + 30 * stripe + (to[x] & 7));
      }
    }
    for (int y = 0; y < ch; y++) {
      for (int x = 0; x < cw; x++) {
        const double ex = (2 * x - cx) / rx;
        const double ey = (2 * y - cy) / ry;
        const bool inside = ex * ex + ey * ey < 1.0;
        const size_t at = static_cast<size_t>(y) * cw + x;
        u_plane[at] = inside ? 110 : Clamp(128 + ((x + dx / 2) >> 4) % 24 - 12);
        v_plane[at] = inside ? 150 : Clamp(128 + ((y + dy / 2) >> 4) % 16 - 8);
      }
    }
    // In place of an ordinary one: two frames at once, the encoder keeps the last.
    if (giant_width_ > 0 && frame % fps_ == 0) {
      source_->OnCapturedFrame(RTCVideoFrame::Create(
          giant_width_, giant_height_, giant_y.data(), giant_width_,
          giant_uv.data(), giant_width_ / 2, giant_uv.data(),
          giant_width_ / 2));
    } else {
      source_->OnCapturedFrame(RTCVideoFrame::Create(
          w, h, y_plane.data(), w, u_plane.data(), cw, v_plane.data(), cw));
    }
    next += period;
    const auto now = std::chrono::steady_clock::now();
    if (next < now) {
      next = now;
    } else {
      std::this_thread::sleep_until(next);
    }
  }
}

}  // namespace flutter_webrtc_plugin
