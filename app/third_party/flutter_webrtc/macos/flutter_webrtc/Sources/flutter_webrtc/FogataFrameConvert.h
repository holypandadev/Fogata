// Fogata: a video frame turned into the BGRA picture Flutter draws (macOS).
//
// Frames come in two kinds of buffers. The others' cameras and screens are decoded by
// libvpx (VP8, the only codec of calls) into I420 buffers (RTCI420Buffer). This device's
// own camera and screen, shown in its own tiles, come as the capture gave them: a
// CVPixelBuffer in NV12 (RTCCVPixelBuffer), from the camera ('420v' or '420f') and from
// ScreenCaptureKit ('420f').
//
// An NV12 frame was first copied whole into a new I420 buffer (toI420), then converted.
// Its luma plane is now read where it is, and only its chroma, a third of the bytes, is
// split into U and V, in a buffer kept from one frame to the next. The conversion is
// still WebRTC's libyuv I420ToARGB (its only one the framework exports, through
// RTCYUVHelper; libyuv's NV12ToARGB is not exported), with the same planes, so the
// picture is the same to the bit; the colour range and matrix are those it always took
// (BT.601, video range, for every buffer). A frame that is turned, cropped or scaled, or
// in another format, goes the way it went before.
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <WebRTC/RTCVideoFrame.h>

NS_ASSUME_NONNULL_BEGIN

// A buffer for an NV12 frame's U and V planes, kept by its owner from frame to frame and
// grown as needed. Starts zeroed; `FogataScratchFree` lets it go.
typedef struct {
  uint8_t* _Nullable data;
  size_t size;
} FogataScratch;

void FogataScratchFree(FogataScratch* scratch);

// The size of `frame`'s picture once turned by its rotation.
CGSize FogataFrameDrawnSize(RTCVideoFrame* frame);

// Converts `frame` into `dst`, a 32BGRA pixel buffer of FogataFrameDrawnSize(frame).
// Answers NO, and writes nothing, for any other buffer.
BOOL FogataFrameToBGRA(RTCVideoFrame* frame, CVPixelBufferRef dst, FogataScratch* scratch);

// The way every frame went before: the frame as I420 (toI420: an I420 buffer itself, any
// other copied), turned when it is turned, then converted. Kept for frames
// FogataFrameToBGRA does not read in place, and for the tests, which compare both.
BOOL FogataFrameToBGRAThroughI420(RTCVideoFrame* frame, CVPixelBufferRef dst);

NS_ASSUME_NONNULL_END
