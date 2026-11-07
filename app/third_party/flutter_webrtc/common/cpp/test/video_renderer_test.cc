// The video renderer: the events it sends for the frames of a track, and its picture
// buffer, read on the raster thread while the platform thread changes the track and
// frames arrive on a third one. Built with ThreadSanitizer by run_tests.sh, which fails
// on any data race it reports.

#include <atomic>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "flutter_video_renderer.h"

using namespace flutter_webrtc_plugin;

namespace {

// A frame of one size, filled with one colour.
class Frame : public RTCVideoFrame {
 public:
  Frame(int width, int height) : width_(width), height_(height) {}
  scoped_refptr<RTCVideoFrame> Copy() override { return this; }
  int width() const override { return width_; }
  int height() const override { return height_; }
  VideoRotation rotation() override { return kVideoRotation_0; }
  const uint8_t* DataY() const override { return nullptr; }
  const uint8_t* DataU() const override { return nullptr; }
  const uint8_t* DataV() const override { return nullptr; }
  int StrideY() const override { return 0; }
  int StrideU() const override { return 0; }
  int StrideV() const override { return 0; }
  int ConvertToARGB(Type, uint8_t* dst, int, int width, int height) override {
    std::memset(dst, 0x7f, size_t(width) * size_t(height) * 4);
    return 0;
  }

 private:
  int width_, height_;
};

// What libwebrtc hands on for a picture it did not decode, as it was too large: a black
// picture of 2 by 2 that says the size refused.
class Refused : public Frame {
 public:
  Refused(int width, int height) : Frame(2, 2), width_(width), height_(height) {}
  bool TooLarge(int* width, int* height) const override {
    *width = width_;
    *height = height_;
    return true;
  }

 private:
  int width_, height_;
};

class Track : public RTCVideoTrack {
 public:
  void AddRenderer(RTCVideoRenderer<scoped_refptr<RTCVideoFrame>>*) override {}
  void RemoveRenderer(RTCVideoRenderer<scoped_refptr<RTCVideoFrame>>*) override {}
  RTCTrackState state() const override { return kLive; }
  const string kind() const override { return "video"; }
  const string id() const override { return "track"; }
  bool enabled() const override { return true; }
  bool set_enabled(bool) override { return true; }
};

class Registrar : public TextureRegistrar {
 public:
  int64_t RegisterTexture(flutter::TextureVariant*) override { return 1; }
  bool MarkTextureFrameAvailable(int64_t) override { return true; }
  bool UnregisterTexture(int64_t) override { return true; }
};

// The name of every event sent, with the size it names for a frame too large.
std::mutex events_lock;
std::vector<std::string> events;

class Events : public EventChannelProxy {
 public:
  void Success(const EncodableValue& event, bool) override {
    const EncodableMap& map = std::get<EncodableMap>(event);
    std::string name = std::get<std::string>(map.at(EncodableValue("event")));
    if (name == "fogataFrameTooLarge") {
      name += " " + std::to_string(std::get<int32_t>(map.at(EncodableValue("width")))) +
              "x" + std::to_string(std::get<int32_t>(map.at(EncodableValue("height"))));
    }
    std::lock_guard<std::mutex> lock(events_lock);
    events.push_back(name);
  }
};

// The events sent since the last call.
std::vector<std::string> Sent() {
  std::lock_guard<std::mutex> lock(events_lock);
  std::vector<std::string> sent;
  sent.swap(events);
  return sent;
}

int failures = 0;

void Expect(const std::vector<std::string>& want, const char* when) {
  const std::vector<std::string> got = Sent();
  if (got == want) return;
  failures++;
  std::fprintf(stderr, "%s: sent", when);
  for (const auto& e : got) std::fprintf(stderr, " [%s]", e.c_str());
  std::fprintf(stderr, ", wanted");
  for (const auto& e : want) std::fprintf(stderr, " [%s]", e.c_str());
  std::fprintf(stderr, "\n");
}

}  // namespace

