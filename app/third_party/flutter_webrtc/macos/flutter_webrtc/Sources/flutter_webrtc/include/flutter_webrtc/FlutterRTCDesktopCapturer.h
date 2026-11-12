#if TARGET_OS_IPHONE
#import <Flutter/Flutter.h>
#elif TARGET_OS_OSX
#import <FlutterMacOS/FlutterMacOS.h>
#endif
#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

#import "FlutterWebRTCPlugin.h"

@interface FlutterWebRTCPlugin (DesktopCapturer)

- (void)getDisplayMedia:(nonnull NSDictionary*)constraints result:(nonnull FlutterResult)result;

- (void)getDesktopSources:(nonnull NSDictionary*)argsMap result:(nonnull FlutterResult)result;

- (void)updateDesktopSources:(nonnull NSDictionary*)argsMap result:(nonnull FlutterResult)result;

- (void)getDesktopSourceThumbnail:(nonnull NSDictionary*)argsMap
                           result:(nonnull FlutterResult)result;

// Fogata: gives the encoders of the screen or window shared as track [trackId] its last
// frame again (ScreenCaptureKit gives none while it stays still); NO for any other track,
// whose capture gives frames all the time.
- (BOOL)refreshScreenTrack:(nonnull NSString*)trackId;

@end