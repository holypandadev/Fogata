// Checks video_head.h, which the Linux and Windows players check a file's first bytes with:
//   c++ -std=c++17 -Wall -Wextra -Werror video_head_test.cpp -o /tmp/t && /tmp/t
#include "../video_head.h"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace {

int failures = 0;

void Check(bool ok, const char* what) {
  std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
  if (!ok) failures++;
}

bool Matches(const std::vector<fogata_video::Head>& heads, const std::string& start) {
  return fogata_video::HeadMatches(
      heads, reinterpret_cast<const uint8_t*>(start.data()), start.size());
}

}  // namespace

int main() {
  using fogata_video::Head;
  // As lib/fogata_video.dart gives them for `.mp4` (two of its boxes), `.mkv` and `.avi`.
  const std::vector<Head> mp4 = {
      {{0, 0, 0, 0, 'f', 't', 'y', 'p'}, {0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}},
      {{0, 0, 0, 0, 'm', 'o', 'o', 'v'}, {0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}},
  };
  const std::vector<Head> mkv = {{{0x1A, 0x45, 0xDF, 0xA3}, {0xFF, 0xFF, 0xFF, 0xFF}}};
  const std::vector<Head> avi = {
      {{'R', 'I', 'F', 'F', 0, 0, 0, 0, 'A', 'V', 'I', ' '},
       {0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}}};
  Check(Matches(mp4, std::string("\0\0\0\x20" "ftypisom", 12)), "an MP4 matches");
  Check(Matches(mp4, std::string("\0\0\1\0" "moov....", 12)), "by any of its boxes");
  Check(!Matches(mp4, "#EXTM3U\n#EXT-X"), "a playlist named as one does not");
  Check(Matches(mkv, "\x1a\x45\xdf\xa3\x9f\x42"), "Matroska matches");
  Check(!Matches(mkv, std::string("\0\0\0\x20" "ftypisom", 12)), "an MP4 named .mkv does not");
  Check(Matches(avi, std::string("RIFF\x10\0\0\0" "AVI LIST", 16)), "AVI matches");
  Check(!Matches(avi, std::string("RIFF\x10\0\0\0" "WAVEfmt ", 16)), "a wave file does not");
  Check(!Matches(mp4, std::string("\0\0\0", 3)), "too few bytes do not");
  Check(!Matches({}, std::string("\0\0\0\x20" "ftypisom", 12)), "no heads match nothing");
  Check(!Matches({{{}, {}}}, "anything"), "nor does an empty head");
  Check(!Matches({{{'a', 'b'}, {0xFF}}}, "ab"), "nor one whose mask is not its size");
  Check(!Matches({{std::vector<uint8_t>(17, 'a'), std::vector<uint8_t>(17, 0xFF)}},
                 std::string(17, 'a')),
        "nor one longer than what is read");
  std::printf(failures == 0 ? "all passed\n" : "%d failed\n", failures);
  return failures == 0 ? 0 : 1;
}
