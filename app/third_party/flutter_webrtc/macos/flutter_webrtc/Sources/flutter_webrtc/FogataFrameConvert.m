#import "FogataFrameConvert.h"

#import <WebRTC/WebRTC.h>

void FogataScratchFree(FogataScratch* scratch) {
  free(scratch->data);
  scratch->data = NULL;
  scratch->size = 0;
}

CGSize FogataFrameDrawnSize(RTCVideoFrame* frame) {
  if (frame.rotation == RTCVideoRotation_90 || frame.rotation == RTCVideoRotation_270) {
    return CGSizeMake(frame.height, frame.width);
  }
  return CGSizeMake(frame.width, frame.height);
}

// A 32BGRA buffer of the frame's drawn size, locked for writing: its first byte and row.
static uint8_t* LockDestination(RTCVideoFrame* frame, CVPixelBufferRef dst, int* stride) {
  const CGSize size = FogataFrameDrawnSize(frame);
  if (CVPixelBufferGetPixelFormatType(dst) != kCVPixelFormatType_32BGRA ||
      CVPixelBufferGetWidth(dst) != (size_t)size.width ||
      CVPixelBufferGetHeight(dst) != (size_t)size.height ||
      CVPixelBufferLockBaseAddress(dst, 0) != kCVReturnSuccess) {
    return NULL;
  }
  *stride = (int)CVPixelBufferGetBytesPerRow(dst);
  return CVPixelBufferGetBaseAddress(dst);
}

static id<RTCI420Buffer> Rotated(id<RTCI420Buffer> src, RTCVideoRotation rotation) {
  int width = src.width;
  int height = src.height;
  if (rotation == RTCVideoRotation_90 || rotation == RTCVideoRotation_270) {
    width = src.height;
    height = src.width;
  }
  id<RTCI420Buffer> buffer = [[RTCI420Buffer alloc] initWithWidth:width height:height];
  [RTCYUVHelper I420Rotate:src.dataY
                srcStrideY:src.strideY
                      srcU:src.dataU
                srcStrideU:src.strideU
                      srcV:src.dataV
                srcStrideV:src.strideV
                      dstY:(uint8_t*)buffer.dataY
                dstStrideY:buffer.strideY
                      dstU:(uint8_t*)buffer.dataU
                dstStrideU:buffer.strideU
                      dstV:(uint8_t*)buffer.dataV
                dstStrideV:buffer.strideV
                     width:src.width
                    height:src.height
                      mode:rotation];
  return buffer;
}

BOOL FogataFrameToBGRAThroughI420(RTCVideoFrame* frame, CVPixelBufferRef dst) {
  int dstStride = 0;
  uint8_t* out = LockDestination(frame, dst, &dstStride);
  if (out == NULL) return NO;
  // An I420 buffer (every frame decoded by libvpx) is its own toI420; any other is copied.
  id<RTCI420Buffer> i420 = [frame.buffer toI420];
  // Turned only when the frame is turned: turning by 0 is a full copy (libyuv's
  // I420Rotate copies the planes then), the same pixels at the same size.
  if (frame.rotation != RTCVideoRotation_0) i420 = Rotated(i420, frame.rotation);
  [RTCYUVHelper I420ToARGB:i420.dataY
                srcStrideY:i420.strideY
                      srcU:i420.dataU
                srcStrideU:i420.strideU
                      srcV:i420.dataV
                srcStrideV:i420.strideV
                   dstARGB:out
             dstStrideARGB:dstStride
                     width:i420.width
                    height:i420.height];
  CVPixelBufferUnlockBaseAddress(dst, 0);
  return YES;
}

// One row of NV12's chroma (U and V by turns) split into its U and V rows, 16 of each at a
// time (one shuffle of two 16-byte loads each, on arm64 as on x86_64). Written out: a
// release build compiles the plugin for size (-Os), where the plain loop is not
// vectorized, and a 1440p frame then took 0.2 ms more than through toI420.
typedef uint8_t FogataBytes16 __attribute__((vector_size(16)));
static void SplitRow(const uint8_t* in, uint8_t* u, uint8_t* v, size_t count) {
  size_t i = 0;
  for (; i + 16 <= count; i += 16) {
    FogataBytes16 low, high;
    memcpy(&low, in + 2 * i, 16);
    memcpy(&high, in + 2 * i + 16, 16);
    const FogataBytes16 us = __builtin_shufflevector(low, high, 0, 2, 4, 6, 8, 10, 12, 14,
                                                     16, 18, 20, 22, 24, 26, 28, 30);
    const FogataBytes16 vs = __builtin_shufflevector(low, high, 1, 3, 5, 7, 9, 11, 13, 15,
                                                     17, 19, 21, 23, 25, 27, 29, 31);
    memcpy(u + i, &us, 16);
    memcpy(v + i, &vs, 16);
  }
  for (; i < count; i++) {
    u[i] = in[2 * i];
    v[i] = in[2 * i + 1];
  }
}

