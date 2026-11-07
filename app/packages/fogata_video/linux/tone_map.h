// HDR pictures (PQ or HLG, usually in BT.2020's wider colours) brought down to the 8-bit
// sRGB pixels a texture shows. Shown as they are, their values look washed out: grey,
// flat and pale. Each value becomes light, in units of HDR's reference white (203 cd/m²,
// BT.2408), BT.2020's colours become BT.709's, and light past the white of the screen
// rolls off smoothly instead of being cut, so highlights keep some detail.
#ifndef FOGATA_VIDEO_TONE_MAP_H_
#define FOGATA_VIDEO_TONE_MAP_H_

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

namespace fogata_video {

// How a picture's values turn into light, from its caps' colorimetry.
enum class Transfer { kStandard, kPq, kHlg };

struct Colours {
  Transfer transfer = Transfer::kStandard;
  // BT.2020's primaries.
  bool wide = false;
};

// GStreamer writes a colorimetry by name ("bt2100-pq") or as
// "range:matrix:transfer:primaries" numbers (transfer 14 is PQ, 15 HLG; primaries 7 is
// BT.2020).
inline Colours colours_of(const char* colorimetry) {
  Colours c;
  if (colorimetry == nullptr) return c;
  const std::string text = colorimetry;
  if (text == "bt2100-pq") return {Transfer::kPq, true};
  if (text == "bt2100-hlg") return {Transfer::kHlg, true};
  if (text == "bt2020" || text == "bt2020-10") {
    c.wide = true;
    return c;
  }
  unsigned fields[4] = {0, 0, 0, 0};
  int used = 0;
  if (std::sscanf(colorimetry, "%u:%u:%u:%u%n", &fields[0], &fields[1], &fields[2],
                  &fields[3], &used) != 4 ||
      colorimetry[used] != '\0') {
    return c;
  }
  if (fields[2] == 14) c.transfer = Transfer::kPq;
  if (fields[2] == 15) c.transfer = Transfer::kHlg;
  c.wide = fields[3] == 7;
  return c;
}

// The output table is indexed by the top bits of the light as a float: 7 bits of
// mantissa, from 2^-14 (darker than any screen shows) to 64 times reference white.
constexpr int kDroppedBits = 16;
constexpr int32_t kLowest = 0x38800000 >> kDroppedBits;
constexpr int32_t kHighest = 0x42800000 >> kDroppedBits;

struct ToneMap {
  bool wide = false;
  // A 12-bit value to light.
  float in[4096];
  // Light to an sRGB byte.
  uint8_t out[kHighest - kLowest + 1];
};

inline double light_of(Transfer transfer, double value) {
  if (transfer == Transfer::kPq) {
    // SMPTE ST 2084: up to 10000 cd/m².
    const double m1 = 2610.0 / 16384;
    const double m2 = 2523.0 / 4096 * 128;
    const double c1 = 3424.0 / 4096;
    const double c2 = 2413.0 / 4096 * 32;
    const double c3 = 2392.0 / 4096 * 32;
    const double p = std::pow(value, 1 / m2);
    return std::pow(std::max(p - c1, 0.0) / (c2 - c3 * p), 1 / m1) * 10000 / 203;
  }
  // ARIB STD-B67 (HLG) on a 1000 cd/m² screen: the scene's light, then the screen's
  // gamma of 1.2, here on each colour rather than on the luminance.
  const double a = 0.17883277;
  const double b = 1 - 4 * a;
  const double c = 0.5 - a * std::log(4 * a);
  const double scene =
      value <= 0.5 ? value * value / 3 : (std::exp((value - c) / a) + b) / 12;
  return std::pow(scene, 1.2) * 1000 / 203;
}

// Light to the screen's: reference white and below as it is, up to 0.7, then rolled
// off towards 1, with the slope kept where the roll-off starts.
inline double rolled_off(double light) {
  const double knee = 0.7;
  if (light <= knee) return light;
  const double over = light - knee;
  return knee + (1 - knee) * over / (over + 1 - knee);
}

inline uint8_t srgb_byte(double light) {
  const double v =
      light <= 0.0031308 ? 12.92 * light : 1.055 * std::pow(light, 1 / 2.4) - 0.055;
  return static_cast<uint8_t>(std::lround(std::clamp(v, 0.0, 1.0) * 255));
}

inline const ToneMap* tone_map(Colours colours) {
  auto make = [](Transfer transfer, bool wide) {
    auto* map = new ToneMap();
    map->wide = wide;
    for (int i = 0; i < 4096; i++) {
      map->in[i] = static_cast<float>(light_of(transfer, i / 4095.0));
    }
    for (int32_t i = kLowest; i <= kHighest; i++) {
      // The middle of the floats this entry stands for.
      const uint32_t bits = static_cast<uint32_t>(i) << kDroppedBits |
                            1u << (kDroppedBits - 1);
      float light;
      memcpy(&light, &bits, sizeof light);
      map->out[i - kLowest] = srgb_byte(i == kLowest ? 0.0 : rolled_off(light));
    }
    return map;
  };
  static const ToneMap* const maps[2][2] = {
      {make(Transfer::kPq, false), make(Transfer::kPq, true)},
      {make(Transfer::kHlg, false), make(Transfer::kHlg, true)}};
  return maps[colours.transfer == Transfer::kHlg][colours.wide];
}

inline int32_t out_index(float light) {
  int32_t bits;
  memcpy(&bits, &light, sizeof bits);
  // A negative light (a colour outside BT.709's) is a negative int, and goes to black.
  return std::clamp(bits >> kDroppedBits, kLowest, kHighest) - kLowest;
}

// Rows `first` to `last` (not included) of `src`, `stride` bytes apart, of RGBA pixels of
// 8 or 16 bits a value (native order), into packed RGBA bytes at `dst`.
inline void tone_map_rows(const ToneMap* map, const uint8_t* src, size_t stride,
                          bool sixteen, int width, int first, int last, uint8_t* dst) {
  std::vector<float> r(width), g(width), b(width);
  for (int y = first; y < last; y++) {
    const uint8_t* row = src + static_cast<size_t>(y) * stride;
    if (sixteen) {
      for (int x = 0; x < width; x++) {
        uint16_t v[3];
        memcpy(v, row + static_cast<size_t>(x) * 8, sizeof v);
        r[x] = map->in[v[0] >> 4];
        g[x] = map->in[v[1] >> 4];
        b[x] = map->in[v[2] >> 4];
      }
    } else {
      for (int x = 0; x < width; x++) {
        const uint8_t* p = row + static_cast<size_t>(x) * 4;
        r[x] = map->in[p[0] * 4095 / 255];
        g[x] = map->in[p[1] * 4095 / 255];
        b[x] = map->in[p[2] * 4095 / 255];
      }
    }
    if (map->wide) {
      // BT.2020 to BT.709, in linear light (BT.2087).
      for (int x = 0; x < width; x++) {
        const float red = r[x];
        const float green = g[x];
        const float blue = b[x];
        r[x] = 1.6605f * red - 0.5876f * green - 0.0728f * blue;
        g[x] = -0.1246f * red + 1.1329f * green - 0.0083f * blue;
        b[x] = -0.0182f * red - 0.1006f * green + 1.1187f * blue;
      }
    }
    uint8_t* out = dst + static_cast<size_t>(y) * width * 4;
    for (int x = 0; x < width; x++, out += 4) {
      out[0] = map->out[out_index(r[x])];
      out[1] = map->out[out_index(g[x])];
      out[2] = map->out[out_index(b[x])];
      out[3] = 255;
    }
  }
}

// The whole picture; a large one on up to four threads, so a 4K video keeps its rate.
inline void tone_map_picture(const ToneMap* map, const uint8_t* src, size_t stride,
                             bool sixteen, int width, int height, uint8_t* dst) {
  const int64_t pixels = static_cast<int64_t>(width) * height;
  const int threads = pixels > 2100000
                          ? static_cast<int>(std::clamp(
                                std::thread::hardware_concurrency() / 2, 1u, 4u))
                          : 1;
  if (threads == 1) {
    tone_map_rows(map, src, stride, sixteen, width, 0, height, dst);
    return;
  }
  std::vector<std::thread> helpers;
  const int part = (height + threads - 1) / threads;
  for (int i = 1; i < threads; i++) {
    const int first = std::min(height, i * part);
    const int last = std::min(height, first + part);
    helpers.emplace_back(tone_map_rows, map, src, stride, sixteen, width, first, last,
                         dst);
  }
  tone_map_rows(map, src, stride, sixteen, width, 0, std::min(height, part), dst);
  for (auto& helper : helpers) helper.join();
}

}  // namespace fogata_video

#endif  // FOGATA_VIDEO_TONE_MAP_H_
