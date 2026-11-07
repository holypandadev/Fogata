#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

NS_ASSUME_NONNULL_BEGIN

@interface FlutterScreenCaptureKitCapturer : NSObject

- (instancetype)initWithDelegate:(id<RTCVideoCapturerDelegate>)delegate;

// Fogata: runs once, on any queue, when a running capture ends by itself: the shared
// window closed, the system's own "stop sharing", or an error. Never after
// stopCaptureWithCompletion: nor for a capture that did not start.
@property(nonatomic, copy, nullable) void (^onEnded)(void);

- (void)startCaptureWithFPS:(NSInteger)fps
                   sourceId:(NSString* _Nullable)sourceId
                  onStarted:(void (^ _Nonnull)(NSError * _Nullable error))onStarted;

// Fogata: a window (sourceId is its window number) instead of a display.
- (void)startCaptureWithFPS:(NSInteger)fps
                   sourceId:(NSString* _Nullable)sourceId
                     window:(BOOL)window
                  onStarted:(void (^ _Nonnull)(NSError * _Nullable error))onStarted;

- (void)stopCaptureWithCompletion:(void (^ _Nonnull)(void))completion;

@end

NS_ASSUME_NONNULL_END
