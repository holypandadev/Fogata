// Tests of FogataFrameConvert, without Flutter: every kind of frame the renderer gets,
// converted the way it was before (through I420) and the way it is now, must give the same
// picture to the bit; then how long each takes, and what it allocates, per frame.
// Built and run by run_tests.sh, against the WebRTC framework the app is built with.
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

#include <mach/mach_time.h>
#include <stdatomic.h>

#import "FogataFrameConvert.h"

// libmalloc's hook (what Instruments' allocation tracking uses): every allocation and its
// size, the frameworks' too.
typedef void(malloc_logger_t)(uint32_t type, uintptr_t arg1, uintptr_t arg2, uintptr_t arg3,
                              uintptr_t result, uint32_t skip);
extern malloc_logger_t* malloc_logger;
enum { kLogAllocate = 2, kLogDeallocate = 4 };
static _Atomic uint64_t allocated_bytes;
static _Atomic uint64_t allocations;
static void count_allocations(uint32_t type, uintptr_t arg1, uintptr_t arg2, uintptr_t arg3,
                              uintptr_t result, uint32_t skip) {
  if (!(type & kLogAllocate)) return;
  atomic_fetch_add(&allocated_bytes, (type & kLogDeallocate) ? arg3 : arg2);
  atomic_fetch_add(&allocations, 1);
}

static int failed = 0;

static uint32_t seed = 1;
static uint8_t noise(void) {
  seed = seed * 1664525u + 1013904223u;
  return (uint8_t)(seed >> 24);
}

// A picture with gradients, edges and noise, so that every rounding of the conversion
// shows somewhere.
static uint8_t sample(int x, int y, int plane) {
  int v = plane == 0 ? (x * 3 + y * 2) : plane == 1 ? (x * 5 - y) : (y * 4 + x);
  if (((x / 16) + (y / 16)) % 7 == 0) v = plane == 0 ? 235 : 16;
  return (uint8_t)((v & 0xff) ^ (noise() & 0x0f));
}

static RTCVideoFrame* I420Frame(int width, int height, RTCVideoRotation rotation) {
  RTCMutableI420Buffer* buffer = [[RTCMutableI420Buffer alloc] initWithWidth:width
                                                                      height:height];
  for (int y = 0; y < height; y++)
    for (int x = 0; x < width; x++) buffer.mutableDataY[y * buffer.strideY + x] = sample(x, y, 0);
  for (int y = 0; y < buffer.chromaHeight; y++)
    for (int x = 0; x < buffer.chromaWidth; x++) {
      buffer.mutableDataU[y * buffer.strideU + x] = sample(x, y, 1);
      buffer.mutableDataV[y * buffer.strideV + x] = sample(x, y, 2);
    }
  return [[RTCVideoFrame alloc] initWithBuffer:buffer rotation:rotation timeStampNs:0];
}

// An NV12 buffer in IOSurface memory, as a camera or ScreenCaptureKit gives.
static CVPixelBufferRef NV12Buffer(int width, int height, OSType format) {
  CVPixelBufferRef pixels = NULL;
  NSDictionary* attributes = @{(id)kCVPixelBufferIOSurfacePropertiesKey : @{}};
  if (CVPixelBufferCreate(kCFAllocatorDefault, width, height, format,
                          (__bridge CFDictionaryRef)attributes, &pixels) != kCVReturnSuccess) {
    return NULL;
  }
  CVPixelBufferLockBaseAddress(pixels, 0);
  uint8_t* luma = CVPixelBufferGetBaseAddressOfPlane(pixels, 0);
  const size_t lumaStride = CVPixelBufferGetBytesPerRowOfPlane(pixels, 0);
  for (int y = 0; y < height; y++)
    for (int x = 0; x < width; x++) luma[y * lumaStride + x] = sample(x, y, 0);
  uint8_t* chroma = CVPixelBufferGetBaseAddressOfPlane(pixels, 1);
  const size_t chromaStride = CVPixelBufferGetBytesPerRowOfPlane(pixels, 1);
  for (size_t y = 0; y < CVPixelBufferGetHeightOfPlane(pixels, 1); y++)
    for (size_t x = 0; x < CVPixelBufferGetWidthOfPlane(pixels, 1); x++) {
      chroma[y * chromaStride + 2 * x] = sample((int)x, (int)y, 1);
      chroma[y * chromaStride + 2 * x + 1] = sample((int)x, (int)y, 2);
    }
  CVPixelBufferUnlockBaseAddress(pixels, 0);
  return pixels;
}

