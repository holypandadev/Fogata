#import "FlutterRTCVideoRenderer.h"

#import <AVFoundation/AVFoundation.h>
#import <CoreGraphics/CGImage.h>
#import <WebRTC/RTCYUVHelper.h>
#import <WebRTC/RTCYUVPlanarBuffer.h>
#import <WebRTC/WebRTC.h>

#import <objc/runtime.h>

#import "FlutterWebRTCPlugin.h"
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
  CGSize _frameSize;
  CGSize _renderSize;
  // The newest frame converted, which copyPixelBuffer hands to Flutter.
  CVPixelBufferRef _pixelBufferRef;
  // Fogata: buffers of the current size, a new one for each frame (see renderFrame:).
  CVPixelBufferPoolRef _pool;
  RTCVideoRotation _rotation;
  FlutterEventChannel* _eventChannel;
  bool _isFirstFrameRendered;
  bool _frameAvailable;
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
    _isFirstFrameRendered = false;
    _frameAvailable = false;
    _frameSize = CGSizeZero;
    _renderSize = CGSizeZero;
    _rotation = -1;
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

- (CVPixelBufferRef)copyPixelBuffer {
  CVPixelBufferRef buffer = nil;
  os_unfair_lock_lock(&_lock);
  if (_pixelBufferRef != nil && _frameAvailable) {
    buffer = CVBufferRetain(_pixelBufferRef);
    _frameAvailable = false;
  }
  os_unfair_lock_unlock(&_lock);
  return buffer;
}

- (void)dealloc {
  if (_pixelBufferRef) CVBufferRelease(_pixelBufferRef);
  if (_pool) CVPixelBufferPoolRelease(_pool);
}

- (void)dispose {
  [_eventChannel setStreamHandler:nil];
  _eventChannel = nil;
  os_unfair_lock_lock(&_lock);
  [_registry unregisterTexture:_textureId];
  _textureId = -1;
  if (_pixelBufferRef) {
    CVBufferRelease(_pixelBufferRef);
    _pixelBufferRef = nil;
  }
  if (_pool) {
    CVPixelBufferPoolRelease(_pool);
    _pool = NULL;
  }
  _frameAvailable = false;
  os_unfair_lock_unlock(&_lock);
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
    _frameSize = CGSizeZero;
    _renderSize = CGSizeZero;
    _rotation = -1;
    if (videoTrack) {
      [videoTrack addRenderer:self];
    }
  }
}

