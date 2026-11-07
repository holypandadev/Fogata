// Pictures' rows as GStreamer hands them, made into the tightly packed rows a texture
// takes. Plain C++, tested on its own (test/run_tests.sh).
#ifndef FOGATA_VIDEO_ROWS_H_
#define FOGATA_VIDEO_ROWS_H_

#include <cstdint>
#include <cstring>
#include <vector>

namespace fogata_video {

// RGBA rows packed tightly, from rows `size / height` bytes apart. False when the bytes
// are not whole rows of at least `width` pixels.
inline bool pack_rows(std::vector<uint8_t>* rgba, int width, int height) {
  const size_t row = static_cast<size_t>(width) * 4;
  if (rgba->size() == row * height) return true;
  if (rgba->size() % height != 0) return false;
  const size_t stride = rgba->size() / height;
  if (stride < row) return false;
  for (int y = 1; y < height; y++) {
    memmove(rgba->data() + y * row, rgba->data() + y * stride, row);
  }
  rgba->resize(row * height);
  return true;
}

}  // namespace fogata_video

#endif  // FOGATA_VIDEO_ROWS_H_
