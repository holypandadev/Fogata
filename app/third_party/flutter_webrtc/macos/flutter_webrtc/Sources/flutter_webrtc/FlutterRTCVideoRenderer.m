#import "FlutterRTCVideoRenderer.h"

#import <AVFoundation/AVFoundation.h>
#import <CoreGraphics/CGImage.h>
#import <WebRTC/RTCYUVHelper.h>
#import <WebRTC/RTCYUVPlanarBuffer.h>
#import <WebRTC/WebRTC.h>

#import <objc/runtime.h>

#import "FlutterWebRTCPlugin.h"
#import "FogataFrameConvert.h"
#import <os/lock.h>

// The largest picture drawn from a video track: 8K (7680 by 4320 pixels, the largest
// screen shared as captured), and no side longer than 8192. Each picture gets a buffer of
// width * height * 4 bytes, and a peer can send frames up to the codec's 16383 by 16383,
// a gigabyte each; larger frames are dropped, and the first of each track is told
// (event "fogataFrameTooLarge", with its width and height).
static BOOL FrameSizeAllowed(CGFloat width, CGFloat height) {
  return width > 0 && height > 0 && width <= 8192 && height <= 8192 &&
         width * height <= 7680.0 * 4320.0;
}

@implementation FlutterRTCVideoRenderer {
  CGSize _renderSize;
  // Fogata: the newest frame, kept as it came, converted only when Flutter asks for it
  // (copyPixelBuffer, on its raster thread), and once: frames Flutter does not draw (a
  // tile not on screen, a window hidden, frames faster than the display) cost nothing.
  // Holding it holds one buffer of the decoder or the capture per renderer.
  RTCVideoFrame* _frame;
  // The picture of the newest frame converted, which copyPixelBuffer hands to Flutter.
  CVPixelBufferRef _pixelBufferRef;
  bool _disposed;
  // Used under _convertLock alone (copyPixelBuffer, and dispose to let them go): buffers
  // of the size of the frames drawn, a new one for each frame converted (a buffer Flutter
  // may still draw from is never written again), and the chroma of an NV12 frame.
  CVPixelBufferPoolRef _pool;
  CGSize _poolSize;
  FogataScratch _scratch;
  os_unfair_lock _convertLock;
  RTCVideoRotation _rotation;
  FlutterEventChannel* _eventChannel;
  bool _isFirstFrameRendered;
  bool _toldTooLarge;
  bool _loggedTooLarge;
  os_unfair_lock _lock;
}

@synthesize textureId = _textureId;
@synthesize registry = _registry;
@synthesize eventSink = _eventSink;
@synthesize videoTrack = _videoTrack;

- (instancetype)initWithTextureRegistry:(id<FlutterTextureRegistry>)registry
                              messenger:(NSObject<FlutterBinaryMessenger>*)messenger {
  self = [super init];
  if (self) {
    _lock = OS_UNFAIR_LOCK_INIT;
    _convertLock = OS_UNFAIR_LOCK_INIT;
    _isFirstFrameRendered = false;
    _renderSize = CGSizeZero;
    _poolSize = CGSizeZero;
    _registry = registry;
    _pixelBufferRef = nil;
    _eventSink = nil;
    _rotation = -1;
    _textureId = [registry registerTexture:self];
    /*Create Event Channel.*/
    _eventChannel = [FlutterEventChannel
        eventChannelWithName:[NSString stringWithFormat:@"FlutterWebRTC/Texture%lld", _textureId]
             binaryMessenger:messenger];
    [_eventChannel setStreamHandler:self];
  }
  return self;
}

