// A shared screen's frame from the portal turned into I420 (linux/portal_i420.h): byte
// for byte what the plain per-pixel loop it replaced gave, for both byte orders, odd and
// even sizes and rows padded past the picture. With an argument, the number of 2560x1440
// frames to time (`portal_i420_test 300`), to compare a build's speed. Plain C++.

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <string>
#include <vector>

#include "portal_i420.h"

using flutter_webrtc_plugin::ToI420;

namespace {

int failures = 0;

// The conversion as it was, one pixel at a time.
void Reference(const uint8_t* src, int stride, int width, int height, bool bgr,
               uint8_t* y, uint8_t* u, uint8_t* v) {
  const int r_at = bgr ? 2 : 0;
  const int b_at = bgr ? 0 : 2;
  const int half = (width + 1) / 2;
  for (int row = 0; row < height; row++) {
    const uint8_t* p = src + size_t(row) * stride;
    uint8_t* yrow = y + size_t(row) * width;
    for (int col = 0; col < width; col++, p += 4) {
      yrow[col] = uint8_t(((66 * p[r_at] + 129 * p[1] + 25 * p[b_at] + 128) >> 8) + 16);
    }
  }
  for (int row = 0; row < height; row += 2) {
    const uint8_t* top = src + size_t(row) * stride;
    const uint8_t* bottom = row + 1 < height ? top + stride : top;
    uint8_t* urow = u + size_t(row / 2) * half;
    uint8_t* vrow = v + size_t(row / 2) * half;
    for (int col = 0; col < width; col += 2) {
      const size_t left = size_t(col) * 4;
      const size_t right = col + 1 < width ? left + 4 : left;
      const int r = top[left + r_at] + top[right + r_at] + bottom[left + r_at] +
                    bottom[right + r_at];
      const int g = top[left + 1] + top[right + 1] + bottom[left + 1] + bottom[right + 1];
      const int b = top[left + b_at] + top[right + b_at] + bottom[left + b_at] +
                    bottom[right + b_at];
      urow[col / 2] = uint8_t(((-38 * r - 74 * g + 112 * b + 512) >> 10) + 128);
      vrow[col / 2] = uint8_t(((112 * r - 94 * g - 18 * b + 512) >> 10) + 128);
    }
  }
}

size_t I420Size(int width, int height) {
  return size_t(width) * height + 2 * size_t((width + 1) / 2) * ((height + 1) / 2);
}

void Same(int width, int height, int padding, bool bgr, std::mt19937& random) {
  const int stride = width * 4 + padding;
  std::vector<uint8_t> src(size_t(stride) * height);
  for (auto& byte : src) byte = uint8_t(random());
  // Flat extremes too: all black and all white rows.
  if (height > 2) {
    std::memset(src.data(), 0, size_t(stride));
    std::memset(src.data() + stride, 255, size_t(stride));
  }
  const size_t size = I420Size(width, height);
  std::vector<uint8_t> want(size), got(size, 0xAB);
  const int half = (width + 1) / 2;
  const size_t luma = size_t(width) * height;
  const size_t chroma = size_t(half) * ((height + 1) / 2);
  Reference(src.data(), stride, width, height, bgr, want.data(), want.data() + luma,
            want.data() + luma + chroma);
  ToI420(src.data(), stride, width, height, bgr, got.data(), got.data() + luma,
         got.data() + luma + chroma);
  if (want != got) {
    std::printf("FAIL: %dx%d (stride %d, %s) differs\n", width, height, stride,
                bgr ? "BGRx" : "RGBx");
    failures++;
  }
}

void Time(int frames) {
  const int width = 2560, height = 1440, stride = width * 4;
  std::vector<uint8_t> src(size_t(stride) * height);
  std::mt19937 random(7);
  for (auto& byte : src) byte = uint8_t(random());
  std::vector<uint8_t> out(I420Size(width, height));
  const size_t luma = size_t(width) * height;
  const size_t chroma = size_t(width / 2) * (height / 2);
  for (int pass = 0; pass < 2; pass++) {
    const auto start = std::chrono::steady_clock::now();
    unsigned sum = 0;
    for (int i = 0; i < frames; i++) {
      // A different picture each time, and the result read: nothing for the compiler
      // to leave out.
      src[size_t(i) % src.size()] ^= 1;
      if (pass == 0) {
        Reference(src.data(), stride, width, height, true, out.data(), out.data() + luma,
                  out.data() + luma + chroma);
      } else {
        ToI420(src.data(), stride, width, height, true, out.data(), out.data() + luma,
               out.data() + luma + chroma);
      }
      sum += out[size_t(i) % out.size()];
    }
    const double ms = std::chrono::duration<double, std::milli>(
                          std::chrono::steady_clock::now() - start)
                          .count();
    std::printf("%s: %.2f ms a 2560x1440 frame (%u)\n",
                pass == 0 ? "per pixel" : "by rows", ms / frames, sum);
  }
}

}  // namespace

int main(int argc, char** argv) {
  std::mt19937 random(1);
  const int sizes[][2] = {{1, 1},   {2, 2},   {3, 3},   {1, 4},    {4, 1},    {5, 4},
                          {16, 9},  {17, 9},  {33, 31}, {640, 360}, {641, 361}, {1366, 768}};
  for (const auto& size : sizes) {
    for (int padding : {0, 4, 60}) {
      Same(size[0], size[1], padding, true, random);
      Same(size[0], size[1], padding, false, random);
    }
  }
  if (argc > 1) Time(std::atoi(argv[1]));
  if (failures == 0) std::printf("portal_i420_test: ok\n");
  return failures == 0 ? 0 : 1;
}
