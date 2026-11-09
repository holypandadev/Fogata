#import "FlutterScreenCaptureKitCapturer.h"

#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreMedia/CoreMedia.h>

#import "FogataCaptureSize.h"

#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#endif

@interface FlutterScreenCaptureKitCapturer ()
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
<SCStreamOutput, SCStreamDelegate>
#endif
@property(nonatomic, strong) RTCVideoCapturer *capturer;
@property(nonatomic, weak) id<RTCVideoCapturerDelegate> delegate;
@property(nonatomic, strong) dispatch_queue_t captureQueue;
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
@property(nonatomic, strong) SCStream *stream API_AVAILABLE(macos(12.3));
// Fogata: the configuration a shared window is captured with, kept to follow its size.
@property(nonatomic, strong) SCStreamConfiguration *config API_AVAILABLE(macos(12.3));
#endif
// Fogata: a window is shared (not a display), and a new size for it is being set; both
// read on the capture queue.
@property(nonatomic, assign) BOOL sharesWindow;
@property(nonatomic, assign) BOOL resizing;
// Fogata: the capture started and was not stopped from here, and its end was reported;
// read and written under @synchronized(self), from SCK's queues and the caller's.
@property(nonatomic, assign) BOOL running;
@property(nonatomic, assign) BOOL ended;
@end

@implementation FlutterScreenCaptureKitCapturer

- (instancetype)initWithDelegate:(id<RTCVideoCapturerDelegate>)delegate {
  self = [super init];
  if (self) {
    _delegate = delegate;
    _capturer = [[RTCVideoCapturer alloc] initWithDelegate:delegate];
    _captureQueue = dispatch_queue_create("com.iperius.sck.capture", DISPATCH_QUEUE_SERIAL);
  }
  return self;
}

- (void)startCaptureWithFPS:(NSInteger)fps
                   sourceId:(NSString* _Nullable)sourceId
                  onStarted:(void (^)(NSError * _Nullable error))onStarted {
  [self startCaptureWithFPS:fps sourceId:sourceId window:NO onStarted:onStarted];
}

- (void)startCaptureWithFPS:(NSInteger)fps
                   sourceId:(NSString* _Nullable)sourceId
                     window:(BOOL)window
                  onStarted:(void (^)(NSError * _Nullable error))onStarted {
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
  if (@available(macOS 12.3, *)) {
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
      if (error != nil) {
        onStarted(error);
        return;
      }

      SCContentFilter *filter = nil;
      size_t width = 0;
      size_t height = 0;
      if (window) {
        // Fogata: one window, wherever it is and whatever covers it, in its own pixels.
        SCWindow *shared = nil;
        for (SCWindow *candidate in content.windows) {
          if ([[NSString stringWithFormat:@"%u", candidate.windowID] isEqualToString:sourceId]) {
            shared = candidate;
            break;
          }
        }
        if (shared == nil) {
          onStarted([NSError errorWithDomain:@"FlutterScreenCaptureKit"
                                        code:-1
                                    userInfo:@{NSLocalizedDescriptionKey: @"No matching window"}]);
          return;
        }
        filter = [[SCContentFilter alloc] initWithDesktopIndependentWindow:shared];
        CGFloat scale = [FlutterScreenCaptureKitCapturer scaleOfScreenUnder:shared.frame];
        if (@available(macOS 14.0, *)) {
          scale = filter.pointPixelScale;
        }
        // Fogata: at most what the others take, as when it changes size later.
        FogataCaptureSize(round(shared.frame.size.width * scale),
                          round(shared.frame.size.height * scale), &width, &height);
      } else {
        SCDisplay *display = [self selectDisplayFromContent:content sourceId:sourceId];
        if (display == nil) {
          NSError *noDisplay = [NSError errorWithDomain:@"FlutterScreenCaptureKit"
                                                   code:-1
                                               userInfo:@{NSLocalizedDescriptionKey: @"No matching display"}];
          onStarted(noDisplay);
          return;
        }
        // Fogata: the display without Fogata's own windows, those opened later too.
        // Captured with them, the call's tiles showed the share inside itself, again and
        // again, and every frame changed.
        NSMutableArray<SCRunningApplication *> *own = [NSMutableArray array];
        const pid_t pid = NSProcessInfo.processInfo.processIdentifier;
        for (SCRunningApplication *app in content.applications) {
          if (app.processID == pid) [own addObject:app];
        }
        filter = [[SCContentFilter alloc] initWithDisplay:display
                                    excludingApplications:own
                                         exceptingWindows:@[]];
        FogataCaptureSize(display.width, display.height, &width, &height);
      }

      SCStreamConfiguration *config = [SCStreamConfiguration new];
      config.width = width;
      config.height = height;
      config.minimumFrameInterval = CMTimeMake(1, (int32_t)MAX(1, fps));
      config.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarFullRange;
      if (@available(macOS 13.0, *)) {
        config.showsCursor = YES;
      }

      self.config = config;
      self.sharesWindow = window;
      self.resizing = NO;
      // Fogata: the stream's delegate hears when it stops by itself.
      self.stream = [[SCStream alloc] initWithFilter:filter configuration:config delegate:self];
      NSError *addOutputError = nil;
      [self.stream addStreamOutput:self
                              type:SCStreamOutputTypeScreen
               sampleHandlerQueue:self.captureQueue
                            error:&addOutputError];
      if (addOutputError != nil) {
        onStarted(addOutputError);
        return;
      }

      [self.stream startCaptureWithCompletionHandler:^(NSError * _Nullable startError) {
        if (startError == nil) {
          @synchronized(self) {
            self.running = YES;
          }
        }
        onStarted(startError);
      }];
    }];
    return;
  }
