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
//   made again as it was: an engine also stops with its input in it when another output
//   is chosen or the system switches devices. Only if that one does not run either (or
//   stops again at once) is it made without the input, which is then tried again by
//   itself after 2, 10 and 30 s, and at each of those changes (or of the default output,
//   or of the output chosen). When the engine that stopped only played, the speaker, not
//   the input, failed: it is made again as it was.
// - When the module adds the input to an engine that played alone, the system's voice
//   processing takes the system's default devices and the module does not give the
//   output the one chosen again: it is given here, before the engine starts.
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

// The output `deviceId` was chosen (as the module lists it; "default" for the system's):
// kept for the engine (see above), and the microphone left out is tried again. Any
// thread.
+ (void)outputChosen:(NSString*)deviceId;

@end

NS_ASSUME_NONNULL_END
