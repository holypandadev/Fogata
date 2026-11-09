#include "flutter_video_renderer.h"

#include <cstdio>

namespace flutter_webrtc_plugin {

FlutterVideoRenderer::~FlutterVideoRenderer() {}

void FlutterVideoRenderer::initialize(
    TextureRegistrar* registrar,
    BinaryMessenger* messenger,
    TaskRunner* task_runner,
    std::unique_ptr<flutter::TextureVariant> texture,
    int64_t trxture_id) {
  registrar_ = registrar;
  texture_ = std::move(texture);
  texture_id_ = trxture_id;
  // Made once and kept as long as the renderer: the raster thread reads it after
  // CopyPixelBuffer returns, while frames of a new track may come on another thread.
  pixel_buffer_.reset(new FlutterDesktopPixelBuffer());
  pixel_buffer_->width = 0;
  pixel_buffer_->height = 0;
  std::string channel_name =
      "FlutterWebRTC/Texture" + std::to_string(texture_id_);
  event_channel_ = EventChannelProxy::Create(messenger, task_runner, channel_name);
}

const FlutterDesktopPixelBuffer* FlutterVideoRenderer::CopyPixelBuffer(
    size_t width,
    size_t height) const {
  // Fogata: only the newest frame is taken under the lock, and converted after it.
  // OnFrame waits for the lock on the thread frames come on, and converting a large
  // picture takes milliseconds: held through it, every conversion held up the next
  // frame. The buffers are only touched here, on the raster thread.
  scoped_refptr<RTCVideoFrame> frame;
  {
    std::lock_guard<std::mutex> lock(mutex_);
    frame = frame_;
  }
  if (!pixel_buffer_.get() || !frame.get() ||
      !FrameSizeAllowed(frame->width(), frame->height())) {
    return nullptr;
  }
  // The same frame asked for again (the texture drawn again with no new frame) is
  // already in the buffer.
  if (frame.get() == drawn_.get() && pixel_buffer_->buffer != nullptr) {
    return pixel_buffer_.get();
  }
  if (pixel_buffer_->width != size_t(frame->width()) ||
      pixel_buffer_->height != size_t(frame->height())) {
    size_t buffer_size =
        (size_t(frame->width()) * size_t(frame->height())) * (32 >> 3);
    rgb_buffer_.reset(new uint8_t[buffer_size]);
    pixel_buffer_->width = frame->width();
    pixel_buffer_->height = frame->height();
  }

  frame->ConvertToARGB(RTCVideoFrame::Type::kABGR, rgb_buffer_.get(), 0,
                       static_cast<int>(pixel_buffer_->width),
                       static_cast<int>(pixel_buffer_->height));

  pixel_buffer_->buffer = rgb_buffer_.get();
  drawn_ = frame;
  return pixel_buffer_.get();
}

void FlutterVideoRenderer::OnFrame(scoped_refptr<RTCVideoFrame> frame) {
  int width = frame->width();
  int height = frame->height();
  // A picture libwebrtc did not decode, being too large, comes as a small stand-in that
  // tells its size: it is dropped and told like a frame too large.
  frame->TooLarge(&width, &height);
  const bool allowed = FrameSizeAllowed(width, height);
  bool too_large = false;
  bool log_too_large = false;
  bool first = false;
  bool rotated = false;
  bool resized = false;
  const RTCVideoFrame::VideoRotation rotation = frame->rotation();
  {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!allowed) {
      too_large = !told_too_large_;
      told_too_large_ = true;
      log_too_large = !logged_too_large_;
      logged_too_large_ = true;
    } else {
      first = !first_frame_rendered;
      first_frame_rendered = true;
      rotated = rotation_ != rotation;
      rotation_ = rotation;
      resized = last_frame_size_.width != size_t(width) ||
                last_frame_size_.height != size_t(height);
      last_frame_size_ = {size_t(width), size_t(height)};
      frame_ = frame;
    }
  }
  if (log_too_large) {
    std::fprintf(stderr, "Dropping video frames of %dx%d: too large to draw\n",
                 width, height);
  }
  if (too_large) {
    // Fogata: the application keeps out of a call whoever sends such pictures.
    EncodableMap params;
    params[EncodableValue("event")] = "fogataFrameTooLarge";
    params[EncodableValue("id")] = EncodableValue(texture_id_);
    params[EncodableValue("width")] = EncodableValue((int32_t)width);
    params[EncodableValue("height")] = EncodableValue((int32_t)height);
    event_channel_->Success(EncodableValue(params));
  }
  if (!allowed) return;
  if (first) {
    EncodableMap params;
    params[EncodableValue("event")] = "didFirstFrameRendered";
    params[EncodableValue("id")] = EncodableValue(texture_id_);
    event_channel_->Success(EncodableValue(params));
  }
  if (rotated) {
    EncodableMap params;
    params[EncodableValue("event")] = "didTextureChangeRotation";
    params[EncodableValue("id")] = EncodableValue(texture_id_);
    params[EncodableValue("rotation")] = EncodableValue((int32_t)rotation);
    event_channel_->Success(EncodableValue(params));
  }
  if (resized) {
    EncodableMap params;
    params[EncodableValue("event")] = "didTextureChangeVideoSize";
    params[EncodableValue("id")] = EncodableValue(texture_id_);
    params[EncodableValue("width")] = EncodableValue((int32_t)width);
    params[EncodableValue("height")] = EncodableValue((int32_t)height);
    event_channel_->Success(EncodableValue(params));
  }
  registrar_->MarkTextureFrameAvailable(texture_id_);
}