#endif

  NSError *unavailable = [NSError errorWithDomain:@"FlutterScreenCaptureKit"
                                             code:-2
                                         userInfo:@{NSLocalizedDescriptionKey: @"ScreenCaptureKit not available"}];
  onStarted(unavailable);
}

- (void)stopCaptureWithCompletion:(void (^)(void))completion {
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
  if (@available(macOS 12.3, *)) {
    @synchronized(self) {
      self.running = NO;
    }
    if (self.stream == nil) {
      completion();
      return;
    }
    SCStream *stream = self.stream;
    self.stream = nil;
    [stream stopCaptureWithCompletionHandler:^(__unused NSError * _Nullable error) {
      completion();
    }];
    return;
  }
#endif
  completion();
}

// Fogata: the scale of the screen that shows most of `frame` (in the window server's
// coordinates, from the top left of the main screen); the main screen's when none does.
+ (CGFloat)scaleOfScreenUnder:(CGRect)frame {
  NSScreen *main = NSScreen.screens.firstObject;
  if (main == nil) return NSScreen.mainScreen.backingScaleFactor;
  // AppKit counts from the bottom left of the main screen.
  const CGRect flipped = CGRectMake(frame.origin.x,
                                    NSMaxY(main.frame) - frame.origin.y - frame.size.height,
                                    frame.size.width, frame.size.height);
  NSScreen *best = main;
  CGFloat most = 0;
  for (NSScreen *screen in NSScreen.screens) {
    const CGRect shown = CGRectIntersection(flipped, screen.frame);
    const CGFloat area = CGRectIsNull(shown) ? 0 : shown.size.width * shown.size.height;
    if (area > most) {
      most = area;
      best = screen;
    }
  }
  return best.backingScaleFactor;
}

