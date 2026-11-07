#ifndef FLUTTER_PLUGIN_FOGATA_VIDEO_PROBE_H_
#define FLUTTER_PLUGIN_FOGATA_VIDEO_PROBE_H_

// A video's first picture and length with Media Foundation, apart from Flutter so that
// test/probe_test.cpp builds it on its own; and what the players share with it.

#include <windows.h>

#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>

#include <chrono>
#include <cstdint>
#include <string>
#include <vector>

#include "../common/video_head.h"

namespace fogata_video {

// A video file opened once, read only, and checked: Media Foundation then reads it through
// byte streams over that same open file, never by its name again, so nothing put at its path
// afterwards is read. Others may still read, write, rename or delete the file meanwhile (as
// with any player): a file renamed or deleted is still read from what was opened.
class HeldVideo {
 public:
  HeldVideo() = default;
  ~HeldVideo();
  HeldVideo(const HeldVideo&) = delete;
  HeldVideo& operator=(const HeldVideo&) = delete;

  // Opens the file at `path` and checks that it begins as one of `heads`; false (and
  // nothing held) when it cannot be opened, is no plain file, or does not begin so.
  bool Hold(const std::wstring& path, const std::vector<Head>& heads);

  // A byte stream over the file held, from its start, with a position of its own: each
  // reader takes its own. It keeps the file open while it lives, whatever this does.
  bool NewStream(IMFByteStream** out) const;

  // The name asked for, whose extension tells Media Foundation the kind.
  const std::wstring& name() const { return name_; }

 private:
  HANDLE file_ = INVALID_HANDLE_VALUE;
  std::wstring name_;
};

// COM for the thread this runs on, while it lives, and Media Foundation.
struct MediaThread {
  MediaThread();
  ~MediaThread();
};

struct Probe {
  std::vector<uint8_t> jpeg;
  int width = 0;
  int height = 0;
  int64_t duration_ms = 0;
};

// How long a probe may take, all of it: opening the file, choosing the decoder and
// decoding the first picture.
constexpr std::chrono::milliseconds kProbeTime{10000};

// Reads a video's first picture as a JPEG at most `max_width` wide, its size and length.
// Every step that waits on the file or the decoder is asynchronous and gives up at `time`:
// what is still pending is cancelled, and everything is let go of before it returns.
bool ProbeVideo(const std::wstring& path, int max_width, Probe* out,
                std::chrono::milliseconds time = kProbeTime);

// The same, of what `stream` holds, which `name` (its extension) names: for tests, which
// stand a stream that stops in for a file.
bool ProbeVideoStream(IMFByteStream* stream, const std::wstring& name,
                      int max_width, Probe* out,
                      std::chrono::milliseconds time = kProbeTime);

// The same, of the video held.
bool ProbeHeld(const HeldVideo& video, int max_width, Probe* out,
               std::chrono::milliseconds time = kProbeTime);

// Whether a stream of `major` type and `subtype` may be decoded: only the codecs the chat
// offers (VP8, VP9, AV1, H.264, H.265, MPEG-4 Part 2, MJPEG; AAC, MP3, Opus, Vorbis, PCM),
// as on Linux, not every decoder the system has: decoders are the most error-prone code a
// received file reaches. Other kinds of stream are not decoded at all.
bool CodecAllowed(const GUID& major, const GUID& subtype);

// Whether every stream `reader` lists may be decoded, and it lists one at least.
bool CodecsAllowed(IMFSourceReader* reader);

// The same, of the video held; it reads the file's index to know.
bool CodecsAllowed(const HeldVideo& video);

// Whether a picture of this size is read at all.
bool SizeAllowed(UINT32 width, UINT32 height);

// The quarter turns clockwise a video's first video stream asks for (MF_MT_VIDEO_ROTATION,
// from an MP4's track matrix: phones store upright videos as sideways pictures), and the
// size its pictures are stored at. The source reader and the media engine hand the
// pictures over as stored.
struct Turns {
  int turns = 0;
  UINT32 width = 0;
  UINT32 height = 0;
};

Turns TurnsOf(IMFByteStream* stream);

// `pixels` (4 bytes each, `width` by `height`, rows packed) turned `turns` quarter turns
// clockwise; `width` and `height` become the turned picture's.
void Turn(std::vector<uint8_t>* pixels, UINT* width, UINT* height, int turns);

}  // namespace fogata_video

#endif  // FLUTTER_PLUGIN_FOGATA_VIDEO_PROBE_H_
