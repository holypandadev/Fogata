#ifndef FLUTTER_WEBRTC_RTC_VIDEO_RENDERER_HXX
#define FLUTTER_WEBRTC_RTC_VIDEO_RENDERER_HXX

#include "flutter_common.h"
#include "flutter_webrtc_base.h"

#include "rtc_video_frame.h"
#include "rtc_video_renderer.h"

#include <mutex>

namespace flutter_webrtc_plugin {

using namespace libwebrtc;

// The largest picture drawn from a video track: 8K (7680 by 4320 pixels, the largest
// screen shared as captured), and no side longer than 8192. Each picture is copied into
// width * height * 4 bytes, and a peer can send frames up to the codec's 16383 by 16383,
// a gigabyte each; larger frames are dropped, and the first of each track is told
// (event "fogataFrameTooLarge", with its width and height).
constexpr size_t kMaxFrameSide = 8192;
constexpr size_t kMaxFramePixels = size_t(7680) * 4320;

inline bool FrameSizeAllowed(int width, int height) {
  return width > 0 && height > 0 && size_t(width) <= kMaxFrameSide &&
         size_t(height) <= kMaxFrameSide &&
         size_t(width) * size_t(height) <= kMaxFramePixels;
}

class FlutterVideoRenderer
    : public RTCVideoRenderer<scoped_refptr<RTCVideoFrame>>,
      public RefCountInterface {
 public:
  FlutterVideoRenderer() = default;
  ~FlutterVideoRenderer();

  void initialize(TextureRegistrar* registrar,
                  BinaryMessenger* messenger,
                  TaskRunner* task_runner,
                  std::unique_ptr<flutter::TextureVariant> texture,
                  int64_t texture_id);

  virtual const FlutterDesktopPixelBuffer* CopyPixelBuffer(size_t width,
                                                           size_t height) const;

  virtual void OnFrame(scoped_refptr<RTCVideoFrame> frame) override;

  void SetVideoTrack(scoped_refptr<RTCVideoTrack> track);

  int64_t texture_id() { return texture_id_; }

  bool CheckMediaStream(std::string mediaId);

  bool CheckVideoTrack(std::string mediaId);

  std::string media_stream_id;

 private:
  struct FrameSize {
    size_t width;
    size_t height;
  };
  // Set by OnFrame, on the thread frames come on, and set back by SetVideoTrack, on
  // the platform thread: under mutex_.
  FrameSize last_frame_size_ = {0, 0};
  bool first_frame_rendered = false;
  bool told_too_large_ = false;
  bool logged_too_large_ = false;
  RTCVideoFrame::VideoRotation rotation_ = RTCVideoFrame::kVideoRotation_0;
  TextureRegistrar* registrar_ = nullptr;
  std::unique_ptr<EventChannelProxy> event_channel_;
  int64_t texture_id_ = -1;
  scoped_refptr<RTCVideoTrack> track_ = nullptr;
  scoped_refptr<RTCVideoFrame> frame_;
  std::unique_ptr<flutter::TextureVariant> texture_;
  std::shared_ptr<FlutterDesktopPixelBuffer> pixel_buffer_;
  mutable std::shared_ptr<uint8_t[]> rgb_buffer_;
  // The frame rgb_buffer_ holds, converted by CopyPixelBuffer on the raster thread.
  mutable scoped_refptr<RTCVideoFrame> drawn_;
  mutable std::mutex mutex_;
};

class FlutterVideoRendererManager {
 public:
  FlutterVideoRendererManager(FlutterWebRTCBase* base);

  void CreateVideoRendererTexture(std::unique_ptr<MethodResultProxy> result);

  void VideoRendererSetSrcObject(int64_t texture_id,
                                 const std::string& stream_id,
                                 const std::string& owner_tag,
                                 const std::string& track_id);

  void VideoRendererDispose(int64_t texture_id,
                            std::unique_ptr<MethodResultProxy> result);

 private:
  FlutterWebRTCBase* base_;
  std::map<int64_t, scoped_refptr<FlutterVideoRenderer>> renderers_;
};

}  // namespace flutter_webrtc_plugin

#endif  // !FLUTTER_WEBRTC_RTC_VIDEO_RENDERER_HXX