std::unique_ptr<EventChannelProxy> EventChannelProxy::Create(
    BinaryMessenger*,
    TaskRunner*,
    const std::string&) {
  return std::make_unique<Events>();
}

// Only the renderer is under test; the manager in the same file needs this to link.
namespace flutter_webrtc_plugin {
scoped_refptr<RTCMediaStream> FlutterWebRTCBase::MediaStreamForId(
    const std::string&,
    std::string) {
  return nullptr;
}
}  // namespace flutter_webrtc_plugin

int main() {
  Registrar registrar;
  scoped_refptr<FlutterVideoRenderer> renderer =
      new RefCountedObject<FlutterVideoRenderer>();
  renderer->initialize(&registrar, nullptr, nullptr, nullptr, 1);
  scoped_refptr<RTCVideoTrack> tracks[2] = {new RefCountedObject<Track>(),
                                            new RefCountedObject<Track>()};
  scoped_refptr<RTCVideoFrame> frames[2] = {new RefCountedObject<Frame>(64, 48),
                                            new RefCountedObject<Frame>(32, 24)};
  scoped_refptr<RTCVideoFrame> giant = new RefCountedObject<Frame>(16382, 16382);

  // A frame too large to draw is told once for each track, with its size, and the
  // frames of a size drawn as before.
  renderer->SetVideoTrack(tracks[0]);
  renderer->OnFrame(giant);
  Expect({"fogataFrameTooLarge 16382x16382"}, "first frame too large");
  renderer->OnFrame(giant);
  renderer->OnFrame(frames[0]);
  Expect({"didFirstFrameRendered", "didTextureChangeVideoSize"}, "then a frame drawn");
  if (renderer->CopyPixelBuffer(0, 0)->width != 64) {
    failures++;
    std::fprintf(stderr, "the picture is not the frame drawn\n");
  }
  renderer->OnFrame(giant);
  Expect({}, "a frame too large again");
  renderer->SetVideoTrack(tracks[1]);
  renderer->OnFrame(giant);
  Expect({"fogataFrameTooLarge 16382x16382"}, "a frame too large of another track");

  // A picture refused before it was decoded is told with the size refused, and its
  // stand-in is not drawn.
  scoped_refptr<RTCVideoTrack> third = new RefCountedObject<Track>();
  scoped_refptr<RTCVideoFrame> refused = new RefCountedObject<Refused>(16383, 16383);
  renderer->SetVideoTrack(third);
  renderer->OnFrame(refused);
  Expect({"fogataFrameTooLarge 16383x16383"}, "a picture refused before decoding");
  renderer->OnFrame(refused);
  Expect({}, "a picture refused again");

  std::atomic<bool> done{false};
  // The raster thread asks for the picture and reads it, as the engine does.
  std::thread raster([&] {
    size_t read = 0;
    while (!done) {
      const FlutterDesktopPixelBuffer* pixels = renderer->CopyPixelBuffer(0, 0);
      if (pixels != nullptr && pixels->buffer != nullptr) {
        read += pixels->buffer[pixels->width * pixels->height * 4 - 1];
      }
    }
    std::printf("read %zu\n", read);
  });
  // Frames come on a thread of their own, some too large, while the platform thread
  // changes the track, as when a stream is shown in place of another.
  std::thread decoder([&] {
    for (int i = 0; !done; i++) {
      renderer->OnFrame(i % 7 == 0 ? giant : frames[(i / 2) % 2]);
    }
  });
  for (int i = 0; i < 20000; i++) {
    renderer->SetVideoTrack(tracks[i % 2]);
  }
  done = true;
  decoder.join();
  raster.join();
  renderer->SetVideoTrack(nullptr);
  if (failures > 0) {
    std::printf("video renderer: %d failed\n", failures);
    return 1;
  }
  std::printf("video renderer: ok\n");
  return 0;
}
