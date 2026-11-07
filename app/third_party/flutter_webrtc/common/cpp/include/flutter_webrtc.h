#ifndef PLUGINS_FLUTTER_WEBRTC_HXX
#define PLUGINS_FLUTTER_WEBRTC_HXX

#include "flutter_common.h"

#include <functional>
#include <memory>
#include <mutex>

#include "flutter_data_channel.h"
#include "flutter_media_stream.h"
#include "flutter_peerconnection.h"
#include "flutter_screen_capture.h"
#include "flutter_video_renderer.h"

#include "libwebrtc.h"
#include "rtc_logging.h"

namespace flutter_webrtc_plugin {

using namespace libwebrtc;

class FlutterWebRTCPlugin : public flutter::Plugin {
 public:
  virtual BinaryMessenger* messenger() = 0;

  virtual TextureRegistrar* textures() = 0;

  virtual TaskRunner* task_runner() = 0;
};

class FlutterWebRTC : public FlutterWebRTCBase,
                      public FlutterVideoRendererManager,
                      public FlutterMediaStream,
                      public FlutterPeerConnection,
                      public FlutterScreenCapture,
                      public FlutterDataChannel {
 public:
  FlutterWebRTC(FlutterWebRTCPlugin* plugin);
  virtual ~FlutterWebRTC();

  void HandleMethodCall(const MethodCallProxy& method_call,
                        std::unique_ptr<MethodResultProxy> result);

  // Fogata: whether answers of checks running on threads of their own (the sound
  // server's) may still be given; closed, under its lock, when the plugin goes.
  struct AnswerGate {
    std::mutex lock;
    bool open = true;
  };

 private:
  // Fogata: asks for the cameras where they come through a portal (see
  // RTCVideoDevice::RequestCameraAccess), then runs [then] on the platform
  // thread, unless the plugin went meanwhile.
  void AskCameraAccess(std::function<void()> then);
  // Whether getUserMedia with [constraints] should ask for the cameras first.
  bool CameraToAskFor(const EncodableMap& constraints);

  void initLoggerCallback(RTCLoggingSeverity severity);
  RTCLoggingSeverity str2LogSeverity(std::string str);

  std::shared_ptr<AnswerGate> answer_gate_ = std::make_shared<AnswerGate>();
};

}  // namespace flutter_webrtc_plugin

#endif  // PLUGINS_FLUTTER_WEBRTC_HXX
