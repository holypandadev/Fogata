// The macOS video renderer without a Flutter engine: frames keep only the newest, are
// converted only when Flutter asks for the picture (copyPixelBuffer), once each, and the
// events of every frame still go out; then frames, pictures asked for and a dispose on
// three threads at once. Built with ThreadSanitizer by run_tests.sh, which fails on any data
// race it reports.
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

#include <mach/mach_time.h>
#include <pthread.h>
#include <stdatomic.h>

#import "FlutterRTCVideoRenderer.h"
#import "FogataFrameConvert.h"

// The renderer's file adds a category to the plugin's class, which is not built here.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wprotocol"
#pragma clang diagnostic ignored "-Wobjc-property-implementation"
#pragma clang diagnostic ignored "-Wincomplete-implementation"
@implementation FlutterWebRTCPlugin
@end
#pragma clang diagnostic pop

// What the engine's texture registry is told.
@interface Registry : NSObject <FlutterTextureRegistry>
@property(atomic) int available;
@property(atomic) int unregistered;
@end
@implementation Registry
- (int64_t)registerTexture:(NSObject<FlutterTexture>*)texture {
  return 7;
}
- (void)textureFrameAvailable:(int64_t)textureId {
  self.available++;
}
- (void)unregisterTexture:(int64_t)textureId {
  self.unregistered++;
}
@end

@interface Messenger : NSObject <FlutterBinaryMessenger>
@end
@implementation Messenger
- (void)sendOnChannel:(NSString*)channel message:(NSData*)message {
}
- (void)sendOnChannel:(NSString*)channel
              message:(NSData*)message
          binaryReply:(FlutterBinaryReply)callback {
}
- (FlutterBinaryMessengerConnection)setMessageHandlerOnChannel:(NSString*)channel
                                          binaryMessageHandler:
                                              (FlutterBinaryMessageHandler)handler {
  return 1;
}
- (void)cleanUpConnection:(FlutterBinaryMessengerConnection)connection {
}
@end

// Stands for a track: the renderer only adds itself to it and removes itself.
@interface Track : NSObject
@end
@implementation Track
- (void)addRenderer:(id)renderer {
}
- (void)removeRenderer:(id)renderer {
}
@end

static int failed = 0;
static void check(BOOL ok, const char* what) {
  if (!ok) {
    printf("FAIL: %s\n", what);
    failed = 1;
  }
}

// A frame of one size whose every luma sample is `luma` (chroma grey).
static RTCVideoFrame* Flat(int width, int height, uint8_t luma, RTCVideoRotation rotation) {
  RTCMutableI420Buffer* buffer = [[RTCMutableI420Buffer alloc] initWithWidth:width
                                                                      height:height];
  memset(buffer.mutableDataY, luma, (size_t)buffer.strideY * height);
  memset(buffer.mutableDataU, 128, (size_t)buffer.strideU * buffer.chromaHeight);
  memset(buffer.mutableDataV, 128, (size_t)buffer.strideV * buffer.chromaHeight);
  return [[RTCVideoFrame alloc] initWithBuffer:buffer rotation:rotation timeStampNs:0];
}

// The blue byte of the picture's first pixel (BGRA): for a flat grey frame, its brightness.
static int FirstBlue(CVPixelBufferRef picture) {
  CVPixelBufferLockBaseAddress(picture, kCVPixelBufferLock_ReadOnly);
  const int blue = ((const uint8_t*)CVPixelBufferGetBaseAddress(picture))[0];
  CVPixelBufferUnlockBaseAddress(picture, kCVPixelBufferLock_ReadOnly);
  return blue;
}

static int BlueOf(uint8_t luma) {
  RTCVideoFrame* frame = Flat(2, 2, luma, RTCVideoRotation_0);
  CVPixelBufferRef picture = NULL;
  CVPixelBufferCreate(kCFAllocatorDefault, 2, 2, kCVPixelFormatType_32BGRA, NULL, &picture);
  FogataFrameToBGRAThroughI420(frame, picture);
  const int blue = FirstBlue(picture);
  CVBufferRelease(picture);
  return blue;
}

