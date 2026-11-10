#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

NS_ASSUME_NONNULL_BEGIN

// Keeps the others' voices playing when the microphone cannot be used (macOS).
//
// The AudioEngine audio module plays and records with one AVAudioEngine. When it is asked
// to record while it plays, it stops the engine, and if the input then cannot be opened
// (no input device, an input not ready yet, the microphone permission not granted) it
// gives up without starting the engine again, while it still counts itself as playing:
// nothing is played any more, and asking it to play changes nothing, until something
// rebuilds the engine. So:
//
// - While no input can be opened, the module is told the input is not available
//   (`setEngineAvailability:`): it plays alone, and WebRTC's requests to record are kept
//   until the input is available again, which happens when an input device is added or
//   the default one changes, a microphone is asked for (`getUserMedia`) or chosen.
// - If the engine still ends up stopped while the module counts itself as playing, it is
//   made again without the input, and the input is not tried again until one of those.
//
// All the work runs on a queue of its own: the module's calls wait for its thread, which
// can be busy for up to two seconds while an engine starts.
@interface FogataPlayout : NSObject <RTCAudioDeviceModuleDelegate>

// Watches `module` (the call factory's). With `observe`, it becomes the module's observer,
// to hear when its engine stops; without (another observer was set), it only keeps the
// input's availability.
- (instancetype)initWithModule:(RTCAudioDeviceModule*)module observe:(BOOL)observe;

- (instancetype)init NS_UNAVAILABLE;

// Stops watching (the factory is going).
- (void)stop;

// The microphone may have become usable (permission asked, a microphone chosen): looks
// again, for the module being watched. Any thread.
+ (void)inputMayHaveChanged;

@end

NS_ASSUME_NONNULL_END