- (id<RTCI420Buffer>)correctRotation:(const id<RTCI420Buffer>)src
                        withRotation:(RTCVideoRotation)rotation {
  int rotated_width = src.width;
  int rotated_height = src.height;

  if (rotation == RTCVideoRotation_90 || rotation == RTCVideoRotation_270) {
    int temp = rotated_width;
    rotated_width = rotated_height;
    rotated_height = temp;
  }

  id<RTCI420Buffer> buffer = [[RTCI420Buffer alloc] initWithWidth:rotated_width
                                                           height:rotated_height];

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

- (void)copyI420ToCVPixelBuffer:(CVPixelBufferRef)outputPixelBuffer
                      withFrame:(RTCVideoFrame*)frame {
  id<RTCI420Buffer> i420Buffer = [frame.buffer toI420];
  // Fogata: turned only when the frame is turned. Turning by 0 made a full copy of every
  // frame (libyuv's I420Rotate copies the planes then), the same pixels at the same size;
  // the buffer from toI420 is held here until the conversion below is done.
  if (frame.rotation != RTCVideoRotation_0) {
    i420Buffer = [self correctRotation:i420Buffer withRotation:frame.rotation];
  }
  CVPixelBufferLockBaseAddress(outputPixelBuffer, 0);

  const OSType pixelFormat = CVPixelBufferGetPixelFormatType(outputPixelBuffer);
  if (pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange ||
      pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) {
    // NV12
    uint8_t* dstY = CVPixelBufferGetBaseAddressOfPlane(outputPixelBuffer, 0);
    const size_t dstYStride = CVPixelBufferGetBytesPerRowOfPlane(outputPixelBuffer, 0);
    uint8_t* dstUV = CVPixelBufferGetBaseAddressOfPlane(outputPixelBuffer, 1);
    const size_t dstUVStride = CVPixelBufferGetBytesPerRowOfPlane(outputPixelBuffer, 1);

    [RTCYUVHelper I420ToNV12:i420Buffer.dataY
                  srcStrideY:i420Buffer.strideY
                        srcU:i420Buffer.dataU
                  srcStrideU:i420Buffer.strideU
                        srcV:i420Buffer.dataV
                  srcStrideV:i420Buffer.strideV
                        dstY:dstY
                  dstStrideY:(int)dstYStride
                       dstUV:dstUV
                 dstStrideUV:(int)dstUVStride
                       width:i420Buffer.width
                      height:i420Buffer.height];

  } else {
    uint8_t* dst = CVPixelBufferGetBaseAddress(outputPixelBuffer);
    const size_t bytesPerRow = CVPixelBufferGetBytesPerRow(outputPixelBuffer);

    if (pixelFormat == kCVPixelFormatType_32BGRA) {
      // Corresponds to libyuv::FOURCC_ARGB

      [RTCYUVHelper I420ToARGB:i420Buffer.dataY
                    srcStrideY:i420Buffer.strideY
                          srcU:i420Buffer.dataU
                    srcStrideU:i420Buffer.strideU
                          srcV:i420Buffer.dataV
                    srcStrideV:i420Buffer.strideV
                       dstARGB:dst
                 dstStrideARGB:(int)bytesPerRow
                         width:i420Buffer.width
                        height:i420Buffer.height];

    } else if (pixelFormat == kCVPixelFormatType_32ARGB) {
      // Corresponds to libyuv::FOURCC_BGRA
      [RTCYUVHelper I420ToBGRA:i420Buffer.dataY
                    srcStrideY:i420Buffer.strideY
                          srcU:i420Buffer.dataU
                    srcStrideU:i420Buffer.strideU
                          srcV:i420Buffer.dataV
                    srcStrideV:i420Buffer.strideV
                       dstBGRA:dst
                 dstStrideBGRA:(int)bytesPerRow
                         width:i420Buffer.width
                        height:i420Buffer.height];
    }
  }

  CVPixelBufferUnlockBaseAddress(outputPixelBuffer, 0);
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

  // Fogata: every frame goes into a buffer of its own, converted outside the lock, and
  // takes the place of the one before even when Flutter has not drawn that yet. Frames
  // were written into one buffer, and one that came before Flutter took the last was
  // dropped: whenever Flutter drew late, the newest frame was lost and an older one
  // stayed on screen. A buffer Flutter may still draw from is never written again: the
  // pool gives another.
  os_unfair_lock_lock(&_lock);
  if(_videoTrack == nil || _pool == NULL) {
    os_unfair_lock_unlock(&_lock);
    return;
  }
  CVPixelBufferPoolRef pool = CVPixelBufferPoolRetain(_pool);
  os_unfair_lock_unlock(&_lock);
  CVPixelBufferRef buffer = NULL;
  if (CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &buffer) ==
          kCVReturnSuccess &&
      buffer != NULL) {
    [self copyI420ToCVPixelBuffer:buffer withFrame:frame];
    os_unfair_lock_lock(&_lock);
    // Kept only while its size is still the one asked (and the renderer not disposed).
    if (_pool == pool) {
      CVPixelBufferRef older = _pixelBufferRef;
      _pixelBufferRef = buffer;
      buffer = older;
      if(_textureId != -1) {
        [_registry textureFrameAvailable:_textureId];
      }
      _frameAvailable = true;
    }
    os_unfair_lock_unlock(&_lock);
    if (buffer != NULL) CVBufferRelease(buffer);
  }
  CVPixelBufferPoolRelease(pool);

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
 * @param size The size of the video frame to render.
 */
- (void)setSize:(CGSize)size {
  os_unfair_lock_lock(&_lock);
  if (size.width != _frameSize.width || size.height != _frameSize.height) {
    if (_pixelBufferRef) {
      CVBufferRelease(_pixelBufferRef);
      _pixelBufferRef = nil;
    }
    if (_pool) {
      CVPixelBufferPoolRelease(_pool);
      _pool = NULL;
    }
    _frameAvailable = false;
    _frameSize = size;
    if (!FrameSizeAllowed(size.width, size.height)) {
      os_unfair_lock_unlock(&_lock);
      return;
    }
    NSDictionary* pixelAttributes = @{
      (id)kCVPixelBufferIOSurfacePropertiesKey : @{},
      (id)kCVPixelBufferPixelFormatTypeKey : @(kCVPixelFormatType_32BGRA),
      (id)kCVPixelBufferWidthKey : @((int)size.width),
      (id)kCVPixelBufferHeightKey : @((int)size.height),
    };
    CVPixelBufferPoolCreate(kCFAllocatorDefault, NULL,
                            (__bridge CFDictionaryRef)(pixelAttributes), &_pool);
  }
  os_unfair_lock_unlock(&_lock);
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