// As WebRTC's adapter hands a frame to a renderer: its size, turned, first when it
// changed, then the frame.
static void Give(FlutterRTCVideoRenderer* renderer, RTCVideoFrame* frame) {
  static __thread CGSize last;
  const CGSize size = FogataFrameDrawnSize(frame);
  if (!CGSizeEqualToSize(size, last)) {
    last = size;
    [renderer setSize:size];
  }
  [renderer renderFrame:frame];
}

static NSMutableArray<NSDictionary*>* events;

static FlutterRTCVideoRenderer* NewRenderer(Registry* registry) {
  FlutterRTCVideoRenderer* renderer =
      [[FlutterRTCVideoRenderer alloc] initWithTextureRegistry:registry
                                                     messenger:[Messenger new]];
  renderer.eventSink = ^(id event) {
    [events addObject:event];
  };
  renderer.videoTrack = (RTCVideoTrack*)[Track new];
  return renderer;
}

// Runs what the renderer queued on the main queue (its events).
static void RunMain(void) {
  [[NSRunLoop mainRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
}

static int Count(NSString* name) {
  int count = 0;
  for (NSDictionary* event in events) count += [event[@"event"] isEqualToString:name];
  return count;
}

static double ms_since(uint64_t start) {
  static mach_timebase_info_data_t timebase;
  if (timebase.denom == 0) mach_timebase_info(&timebase);
  return (double)(mach_absolute_time() - start) * timebase.numer / timebase.denom / 1e6;
}

static void frames_without_drawing(void) {
  events = [NSMutableArray array];
  Registry* registry = [Registry new];
  FlutterRTCVideoRenderer* renderer = NewRenderer(registry);
  check([renderer copyPixelBuffer] == NULL, "no picture before a frame");
  NSMutableArray<RTCVideoFrame*>* frames = [NSMutableArray array];
  for (int i = 0; i < 20; i++) [frames addObject:Flat(2560, 1440, 16 + i * 3, RTCVideoRotation_0)];
  const uint64_t start = mach_absolute_time();
  for (RTCVideoFrame* frame in frames) Give(renderer, frame);
  // Only told: a conversion of such a frame takes more than half a millisecond.
  printf("a 2560x1440 frame given to the renderer, not drawn: %.4f ms\n",
         ms_since(start) / frames.count);
  // The newest frame changed after it came: what is drawn is read only when asked for.
  RTCMutableI420Buffer* newest = (RTCMutableI420Buffer*)frames.lastObject.buffer;
  memset(newest.mutableDataY, 200, (size_t)newest.strideY * newest.height);
  check(registry.available == 20, "Flutter told of every frame");
  RunMain();
  check(Count(@"didFirstFrameRendered") == 1, "the first frame told, undrawn");
  check(Count(@"didTextureChangeVideoSize") == 1, "the size told, undrawn");
  check(Count(@"didTextureChangeRotation") == 1, "the rotation told, undrawn");

  // Asked for: the newest frame, converted once however many times it is asked for.
  CVPixelBufferRef first = [renderer copyPixelBuffer];
  CVPixelBufferRef again = [renderer copyPixelBuffer];
  check(first != NULL && first == again, "the same picture again, not converted again");
  check(first != NULL && CVPixelBufferGetWidth(first) == 2560 &&
            CVPixelBufferGetHeight(first) == 1440,
        "the picture is of the frame's size");
  check(first != NULL && FirstBlue(first) == BlueOf(200),
        "the newest frame drawn, converted when asked for and not before");
  CVBufferRelease(first);
  CVBufferRelease(again);

  // A new size, then a frame turned: each picture of its own size.
  Give(renderer, Flat(640, 360, 100, RTCVideoRotation_0));
  CVPixelBufferRef smaller = [renderer copyPixelBuffer];
  check(smaller != NULL && CVPixelBufferGetWidth(smaller) == 640 &&
            FirstBlue(smaller) == BlueOf(100),
        "a frame of another size drawn");
  Give(renderer, Flat(640, 360, 120, RTCVideoRotation_90));
  CVPixelBufferRef turned = [renderer copyPixelBuffer];
  check(turned != NULL && CVPixelBufferGetWidth(turned) == 360 &&
            CVPixelBufferGetHeight(turned) == 640,
        "a frame turned drawn turned");
  // Too large: not kept, told; the picture before stays.
  Give(renderer, Flat(8200, 2, 50, RTCVideoRotation_0));
  CVPixelBufferRef kept = [renderer copyPixelBuffer];
  check(kept == turned, "the picture before stays when a frame too large comes");
  RunMain();
  check(Count(@"fogataFrameTooLarge") == 1, "a frame too large told");
  check(Count(@"didTextureChangeVideoSize") == 2, "the new size told");
  check(Count(@"didTextureChangeRotation") == 2, "the new rotation told");
  CVBufferRelease(smaller);
  CVBufferRelease(turned);
  if (kept) CVBufferRelease(kept);

  // Disposed while a frame waits: nothing drawn any more, nothing more told.
  Give(renderer, Flat(640, 360, 140, RTCVideoRotation_0));
  renderer.videoTrack = nil;
  [renderer dispose];
  check(registry.unregistered == 1, "the texture let go of");
  check([renderer copyPixelBuffer] == NULL, "no picture once disposed");
  const int told = registry.available;
  Give(renderer, Flat(640, 360, 160, RTCVideoRotation_0));
  check(registry.available == told, "frames after the dispose are not told");
}

// Frames of several sizes on one thread, the picture asked for on another, and a dispose
// from a third; every picture handed out must be a whole frame of its own size.
static FlutterRTCVideoRenderer* shared;
static _Atomic bool stop;
static _Atomic int pictures;
static _Atomic int wrong;
static RTCVideoFrame* __strong kinds[4];
static int blues[4];

static void* Frames(void* unused) {
  for (int i = 0; i < 3000 && !atomic_load(&stop); i++) {
    @autoreleasepool {
      Give(shared, kinds[i % 4]);
    }
  }
  return NULL;
}

static void* Raster(void* unused) {
  while (!atomic_load(&stop)) {
    @autoreleasepool {
      CVPixelBufferRef picture = [shared copyPixelBuffer];
      if (picture == NULL) continue;
      atomic_fetch_add(&pictures, 1);
      const size_t width = CVPixelBufferGetWidth(picture);
      const int kind = width == 96 ? 0 : width == 128 ? 1 : width == 160 ? 2 : width == 54 ? 3 : -1;
      if (kind < 0 || FirstBlue(picture) != blues[kind]) atomic_fetch_add(&wrong, 1);
      CVBufferRelease(picture);
    }
  }
  return NULL;
}

static void three_threads(void) {
  events = [NSMutableArray array];
  const int sizes[4][2] = {{96, 54}, {128, 72}, {160, 90}, {96, 54}};
  for (int k = 0; k < 4; k++) {
    kinds[k] = Flat(sizes[k][0], sizes[k][1], 30 + 50 * k,
                    k == 3 ? RTCVideoRotation_90 : RTCVideoRotation_0);
    blues[k] = BlueOf(30 + 50 * k);
  }
  shared = NewRenderer([Registry new]);
  pthread_t frames, raster;
  pthread_create(&frames, NULL, Frames, NULL);
  pthread_create(&raster, NULL, Raster, NULL);
  usleep(20000);
  [shared dispose];
  pthread_join(frames, NULL);
  atomic_store(&stop, true);
  pthread_join(raster, NULL);
  check(atomic_load(&wrong) == 0, "every picture a whole frame of its size");
  printf("three threads: %d pictures drawn, %d wrong\n", atomic_load(&pictures),
         atomic_load(&wrong));
  shared.videoTrack = nil;
  shared = nil;
  RunMain();
}

int main(void) {
  @autoreleasepool {
    frames_without_drawing();
    three_threads();
  }
  if (!failed) printf("video_renderer_test: all passed\n");
  return failed;
}
