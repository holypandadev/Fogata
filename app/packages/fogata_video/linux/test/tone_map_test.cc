// Tests of tone_map.h: HDR pictures (PQ, HLG, BT.2020) brought to 8-bit sRGB. Plain C++,
// no GStreamer: run_tests.sh builds and runs it on any computer with a C++ compiler.
#include "../tone_map.h"

#include <cstdio>
#include <cstdlib>

namespace {

int failed = 0;

void check(bool ok, const char* what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what);
    failed = 1;
  }
}

// The PQ signal for `nits`, by ST 2084's own formula: the inverse of what is tested.
double pq_signal(double nits) {
  const double m1 = 2610.0 / 16384;
  const double m2 = 2523.0 / 4096 * 128;
  const double c1 = 3424.0 / 4096;
  const double c2 = 2413.0 / 4096 * 32;
  const double c3 = 2392.0 / 4096 * 32;
  const double y = std::pow(nits / 10000, m1);
  return std::pow((c1 + c2 * y) / (1 + c3 * y), m2);
}

// One 16-bit RGBA pixel of grey `signal` (0 to 1) through `map`: its red byte.
uint8_t shown16(const fogata_video::ToneMap* map, double signal) {
  const uint16_t v = static_cast<uint16_t>(std::lround(signal * 65535));
  const uint16_t px[4] = {v, v, v, 65535};
  uint8_t out[4];
  fogata_video::tone_map_rows(map, reinterpret_cast<const uint8_t*>(px), sizeof px, true,
                              1, 0, 1, out);
  return out[0];
}

void colorimetry() {
  using fogata_video::colours_of;
  using fogata_video::Transfer;
  check(colours_of("bt2100-pq").transfer == Transfer::kPq && colours_of("bt2100-pq").wide,
        "bt2100-pq is PQ in BT.2020");
  check(colours_of("bt2100-hlg").transfer == Transfer::kHlg, "bt2100-hlg is HLG");
  check(colours_of("bt2020").transfer == Transfer::kStandard && colours_of("bt2020").wide,
        "bt2020 is wide, standard light");
  check(colours_of("2:6:14:7").transfer == Transfer::kPq && colours_of("2:6:14:7").wide,
        "numbers: transfer 14 is PQ, primaries 7 BT.2020");
  check(colours_of("2:6:15:1").transfer == Transfer::kHlg && !colours_of("2:6:15:1").wide,
        "numbers: transfer 15 is HLG, primaries 1 BT.709");
  check(colours_of("2:6:14:7x").transfer == Transfer::kStandard, "trailing junk is SDR");
  check(colours_of("bt709").transfer == Transfer::kStandard && !colours_of("bt709").wide,
        "bt709 is SDR");
  check(colours_of(nullptr).transfer == Transfer::kStandard, "no colorimetry is SDR");
}

void light() {
  using fogata_video::light_of;
  using fogata_video::Transfer;
  // Reference white (203 cd/m²) is light 1, in both.
  check(std::fabs(light_of(Transfer::kPq, pq_signal(203)) - 1) < 1e-3, "PQ 203 cd/m² is 1");
  check(std::fabs(light_of(Transfer::kPq, pq_signal(1000)) - 1000.0 / 203) < 1e-2,
        "PQ 1000 cd/m²");
  check(std::fabs(light_of(Transfer::kHlg, 0.75) - 1) < 0.01, "HLG 75% is reference white");
  check(light_of(Transfer::kPq, 0) < 1e-6 && light_of(Transfer::kHlg, 0) < 1e-6,
        "black is black");
}

void pictures() {
  using fogata_video::Colours;
  using fogata_video::Transfer;
  const auto* pq = fogata_video::tone_map(Colours{Transfer::kPq, false});
  // Reference white shows near white, not as the pale grey its raw value (58%) would be.
  const uint8_t white = shown16(pq, pq_signal(203));
  check(white > 225, "reference white shows bright");
  check(white > 0.58 * 255 + 40, "reference white is not its raw value");
  // A dim grey (5 cd/m²) shows dark, where its raw value would be washed out.
  check(shown16(pq, pq_signal(5)) < pq_signal(5) * 255 - 20, "a dim grey stays dark");
  // Highlights roll off: brighter keeps brighter, nothing past white is cut flat.
  const uint8_t bright = shown16(pq, pq_signal(1000));
  const uint8_t brighter = shown16(pq, pq_signal(4000));
  check(white < bright && bright < brighter, "highlights keep their detail");
  // Darker to brighter, never the other way.
  uint8_t last = 0;
  bool rising = true;
  for (int i = 0; i <= 1000; i++) {
    const uint8_t now = shown16(pq, i / 1000.0);
    rising = rising && now >= last;
    last = now;
  }
  check(rising, "brighter signals never show darker");
  check(shown16(pq, 0) == 0, "black shows black");

  // BT.2020's pure green has no red in BT.709: the negative light goes to black.
  const auto* wide = fogata_video::tone_map(Colours{Transfer::kPq, true});
  const uint16_t g = static_cast<uint16_t>(pq_signal(203) * 65535);
  const uint16_t px[4] = {0, g, 0, 65535};
  uint8_t out[4];
  fogata_video::tone_map_rows(wide, reinterpret_cast<const uint8_t*>(px), sizeof px, true, 1,
                              0, 1, out);
  check(out[0] == 0 && out[1] > 200 && out[2] == 0 && out[3] == 255,
        "BT.2020 green is BT.709 green");

  // Rows padded past their pixels (a stride longer than the row) are read where they are,
  // in 8 bits as in 16; a large picture on several threads gives the same bytes.
  const int width = 2000, height = 1200;  // past the size split over threads
  const size_t stride = width * 4 + 64;
  std::vector<uint8_t> src(stride * height, 0xEE);
  for (int y = 0; y < height; y++) {
    for (int x = 0; x < width; x++) {
      uint8_t* p = &src[y * stride + x * 4];
      p[0] = static_cast<uint8_t>(x);
      p[1] = static_cast<uint8_t>(y);
      p[2] = static_cast<uint8_t>(x + y);
      p[3] = 255;
    }
  }
  std::vector<uint8_t> whole(size_t(width) * height * 4), rows(whole.size());
  fogata_video::tone_map_picture(pq, src.data(), stride, false, width, height, whole.data());
  fogata_video::tone_map_rows(pq, src.data(), stride, false, width, 0, height, rows.data());
  check(whole == rows, "threads give the same picture");
  uint8_t one[4];
  const uint8_t px8[4] = {7, 9, 16, 255};
  fogata_video::tone_map_rows(pq, px8, 4, false, 1, 0, 1, one);
  const uint8_t* at = &whole[(9 * width + 7) * 4];
  check(std::memcmp(at, one, 4) == 0, "a padded row is read from its stride");
}

}  // namespace

int main() {
  colorimetry();
  light();
  pictures();
  if (!failed) std::printf("tone_map_test: all passed\n");
  return failed;
}