// Reads an NV12 frame where it is. The planes are taken as WebRTC's own toI420 takes them
// (RTCCVPixelBuffer: the crop by moving the planes' starts, then libyuv's NV12ToI420,
// which keeps the luma and splits the chroma), so the same I420ToARGB gets the same bytes.
static BOOL NV12ToBGRA(RTCCVPixelBuffer* buffer, RTCVideoFrame* frame, CVPixelBufferRef dst,
                       FogataScratch* scratch) {
  CVPixelBufferRef src = buffer.pixelBuffer;
  const int width = buffer.width;
  const int height = buffer.height;
  const int cropX = buffer.cropX;
  const int cropY = buffer.cropY;
  const size_t halfWidth = ((size_t)width + 1) / 2;
  const size_t halfHeight = ((size_t)height + 1) / 2;
  if (width <= 0 || height <= 0 || CVPixelBufferGetPlaneCount(src) != 2 ||
      (size_t)cropX + width > CVPixelBufferGetWidthOfPlane(src, 0) ||
      (size_t)cropY + height > CVPixelBufferGetHeightOfPlane(src, 0) ||
      (size_t)cropX / 2 + halfWidth > CVPixelBufferGetWidthOfPlane(src, 1) ||
      (size_t)cropY / 2 + halfHeight > CVPixelBufferGetHeightOfPlane(src, 1)) {
    return FogataFrameToBGRAThroughI420(frame, dst);
  }
  const size_t chroma = halfWidth * halfHeight;
  if (scratch->size < chroma * 2) {
    uint8_t* grown = malloc(chroma * 2);
    if (grown == NULL) return NO;
    free(scratch->data);
    scratch->data = grown;
    scratch->size = chroma * 2;
  }
  int dstStride = 0;
  uint8_t* out = LockDestination(frame, dst, &dstStride);
  if (out == NULL) return NO;
  if (CVPixelBufferLockBaseAddress(src, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
    CVPixelBufferUnlockBaseAddress(dst, 0);
    return NO;
  }
  const size_t strideY = CVPixelBufferGetBytesPerRowOfPlane(src, 0);
  const size_t strideUV = CVPixelBufferGetBytesPerRowOfPlane(src, 1);
  const uint8_t* y = (const uint8_t*)CVPixelBufferGetBaseAddressOfPlane(src, 0) +
                     strideY * cropY + cropX;
  const uint8_t* uv = (const uint8_t*)CVPixelBufferGetBaseAddressOfPlane(src, 1) +
                      strideUV * (cropY / 2) + cropX;
  uint8_t* u = scratch->data;
  uint8_t* v = scratch->data + chroma;
  for (size_t row = 0; row < halfHeight; row++) {
    SplitRow(uv + row * strideUV, u + row * halfWidth, v + row * halfWidth, halfWidth);
  }
  [RTCYUVHelper I420ToARGB:y
                srcStrideY:(int)strideY
                      srcU:u
                srcStrideU:(int)halfWidth
                      srcV:v
                srcStrideV:(int)halfWidth
                   dstARGB:out
             dstStrideARGB:dstStride
                     width:width
                    height:height];
  CVPixelBufferUnlockBaseAddress(src, kCVPixelBufferLock_ReadOnly);
  CVPixelBufferUnlockBaseAddress(dst, 0);
  return YES;
}

BOOL FogataFrameToBGRA(RTCVideoFrame* frame, CVPixelBufferRef dst, FogataScratch* scratch) {
  if (frame.rotation == RTCVideoRotation_0 &&
      [(NSObject*)frame.buffer isKindOfClass:[RTCCVPixelBuffer class]]) {
    RTCCVPixelBuffer* buffer = (RTCCVPixelBuffer*)frame.buffer;
    const OSType format = CVPixelBufferGetPixelFormatType(buffer.pixelBuffer);
    // Cropped by an even number of pixels only, as toI420 moves the chroma's start by the
    // crop's x in bytes: two bytes, U and V, for every two pixels.
    if ((format == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange ||
         format == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) &&
        ![buffer requiresScalingToWidth:buffer.width height:buffer.height] &&
        buffer.cropX % 2 == 0 && buffer.cropY % 2 == 0) {
      return NV12ToBGRA(buffer, frame, dst, scratch);
    }
  }
  return FogataFrameToBGRAThroughI420(frame, dst);
}
