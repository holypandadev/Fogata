#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

NS_ASSUME_NONNULL_BEGIN

// Keeps the others' voices playing when the microphone cannot be used, and plays and
// records on the devices chosen (macOS).
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
// - If the engine still ends up stopped while the module counts itself as playing (or,
//   with devices chosen, recording), it is made again as it was: an engine also stops
//   with its input in it when the system switches devices. Only if that one does not run
//   either (or stops again at once) is the pair chosen taken as refused, and then the
//   input left out,
//   which is then tried again by itself after 2, 10 and 30 s, and at each change of the
//   devices, of the default output or of those chosen.
// - The devices chosen are given to the engine here, not by the module. Without the
//   system's voice processing (Fogata cancels the echo with WebRTC's own), the engine's
//   input and output are one audio unit, which takes one device: the module gave it the
//   output chosen while it recorded, the system refused any device without an input
//   (kAudioUnitErr_InvalidPropertyValue), and the module was left with no engine at all,
//   the microphone lost with the sound. The module is kept on the system's devices. When
//   the output and the microphone the call uses can be put together (the same device, or
//   two at the same rate the engine has not refused in this call), the engine's unit is
//   given, before the engine reads its devices and before it starts, the output chosen
//   alone while it does not record and the pair while it does: the one device, or a
//   private aggregate of the two (the output's clock, the input's drift compensated), as
//   the system itself does for its default devices, each kept only when the engine reads
//   an output and an input from it. When they cannot, nothing is given, and the engine
//   follows the system's devices as with nothing chosen, muted or not; a pair the engine
//   refuses is taken as one that cannot, and a new engine follows the system. Nothing here
//   ever asks the module to record.
//
// All the work runs on a queue of its own: the module's calls wait for its thread, which
// can be busy for up to two seconds while an engine starts.
@interface FogataPlayout : NSObject <RTCAudioDeviceModuleDelegate>

// Watches `module` (the call factory's). With `observe`, it becomes the module's observer,
// to hear when its engine stops and to give it the devices chosen; without (another
// observer was set), it only keeps the input's availability, and the module chooses the
// devices itself.
- (instancetype)initWithModule:(RTCAudioDeviceModule*)module observe:(BOOL)observe;

- (instancetype)init NS_UNAVAILABLE;

// Stops watching (the factory is going).
- (void)stop;

// Whether the devices chosen are given to the engine here (+outputChosen:name:,
// +inputChosen:name:) rather than to the module. Any thread.
+ (BOOL)choosesDevices;

// The microphone may have become usable (permission asked, a microphone chosen): looks
// again, for the module being watched. Any thread.
+ (void)inputMayHaveChanged;

// The output `deviceId` was chosen (as the module lists it; "default" for the system's),
// under the name the list gave it: the engine is made again with it, the same one chosen
// again included, and the microphone left out is tried again. Any thread.
+ (void)outputChosen:(NSString*)deviceId name:(nullable NSString*)name;

// The input `deviceId` was chosen, as +outputChosen:name:. Any thread.
+ (void)inputChosen:(NSString*)deviceId name:(nullable NSString*)name;

// Answers, on its own queue, facts only. For the output: the one chosen ("chosen", absent
// for the system's default) and the name the list gave it ("chosenName"), whether it is
// in the system now ("chosenPresent"), the one the engine was last given ("inUse",
// "default" for the system's; absent before an engine started with a device chosen) and
// its name now ("inUseName"); the same for the input ("inputChosen", "inputChosenName",
// "inputChosenPresent", "inputInUse", "inputInUseName"). Whether a choice is still being
// carried out ("changing"). Whether the pair the devices chosen make can be put together
// ("combinable", only with a device chosen) and "reason": "rate" when their rates differ.
// What follows the system's devices is given as those devices, by their own ids. Whether
// the module was asked to record ("recordingAsked"),
// whether it records now (asked, its input let in, its engine running: "recording"), and
// whether its input is let in ("inputUsable"). Any thread.
+ (void)state:(void (^)(NSDictionary<NSString*, id>* state))answer;

// Whether `deviceId` is a device made here (an aggregate), not to be listed. Any thread.
+ (BOOL)isOwnDevice:(NSString*)deviceId;

// For app/integration_test/call_test.dart only (the method `fogataTestRefuse`, which
// nothing else calls), and only in a process started with FOGATA_TEST_REFUSE set: 1, no
// pair of devices chosen can be put together; 2, the next pair given with the microphone
// is refused, in either order, once; 3, the next start of an engine given the pair fails,
// once; 0, as always. Any thread.
+ (void)testRefuse:(int)mode;

// A call starts: the devices chosen and the pairs refused before are forgotten (the
// call's own choices follow), and the test's refusal is off. Any thread.
+ (void)newCall;

@end

NS_ASSUME_NONNULL_END