// Called by Flutter on its raster thread when it draws the texture, after
// textureFrameAvailable, and again whenever it needs the picture (it may draw it more
// than once, or never). Answers a buffer retained for Flutter, or nil before any frame.
- (CVPixelBufferRef)copyPixelBuffer {
  os_unfair_lock_lock(&_convertLock);
  os_unfair_lock_lock(&_lock);
  RTCVideoFrame* frame = _frame;
  _frame = nil;
  CVPixelBufferRef shown = _disposed || _pixelBufferRef == nil ? nil
                                                               : CVBufferRetain(_pixelBufferRef);
  const bool disposed = _disposed;
  os_unfair_lock_unlock(&_lock);
  if (frame == nil || disposed) {
    os_unfair_lock_unlock(&_convertLock);
    return shown;
  }
  // The newest frame, converted outside _lock: frames go on coming meanwhile, and the
  // next one waits as the newest.
  const CGSize size = FogataFrameDrawnSize(frame);
  if (_pool == NULL || !CGSizeEqualToSize(size, _poolSize)) {
    if (_pool) CVPixelBufferPoolRelease(_pool);
    _pool = NULL;
    _poolSize = size;
    @autoreleasepool {
      NSDictionary* pixelAttributes = @{
        (id)kCVPixelBufferIOSurfacePropertiesKey : @{},
        (id)kCVPixelBufferPixelFormatTypeKey : @(kCVPixelFormatType_32BGRA),
        (id)kCVPixelBufferWidthKey : @((int)size.width),
        (id)kCVPixelBufferHeightKey : @((int)size.height),
      };
      CVPixelBufferPoolCreate(kCFAllocatorDefault, NULL,
                              (__bridge CFDictionaryRef)(pixelAttributes), &_pool);
    }
  }
  CVPixelBufferRef buffer = NULL;
  BOOL converted = NO;
  // What a conversion autoreleases (toI420's copy, for a frame not read in place) goes
  // as soon as it is done, and the frame with it, not when the raster thread's task ends.
  @autoreleasepool {
    converted = _pool != NULL &&
                CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, _pool, &buffer) ==
                    kCVReturnSuccess &&
                buffer != NULL && FogataFrameToBGRA(frame, buffer, &_scratch);
    frame = nil;
  }
  if (!converted) {
    if (buffer) CVBufferRelease(buffer);
    os_unfair_lock_unlock(&_convertLock);
    return shown;
  }
  os_unfair_lock_lock(&_lock);
  CVPixelBufferRef older = nil;
  if (!_disposed) {
    older = _pixelBufferRef;
    _pixelBufferRef = CVBufferRetain(buffer);
  }
  os_unfair_lock_unlock(&_lock);
  os_unfair_lock_unlock(&_convertLock);
  if (older) CVBufferRelease(older);
  if (shown) CVBufferRelease(shown);
  return buffer;
}

- (void)dealloc {
  if (_pixelBufferRef) CVBufferRelease(_pixelBufferRef);
  if (_pool) CVPixelBufferPoolRelease(_pool);
  FogataScratchFree(&_scratch);
}

- (void)dispose {
  [_eventChannel setStreamHandler:nil];
  _eventChannel = nil;
  CVPixelBufferRef picture = nil;
  {
    // Let go of at the end of this block, after the lock: a frame may hold the last
    // reference to a decoder's or a capture's buffer.
    __unused NS_VALID_UNTIL_END_OF_SCOPE RTCVideoFrame* frame;
    os_unfair_lock_lock(&_lock);
    [_registry unregisterTexture:_textureId];
    _textureId = -1;
    _disposed = true;
    picture = _pixelBufferRef;
    _pixelBufferRef = nil;
    frame = _frame;
    _frame = nil;
    os_unfair_lock_unlock(&_lock);
  }
  if (picture) CVBufferRelease(picture);
  // The pool's free buffers too; a conversion under way ends first.
  os_unfair_lock_lock(&_convertLock);
  if (_pool) {
    CVPixelBufferPoolRelease(_pool);
    _pool = NULL;
  }
  FogataScratchFree(&_scratch);
  os_unfair_lock_unlock(&_convertLock);
}

- (void)setVideoTrack:(RTCVideoTrack*)videoTrack {
  RTCVideoTrack* oldValue = self.videoTrack;
  if (oldValue != videoTrack) {
    os_unfair_lock_lock(&_lock);
    _videoTrack = videoTrack;
    _toldTooLarge = false;
    os_unfair_lock_unlock(&_lock);
    _isFirstFrameRendered = false;
    if (oldValue) {
      [oldValue removeRenderer:self];
    }
    _renderSize = CGSizeZero;
    _rotation = -1;
    if (videoTrack) {
      [videoTrack addRenderer:self];
    }
  }
}