void FlutterVideoRenderer::SetVideoTrack(scoped_refptr<RTCVideoTrack> track) {
  if (track_ == track) return;
  // The old track's frames stop before what they set is set back (libwebrtc's
  // broadcaster lets a frame on its way to the renderer arrive first). Never under
  // mutex_: OnFrame takes it while the broadcaster holds its own lock.
  if (track_) track_->RemoveRenderer(this);
  track_ = track;
  {
    std::lock_guard<std::mutex> lock(mutex_);
    last_frame_size_ = {0, 0};
    first_frame_rendered = false;
    told_too_large_ = false;
  }
  if (track_) track_->AddRenderer(this);
}

bool FlutterVideoRenderer::CheckMediaStream(std::string mediaId) {
  if (0 == mediaId.size() || 0 == media_stream_id.size()) {
    return false;
  }
  return mediaId == media_stream_id;
}

bool FlutterVideoRenderer::CheckVideoTrack(std::string mediaId) {
  if (0 == mediaId.size() || !track_) {
    return false;
  }
  return mediaId == track_->id().std_string();
}

FlutterVideoRendererManager::FlutterVideoRendererManager(
    FlutterWebRTCBase* base)
    : base_(base) {}

void FlutterVideoRendererManager::CreateVideoRendererTexture(
    std::unique_ptr<MethodResultProxy> result) {
  auto texture = new RefCountedObject<FlutterVideoRenderer>();
  auto textureVariant =
      std::make_unique<flutter::TextureVariant>(flutter::PixelBufferTexture(
          [texture](size_t width,
                    size_t height) -> const FlutterDesktopPixelBuffer* {
            return texture->CopyPixelBuffer(width, height);
          }));

  auto texture_id = base_->textures_->RegisterTexture(textureVariant.get());
  texture->initialize(base_->textures_, base_->messenger_, base_->task_runner_,
                      std::move(textureVariant), texture_id);
  renderers_[texture_id] = texture;
  EncodableMap params;
  params[EncodableValue("textureId")] = EncodableValue(texture_id);
  result->Success(EncodableValue(params));
}

void FlutterVideoRendererManager::VideoRendererSetSrcObject(
    int64_t texture_id,
    const std::string& stream_id,
    const std::string& owner_tag,
    const std::string& track_id) {
  scoped_refptr<RTCMediaStream> stream =
      base_->MediaStreamForId(stream_id, owner_tag);

  auto it = renderers_.find(texture_id);
  if (it != renderers_.end()) {
    FlutterVideoRenderer* renderer = it->second.get();
    if (stream.get()) {
      auto video_tracks = stream->video_tracks();
      if (video_tracks.size() > 0) {
        if (track_id == std::string()) {
          renderer->SetVideoTrack(video_tracks[0]);
        } else {
          for (auto track : video_tracks.std_vector()) {
            if (track->id().std_string() == track_id) {
              renderer->SetVideoTrack(track);
              break;
            }
          }
        }
        renderer->media_stream_id = stream_id;
      }
    } else {
      renderer->SetVideoTrack(nullptr);
    }
  }
}

void FlutterVideoRendererManager::VideoRendererDispose(
    int64_t texture_id,
    std::unique_ptr<MethodResultProxy> result) {
  auto it = renderers_.find(texture_id);
  if (it != renderers_.end()) {
    it->second->SetVideoTrack(nullptr);
#if defined(_WINDOWS)
    base_->textures_->UnregisterTexture(texture_id,
                                        [&, it] { renderers_.erase(it); });
#else
    base_->textures_->UnregisterTexture(texture_id);
    renderers_.erase(it);
#endif
    result->Success();
    return;
  }
  result->Error("VideoRendererDisposeFailed",
                "VideoRendererDispose() texture not found!");
}

}  // namespace flutter_webrtc_plugin