static RTCVideoFrame* NV12Frame(int width, int height, OSType format, RTCVideoRotation rotation) {
  CVPixelBufferRef pixels = NV12Buffer(width, height, format);
  RTCCVPixelBuffer* buffer = [[RTCCVPixelBuffer alloc] initWithPixelBuffer:pixels];
  CVBufferRelease(pixels);
  return [[RTCVideoFrame alloc] initWithBuffer:buffer rotation:rotation timeStampNs:0];
}

// Part of a larger NV12 buffer, as WebRTC's source gives one it adapted: `adapted` the
// size the frame says, `crop` the part of the buffer taken.
static RTCVideoFrame* CroppedNV12Frame(int bufferWidth, int bufferHeight, int adaptedWidth,
                                       int adaptedHeight, int cropX, int cropY, int cropWidth,
                                       int cropHeight) {
  CVPixelBufferRef pixels =
      NV12Buffer(bufferWidth, bufferHeight, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);
  RTCCVPixelBuffer* buffer = [[RTCCVPixelBuffer alloc] initWithPixelBuffer:pixels
                                                               adaptedWidth:adaptedWidth
                                                              adaptedHeight:adaptedHeight
                                                                  cropWidth:cropWidth
                                                                 cropHeight:cropHeight
                                                                      cropX:cropX
                                                                      cropY:cropY];
  CVBufferRelease(pixels);
  return [[RTCVideoFrame alloc] initWithBuffer:buffer
                                      rotation:RTCVideoRotation_0
                                   timeStampNs:0];
}

static CVPixelBufferPoolRef PoolFor(RTCVideoFrame* frame) {
  const CGSize size = FogataFrameDrawnSize(frame);
  NSDictionary* attributes = @{
    (id)kCVPixelBufferIOSurfacePropertiesKey : @{},
    (id)kCVPixelBufferPixelFormatTypeKey : @(kCVPixelFormatType_32BGRA),
    (id)kCVPixelBufferWidthKey : @((int)size.width),
    (id)kCVPixelBufferHeightKey : @((int)size.height),
  };
  CVPixelBufferPoolRef pool = NULL;
  CVPixelBufferPoolCreate(kCFAllocatorDefault, NULL, (__bridge CFDictionaryRef)attributes,
                          &pool);
  return pool;
}

static CVPixelBufferRef NewPicture(CVPixelBufferPoolRef pool) {
  CVPixelBufferRef picture = NULL;
  CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &picture);
  return picture;
}

// The same picture both ways, every byte of every row.
static void check_same(const char* name, RTCVideoFrame* frame) {
  CVPixelBufferPoolRef pool = PoolFor(frame);
  CVPixelBufferRef before = NewPicture(pool);
  CVPixelBufferRef now = NewPicture(pool);
  FogataScratch scratch = {0};
  if (!FogataFrameToBGRAThroughI420(frame, before) || !FogataFrameToBGRA(frame, now, &scratch)) {
    printf("FAIL %s: not converted\n", name);
    failed = 1;
  } else {
    CVPixelBufferLockBaseAddress(before, kCVPixelBufferLock_ReadOnly);
    CVPixelBufferLockBaseAddress(now, kCVPixelBufferLock_ReadOnly);
    const size_t width = CVPixelBufferGetWidth(before);
    const size_t height = CVPixelBufferGetHeight(before);
    size_t differ = 0;
    int largest = 0;
    for (size_t y = 0; y < height; y++) {
      const uint8_t* a = (const uint8_t*)CVPixelBufferGetBaseAddress(before) +
                         y * CVPixelBufferGetBytesPerRow(before);
      const uint8_t* b = (const uint8_t*)CVPixelBufferGetBaseAddress(now) +
                         y * CVPixelBufferGetBytesPerRow(now);
      for (size_t i = 0; i < width * 4; i++) {
        if (a[i] != b[i]) {
          differ++;
          largest = MAX(largest, abs((int)a[i] - (int)b[i]));
        }
      }
    }
    CVPixelBufferUnlockBaseAddress(before, kCVPixelBufferLock_ReadOnly);
    CVPixelBufferUnlockBaseAddress(now, kCVPixelBufferLock_ReadOnly);
    if (differ != 0) {
      printf("FAIL %s: %zu bytes differ, by up to %d\n", name, differ, largest);
      failed = 1;
    } else {
      printf("same picture: %s (%zux%zu)\n", name, width, height);
    }
  }
  FogataScratchFree(&scratch);
  CVBufferRelease(before);
  CVBufferRelease(now);
  CVPixelBufferPoolRelease(pool);
}

static double ms_since(uint64_t start) {
  static mach_timebase_info_data_t timebase;
  if (timebase.denom == 0) mach_timebase_info(&timebase);
  return (double)(mach_absolute_time() - start) * timebase.numer / timebase.denom / 1e6;
}