#pragma mark - RTCVideoRenderer methods
- (void)renderFrame:(RTCVideoFrame*)frame {
  int width = (int)frame.width;
  int height = (int)frame.height;
  // A picture Fogata's WebRTC did not decode, being too large, comes as a small stand-in
  // whose buffer tells its size (refusedSize, read by name: WebRTC-SDK's own builds lack it).
  if ([(NSObject*)frame.buffer respondsToSelector:NSSelectorFromString(@"refusedSize")]) {
    NSValue* refused = [(NSObject*)frame.buffer valueForKey:@"refusedSize"];
#if TARGET_OS_OSX
    CGSize size = NSSizeToCGSize(refused.sizeValue);
#else
    CGSize size = refused.CGSizeValue;
#endif
    if (size.width > 0 && size.height > 0) {
      width = (int)size.width;
      height = (int)size.height;
    }
  }
  if (!FrameSizeAllowed(width, height)) {
    os_unfair_lock_lock(&_lock);
    BOOL tell = _videoTrack != nil && !_toldTooLarge;
    BOOL first = !_loggedTooLarge;
    _toldTooLarge = true;
    _loggedTooLarge = true;
    os_unfair_lock_unlock(&_lock);
    if (first) {
      NSLog(@"Dropping video frames of %dx%d: too large to draw", width, height);
    }
    if (tell) {
      // Fogata: the application keeps out of a call whoever sends such pictures.
      __weak FlutterRTCVideoRenderer* weakSelf = self;
      dispatch_async(dispatch_get_main_queue(), ^{
        FlutterRTCVideoRenderer* strongSelf = weakSelf;
        if (strongSelf.eventSink) {
          strongSelf.eventSink(@{
            @"event" : @"fogataFrameTooLarge",
            @"id" : @(strongSelf.textureId),
            @"width" : @(width),
            @"height" : @(height),
          });
        }
      });
    }
    return;
  }

  // Fogata: only the newest frame is kept, in place of the one before, drawn or not;
  // copyPixelBuffer converts it if Flutter draws it. Its size, rotation and first frame
  // are still told below as each frame comes: the application watches them for every
  // picture, drawn or not.
  {
    // Let go of at the end of this block, after the lock: it may hold the last reference
    // to a decoder's or a capture's buffer.
    __unused NS_VALID_UNTIL_END_OF_SCOPE RTCVideoFrame* older;
    os_unfair_lock_lock(&_lock);
    if (_videoTrack == nil || _disposed) {
      os_unfair_lock_unlock(&_lock);
      return;
    }
    older = _frame;
    _frame = frame;
    if (_textureId != -1) {
      [_registry textureFrameAvailable:_textureId];
    }
    os_unfair_lock_unlock(&_lock);
  }

  __weak FlutterRTCVideoRenderer* weakSelf = self;
  if (_renderSize.width != frame.width || _renderSize.height != frame.height) {
    dispatch_async(dispatch_get_main_queue(), ^{
      FlutterRTCVideoRenderer* strongSelf = weakSelf;
      if (strongSelf.eventSink) {
        strongSelf.eventSink(@{
          @"event" : @"didTextureChangeVideoSize",
          @"id" : @(strongSelf.textureId),
          @"width" : @(frame.width),
          @"height" : @(frame.height),
        });
      }
    });
    _renderSize = CGSizeMake(frame.width, frame.height);
  }

  if (frame.rotation != _rotation) {
    dispatch_async(dispatch_get_main_queue(), ^{
      FlutterRTCVideoRenderer* strongSelf = weakSelf;
      if (strongSelf.eventSink) {
        strongSelf.eventSink(@{
          @"event" : @"didTextureChangeRotation",
          @"id" : @(strongSelf.textureId),
          @"rotation" : @(frame.rotation),
        });
      }
    });

    _rotation = frame.rotation;
  }

  // Notify Flutter once the first frame has been rendered. Frames arrive on a
  // decoder thread and this block runs later on the main queue, so the
  // renderer may already have been disposed and freed by the time it runs.
  // The weak reference is then nil and must not be dereferenced.
  if (!_isFirstFrameRendered) {
    dispatch_async(dispatch_get_main_queue(), ^{
      FlutterRTCVideoRenderer* strongSelf = weakSelf;
      if (strongSelf == nil || strongSelf->_isFirstFrameRendered) {
        return;
      }
      if (strongSelf.eventSink) {
        strongSelf.eventSink(@{@"event" : @"didFirstFrameRendered"});
        strongSelf->_isFirstFrameRendered = true;
      }
    });
  }
}

/**
 * Sets the size of the video frame to render.
 *
 * Fogata: nothing to do here. copyPixelBuffer makes its buffers of the size of the frame
 * it converts; renderFrame: keeps no frame too large to draw.
 *
 * @param size The size of the video frame to render.
 */
- (void)setSize:(CGSize)size {
}

#pragma mark - FlutterStreamHandler methods

- (FlutterError* _Nullable)onCancelWithArguments:(id _Nullable)arguments {
  _eventSink = nil;
  return nil;
}

- (FlutterError* _Nullable)onListenWithArguments:(id _Nullable)arguments
                                       eventSink:(nonnull FlutterEventSink)sink {
  _eventSink = sink;
  return nil;
}
@end

@implementation FlutterWebRTCPlugin (FlutterVideoRendererManager)

- (FlutterRTCVideoRenderer*)createWithTextureRegistry:(id<FlutterTextureRegistry>)registry
                                            messenger:(NSObject<FlutterBinaryMessenger>*)messenger {
  return [[FlutterRTCVideoRenderer alloc] initWithTextureRegistry:registry messenger:messenger];
}

- (void)rendererSetSrcObject:(FlutterRTCVideoRenderer*)renderer stream:(RTCVideoTrack*)videoTrack {
  renderer.videoTrack = videoTrack;
}
@end
