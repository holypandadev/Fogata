#ifndef FLUTTER_WEBRTC_PORTAL_I420_H_
#define FLUTTER_WEBRTC_PORTAL_I420_H_

// Fogata: a shared screen's frame from the portal (4 bytes a pixel, RGBx or BGRx) turned
// into I420 for WebRTC. Header only, with neither PipeWire nor libwebrtc, so that its
// test (common/cpp/test/portal_i420_test.cc) builds anywhere.
//
// libyuv, whose conversion uses the processor's vector instructions, is inside libwebrtc
// but not exported: the library shows only the wrapper's API (Chromium builds with hidden
// symbols, and libyuv marks its own visible only when built as a library of its own), and
// the wrapper has no call that takes 4-byte pixels. So the loops are written for the
// compiler to vectorize them in a release build (-O2/-O3): the byte order is known at
// compile time, each row is one loop over pointers that never alias what they write,
// and no pixel takes a branch (an odd last column is done after its row's loop).

#include <cstddef>
#include <cstdint>

#if defined(__GNUC__) || defined(__clang__)
#define FOGATA_RESTRICT __restrict__
#else
#define FOGATA_RESTRICT
#endif

namespace flutter_webrtc_plugin {

namespace portal_i420 {

// BT.601 limited range, as WebRTC's encoders expect.
inline uint8_t Luma(int r, int g, int b) {
  return uint8_t(((66 * r + 129 * g + 25 * b + 128) >> 8) + 16);
}

// From sums of four pixels: the shift takes the average too.
inline uint8_t ChromaU(int r, int g, int b) {
  return uint8_t(((-38 * r - 74 * g + 112 * b + 512) >> 10) + 128);
}

inline uint8_t ChromaV(int r, int g, int b) {
  return uint8_t(((112 * r - 94 * g - 18 * b + 512) >> 10) + 128);
}

template <int R, int B>
void LumaRow(const uint8_t* FOGATA_RESTRICT p, uint8_t* FOGATA_RESTRICT y, int width) {
  for (int col = 0; col < width; col++) {
    y[col] = Luma(p[4 * col + R], p[4 * col + 1], p[4 * col + B]);
  }
}

// One row of colour samples from two rows of pixels (the same row twice for the last of
// an odd height: reading it through both pointers is allowed, as neither writes).
template <int R, int B>
void ChromaRow(const uint8_t* FOGATA_RESTRICT top,
               const uint8_t* FOGATA_RESTRICT bottom,
               uint8_t* FOGATA_RESTRICT u,
               uint8_t* FOGATA_RESTRICT v,
               int width) {
  const int pairs = width / 2;
  for (int i = 0; i < pairs; i++) {
    const int at = 8 * i;
    const int r = top[at + R] + top[at + 4 + R] + bottom[at + R] + bottom[at + 4 + R];
    const int g = top[at + 1] + top[at + 5] + bottom[at + 1] + bottom[at + 5];
    const int b = top[at + B] + top[at + 4 + B] + bottom[at + B] + bottom[at + 4 + B];
    u[i] = ChromaU(r, g, b);
    v[i] = ChromaV(r, g, b);
  }
  if (width % 2 != 0) {
    // The last column of an odd width stands for itself twice.
    const int at = 8 * pairs;
    const int r = 2 * (top[at + R] + bottom[at + R]);
    const int g = 2 * (top[at + 1] + bottom[at + 1]);
    const int b = 2 * (top[at + B] + bottom[at + B]);
    u[pairs] = ChromaU(r, g, b);
    v[pairs] = ChromaV(r, g, b);
  }
}

template <int R, int B>
void Convert(const uint8_t* src, int stride, int width, int height, uint8_t* y,
             uint8_t* u, uint8_t* v) {
  const int half = (width + 1) / 2;
  for (int row = 0; row < height; row++) {
    LumaRow<R, B>(src + size_t(row) * stride, y + size_t(row) * width, width);
  }
  for (int row = 0; row < height; row += 2) {
    const uint8_t* top = src + size_t(row) * stride;
    // The last row of an odd height stands for itself twice.
    const uint8_t* bottom = row + 1 < height ? top + stride : top;
    ChromaRow<R, B>(top, bottom, u + size_t(row / 2) * half, v + size_t(row / 2) * half,
                    width);
  }
}

}  // namespace portal_i420

// Each colour sample is the average of the 2x2 block it stands for: the top-left pixel
// alone gave coloured fringes along text and thin lines. `bgr`: blue in the first byte
// (BGRx, BGRA), else red (RGBx, RGBA). `y` has `width` bytes a row, `u` and `v` half that,
// rounded up.
inline void ToI420(const uint8_t* src, int stride, int width, int height, bool bgr,
                   uint8_t* y, uint8_t* u, uint8_t* v) {
  if (bgr) {
    portal_i420::Convert<2, 0>(src, stride, width, height, y, u, v);
  } else {
    portal_i420::Convert<0, 2>(src, stride, width, height, y, u, v);
  }
}

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_PORTAL_I420_H_
