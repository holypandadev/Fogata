#ifndef FLUTTER_WEBRTC_TEST_CAMERA_H_
#define FLUTTER_WEBRTC_TEST_CAMERA_H_

#include <atomic>
#include <thread>

#include "rtc_video_device.h"
#include "rtc_video_source.h"

namespace flutter_webrtc_plugin {

using namespace libwebrtc;

// A made-up camera for tests on computers without one (FOGATA_TEST_CAMERA=1):
// a textured background that sways, a shape that moves over it and a little
// noise on every frame, at the size and rate asked for, so the encoder works
// about as it would on a real camera. With FOGATA_TEST_CAMERA_GIANT=<w>x<h> it
// also sends, once a second, one flat picture of that size in place of an
// ordinary one, as a modified copy could, for tests of what a receiver does
// with pictures larger than any copy sends.
class TestCamera : public RTCVideoCapturer {
 public:
  // Whether getUserMedia should use it when the computer has no camera.
  static bool Wanted();

  TestCamera(scoped_refptr<RTCVideoSource> source, int width, int height,
             int fps);
  ~TestCamera() override;

  bool StartCapture() override;
  bool CaptureStarted() override;
  void StopCapture() override;

 private:
  void Run();

  scoped_refptr<RTCVideoSource> source_;
  int width_;
  int height_;
  int fps_;
  // The size of the picture sent once a second besides the others; 0 for none.
  int giant_width_ = 0;
  int giant_height_ = 0;
  std::atomic<bool> running_{false};
  std::thread thread_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FLUTTER_WEBRTC_TEST_CAMERA_H_
