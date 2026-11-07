// Tests of rows.h: a picture's rows packed tightly, whatever padding GStreamer gave them.
#include "../rows.h"

#include <cstdio>

namespace {

int failed = 0;

void check(bool ok, const char* what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what);
    failed = 1;
  }
}

// A picture `width` by `height` whose rows are `stride` bytes apart; the padding is 0xEE.
std::vector<uint8_t> picture(int width, int height, size_t stride) {
  std::vector<uint8_t> bytes(stride * height, 0xEE);
  for (int y = 0; y < height; y++) {
    for (int i = 0; i < width * 4; i++) bytes[y * stride + i] = uint8_t(y * 31 + i);
  }
  return bytes;
}

}  // namespace

int main() {
  // Packed already: as it is.
  auto packed = picture(3, 2, 12);
  const auto before = packed;
  check(fogata_video::pack_rows(&packed, 3, 2) && packed == before, "packed rows stay");
  // Rows padded to a multiple of 64 bytes, as decoders often give them: the padding goes,
  // the pixels stay where they were in their row.
  auto padded = picture(5, 3, 64);
  check(fogata_video::pack_rows(&padded, 5, 3), "padded rows are taken");
  check(padded == picture(5, 3, 20), "padded rows are packed");
  // Not whole rows, or rows shorter than the picture: refused.
  std::vector<uint8_t> odd(5 * 4 * 3 + 1);
  check(!fogata_video::pack_rows(&odd, 5, 3), "bytes that are not whole rows");
  std::vector<uint8_t> short_rows(4 * 4 * 3);
  check(!fogata_video::pack_rows(&short_rows, 5, 3), "rows shorter than the picture");
  if (!failed) std::printf("rows_test: all passed\n");
  return failed;
}