static int by_value(const void* a, const void* b) {
  const double x = *(const double*)a, y = *(const double*)b;
  return x < y ? -1 : x > y;
}

// Time and allocations per frame, a new picture from the pool for each frame as the
// renderer takes; the median of `count` frames.
static void measure(const char* name, RTCVideoFrame* frame, BOOL now, int count) {
  CVPixelBufferPoolRef pool = PoolFor(frame);
  FogataScratch scratch = {0};
  double* times = calloc(count, sizeof(double));
  // A first frame before measuring: the pool's buffers and the scratch made.
  for (int i = 0; i < 3; i++) {
    CVPixelBufferRef picture = NewPicture(pool);
    now ? FogataFrameToBGRA(frame, picture, &scratch)
        : FogataFrameToBGRAThroughI420(frame, picture);
    CVBufferRelease(picture);
  }
  atomic_store(&allocated_bytes, 0);
  atomic_store(&allocations, 0);
  malloc_logger = count_allocations;
  for (int i = 0; i < count; i++) {
    @autoreleasepool {
      const uint64_t start = mach_absolute_time();
      CVPixelBufferRef picture = NewPicture(pool);
      now ? FogataFrameToBGRA(frame, picture, &scratch)
          : FogataFrameToBGRAThroughI420(frame, picture);
      CVBufferRelease(picture);
      times[i] = ms_since(start);
    }
  }
  malloc_logger = NULL;
  qsort(times, count, sizeof(double), by_value);
  printf("%-32s %-6s %7.3f ms/frame (median of %d; fastest %.3f), %9.0f bytes in %5.1f "
         "allocations/frame\n",
         name, now ? "now" : "before", times[count / 2], count, times[0],
         (double)atomic_load(&allocated_bytes) / count, (double)atomic_load(&allocations) / count);
  free(times);
  FogataScratchFree(&scratch);
  CVPixelBufferPoolRelease(pool);
}

int main(int argc, char** argv) {
  @autoreleasepool {
    // The hook counts: one megabyte asked for is seen.
    atomic_store(&allocated_bytes, 0);
    malloc_logger = count_allocations;
    void* probe = malloc(1 << 20);
    __asm__ volatile("" : : "r"(probe) : "memory");  // kept: the pair is not optimized away
    malloc_logger = NULL;
    free(probe);
    const BOOL counting = atomic_load(&allocated_bytes) >= (1 << 20);
    if (!counting) printf("(allocations are not counted on this system)\n");

    const OSType video = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
    const OSType full = kCVPixelFormatType_420YpCbCr8BiPlanarFullRange;
    check_same("I420 1280x720", I420Frame(1280, 720, RTCVideoRotation_0));
    check_same("I420 641x361 turned 90", I420Frame(641, 361, RTCVideoRotation_90));
    check_same("NV12 video range 1280x720", NV12Frame(1280, 720, video, RTCVideoRotation_0));
    check_same("NV12 full range 2560x1440", NV12Frame(2560, 1440, full, RTCVideoRotation_0));
    check_same("NV12 odd 641x361", NV12Frame(641, 361, full, RTCVideoRotation_0));
    check_same("NV12 odd 1x1", NV12Frame(1, 1, video, RTCVideoRotation_0));
    check_same("NV12 turned 270", NV12Frame(1280, 720, video, RTCVideoRotation_270));
    check_same("NV12 cropped", CroppedNV12Frame(1280, 960, 1280, 720, 0, 120, 1280, 720));
    check_same("NV12 cropped at odd x",
               CroppedNV12Frame(1280, 960, 1270, 720, 3, 121, 1270, 720));
    check_same("NV12 cropped and scaled",
               CroppedNV12Frame(1280, 960, 640, 360, 0, 120, 1280, 720));
    if (failed) return 1;
    printf("frame_convert_test: all the same\n\n");

    if (argc > 1 && strcmp(argv[1], "--measure") == 0) {
      struct {
        const char* name;
        RTCVideoFrame* frame;
        int count;
      } cases[] = {
          {"I420 1280x720 (others' camera)", I420Frame(1280, 720, RTCVideoRotation_0), 300},
          {"I420 2560x1440 (others' screen)", I420Frame(2560, 1440, RTCVideoRotation_0), 150},
          {"NV12 1280x720 (own camera)", NV12Frame(1280, 720, video, RTCVideoRotation_0), 300},
          {"NV12 2560x1440 (own screen)", NV12Frame(2560, 1440, full, RTCVideoRotation_0), 150},
      };
      for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        measure(cases[i].name, cases[i].frame, NO, cases[i].count);
        measure(cases[i].name, cases[i].frame, YES, cases[i].count);
      }
    }
  }
  return 0;
}
