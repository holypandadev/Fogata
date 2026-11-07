#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// The microphone's noise suppression (RNNoise, in the application's Rust library) as the
// audio processing module's capture post-processing: it runs on WebRTC's audio thread on
// every 10 ms of microphone sound, after echo cancelling and gain control, and the switch
// only flips a flag it reads, so a change applies from the next 10 ms without opening the
// microphone again.
@interface FogataDenoise : NSObject

// Turns it on or off. The first call makes the suppressor with the table of functions at
// `api`, which the application hands over (fogata-ffi's denoise.rs); NO if there is none
// to make it with.
+ (BOOL)suppress:(BOOL)on api:(int64_t)api;

@end

NS_ASSUME_NONNULL_END