#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
// Fogata: a shared window that changes size, or moves to a screen of another scale, is
// captured at its new size in pixels instead of being fitted into the size it had at the
// start. Its size comes with each frame: the content's rectangle, how much it was scaled
// down to fit, and the screen's pixels per point.
- (void)followWindowSize:(CMSampleBufferRef)sampleBuffer API_AVAILABLE(macos(12.3)) {
  SCStream *stream = self.stream;
  SCStreamConfiguration *config = self.config;
  if (!self.sharesWindow || self.resizing || stream == nil || config == nil) return;
  CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
  if (attachments == NULL || CFArrayGetCount(attachments) == 0) return;
  NSDictionary *info = (__bridge NSDictionary *)CFArrayGetValueAtIndex(attachments, 0);
  NSDictionary *rectInfo = info[SCStreamFrameInfoContentRect];
  NSNumber *contentScale = info[SCStreamFrameInfoContentScale];
  NSNumber *scaleFactor = info[SCStreamFrameInfoScaleFactor];
  CGRect rect = CGRectZero;
  if (![rectInfo isKindOfClass:NSDictionary.class] ||
      ![contentScale isKindOfClass:NSNumber.class] ||
      ![scaleFactor isKindOfClass:NSNumber.class] || contentScale.doubleValue <= 0 ||
      scaleFactor.doubleValue <= 0 ||
      !CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)rectInfo, &rect)) {
    return;
  }
  const double pixels = scaleFactor.doubleValue / contentScale.doubleValue;
  // Even sizes, as the encoders take them; no larger than 8K, in its sides or in all.
  size_t width = 0;
  size_t height = 0;
  FogataCaptureSize(round(rect.size.width * pixels), round(rect.size.height * pixels), &width,
                    &height);
  if (labs((long)width - (long)config.width) < 4 && labs((long)height - (long)config.height) < 4) {
    return;
  }
  config.width = width;
  config.height = height;
  self.resizing = YES;
  __weak FlutterScreenCaptureKitCapturer *weakSelf = self;
  [stream updateConfiguration:config
            completionHandler:^(NSError *_Nullable error) {
              FlutterScreenCaptureKitCapturer *strongSelf = weakSelf;
              if (strongSelf == nil) return;
              dispatch_async(strongSelf.captureQueue, ^{
                strongSelf.resizing = NO;
              });
            }];
}

- (SCDisplay *)selectDisplayFromContent:(SCShareableContent *)content
                               sourceId:(NSString *)sourceId API_AVAILABLE(macos(12.3)) {
  if (content.displays.count == 0) {
    return nil;
  }

  if (sourceId != nil && sourceId.length > 0) {
    for (SCDisplay *display in content.displays) {
      if ([[NSString stringWithFormat:@"%u", display.displayID] isEqualToString:sourceId]) {
        return display;
      }
    }
  }

  CGDirectDisplayID mainDisplay = CGMainDisplayID();
  for (SCDisplay *display in content.displays) {
    if (display.displayID == mainDisplay) {
      return display;
    }
  }

  return content.displays.firstObject;
}

// Fogata: reports, once, that a running capture ended by itself.
- (void)reportEnded {
  @synchronized(self) {
    if (!self.running || self.ended) return;
    self.ended = YES;
  }
  void (^ended)(void) = self.onEnded;
  if (ended != nil) ended();
}

// Fogata: the stream stopped by itself: the system's own "stop sharing", the shared
// window or display went away, or an error.
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error API_AVAILABLE(macos(12.3)) {
  NSLog(@"ScreenCaptureKit stream stopped: %@", error);
  [self reportEnded];
}

- (void)stream:(SCStream *)stream
didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
        ofType:(SCStreamOutputType)type API_AVAILABLE(macos(12.3)) {
  if (type != SCStreamOutputTypeScreen) {
    return;
  }

  // Fogata: a frame that says the stream stopped (what was shared is gone) ends it.
  CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
  if (attachments != NULL && CFArrayGetCount(attachments) > 0) {
    NSDictionary *info = (__bridge NSDictionary *)CFArrayGetValueAtIndex(attachments, 0);
    NSNumber *status = info[SCStreamFrameInfoStatus];
    if ([status isKindOfClass:NSNumber.class] && status.integerValue == SCFrameStatusStopped) {
      [self reportEnded];
      return;
    }
  }

  CVPixelBufferRef pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
  if (pixelBuffer == nil) {
    return;
  }
  [self followWindowSize:sampleBuffer];

  CMTime timestamp = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
  int64_t timeStampNs = (int64_t)(CMTimeGetSeconds(timestamp) * 1000000000.0);

  id<RTCVideoFrameBuffer> rtcBuffer = [[RTCCVPixelBuffer alloc] initWithPixelBuffer:pixelBuffer];
  RTCVideoFrame *frame = [[RTCVideoFrame alloc] initWithBuffer:rtcBuffer
                                                      rotation:RTCVideoRotation_0
                                                   timeStampNs:timeStampNs];
  [self.delegate capturer:self.capturer didCaptureVideoFrame:frame];
}
#endif

@end
