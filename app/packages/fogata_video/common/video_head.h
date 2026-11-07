#ifndef FOGATA_VIDEO_COMMON_VIDEO_HEAD_H_
#define FOGATA_VIDEO_COMMON_VIDEO_HEAD_H_

// How a video file must begin, as the application says for its name (`videoHeads` in
// lib/fogata_video.dart): the players of Linux and Windows check it against the bytes of
// the file they opened themselves, and then read the video from that same open file.

#include <cstddef>
#include <cstdint>
#include <vector>

namespace fogata_video {

// One way a video may begin: `bytes` where `mask` has bits set, anything elsewhere.
struct Head {
  std::vector<uint8_t> bytes;
  std::vector<uint8_t> mask;
};

// The most bytes of a file's start that are read to check it.
constexpr size_t kHeadBytes = 16;

// Whether the `size` bytes at `start`, a file's first, are one of `heads`. None is a
// match for no head at all, nor for a head longer than what was read.
inline bool HeadMatches(const std::vector<Head>& heads, const uint8_t* start,
                        size_t size) {
  for (const Head& head : heads) {
    if (head.bytes.empty() || head.mask.size() != head.bytes.size() ||
        head.bytes.size() > size || head.bytes.size() > kHeadBytes) {
      continue;
    }
    bool same = true;
    for (size_t i = 0; i < head.bytes.size() && same; i++) {
      same = (start[i] & head.mask[i]) == (head.bytes[i] & head.mask[i]);
    }
    if (same) return true;
  }
  return false;
}

}  // namespace fogata_video

#endif  // FOGATA_VIDEO_COMMON_VIDEO_HEAD_H_
