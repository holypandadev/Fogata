#import "FogataPlayout.h"

#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <CoreAudio/CoreAudio.h>
#import <os/lock.h>

// The one being watched (the call factory's module), for +inputMayHaveChanged and
// +outputChosen:.
static __weak FogataPlayout* gCurrent = nil;

// How long the devices have to stay put before the input is looked at again. Longer than
// the audio module's own wait (500 ms) before it follows a new default device.
static const int64_t kSettleNs = 600 * NSEC_PER_MSEC;

// A stopped engine is looked at once more after this, before it is made again: the
// system stops it by itself for a moment when the devices change.
static const int64_t kStuckAgainNs = 500 * NSEC_PER_MSEC;

// Times in a row the engine is made again without anything changing in between, so a
// speaker that cannot be opened either does not have it made again forever.
static const int kMostRecoveries = 3;

// An engine made again that then ran this long worked: a stop after that is a new one, and
// the count above starts again.
static const int64_t kSteadyNs = 5 * NSEC_PER_SEC;

// When a microphone left out is tried again by itself, counted from when it was left out
// or last failed: the first before the call notices a microphone that reads nothing (8 s).
// After the last, it is tried again only when the devices, or those chosen, change.
static const int64_t kInputRetryNs[] = {2 * NSEC_PER_SEC, 10 * NSEC_PER_SEC,
                                        30 * NSEC_PER_SEC};
static const int kInputRetries = sizeof(kInputRetryNs) / sizeof(kInputRetryNs[0]);

// The system's lists of devices and its default input and output. A new default output
// (a speaker that woke up, or another one chosen) may let an engine run that could not.
static const AudioObjectPropertyAddress kWatched[] = {
    {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal,
     kAudioObjectPropertyElementMain},
    {kAudioHardwarePropertyDefaultInputDevice, kAudioObjectPropertyScopeGlobal,
     kAudioObjectPropertyElementMain},
    {kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal,
     kAudioObjectPropertyElementMain},
};

static BOOL Property(AudioObjectID object, AudioObjectPropertySelector selector,
                     AudioObjectPropertyScope scope, UInt32 size, void* data) {
  AudioObjectPropertyAddress address = {selector, scope, kAudioObjectPropertyElementMain};
  UInt32 got = size;
  return AudioObjectGetPropertyData(object, &address, 0, NULL, &got, data) == noErr &&
         got == size;
}

static BOOL IsDefault(NSString* _Nullable deviceId) {
  return deviceId.length == 0 || [deviceId isEqualToString:@"default"];
}

// The device of a unique id as the module lists them, or kAudioObjectUnknown.
static AudioObjectID DeviceOf(NSString* deviceId) {
  AudioObjectID device = kAudioObjectUnknown;
  CFStringRef uid = (__bridge CFStringRef)deviceId;
  AudioObjectPropertyAddress address = {kAudioHardwarePropertyTranslateUIDToDevice,
                                        kAudioObjectPropertyScopeGlobal,
                                        kAudioObjectPropertyElementMain};
  UInt32 size = sizeof(device);
  if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &address, sizeof(uid), &uid, &size,
                                 &device) != noErr) {
    return kAudioObjectUnknown;
  }
  return device;
}

// The device the module records from: the one chosen, or the system's default input
// (the module's first input, "default", has an empty id when there is no default input).
static AudioObjectID InputDevice(NSString* _Nullable deviceId) {
  if (!IsDefault(deviceId)) return DeviceOf(deviceId);
  AudioObjectID device = kAudioObjectUnknown;
  if (!Property(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultInputDevice,
                kAudioObjectPropertyScopeGlobal, sizeof(device), &device)) {
    return kAudioObjectUnknown;
  }
  return device;
}

static uint64_t Now(void) { return clock_gettime_nsec_np(CLOCK_UPTIME_RAW); }

static UInt32 InputChannels(AudioObjectID device) {
  AudioObjectPropertyAddress address = {kAudioDevicePropertyStreamConfiguration,
                                        kAudioDevicePropertyScopeInput,
                                        kAudioObjectPropertyElementMain};
  UInt32 size = 0;
  if (AudioObjectGetPropertyDataSize(device, &address, 0, NULL, &size) != noErr ||
      size < sizeof(AudioBufferList)) {
    return 0;
  }
  AudioBufferList* list = (AudioBufferList*)malloc(size);
  if (list == NULL) return 0;
  UInt32 channels = 0;
  if (AudioObjectGetPropertyData(device, &address, 0, NULL, &size, list) == noErr) {
    for (UInt32 i = 0; i < list->mNumberBuffers; i++) {
      channels += list->mBuffers[i].mNumberChannels;
    }
  }
  free(list);
  return channels;
}

// Why the module could not open its input now, or nil when it could: what the module
// itself checks before it records (the permission, an input format with channels and a
// rate), read from the system so that nothing is opened.
static NSString* _Nullable WhyNoInput(RTCAudioDeviceModule* module) {
  if ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] !=
      AVAuthorizationStatusAuthorized) {
    return @"the microphone permission is not granted";
  }
  AudioObjectID device = InputDevice(module.inputDevice.deviceId);
  if (device == kAudioObjectUnknown) return @"there is no input device";
  UInt32 alive = 0;
  if (!Property(device, kAudioDevicePropertyDeviceIsAlive, kAudioObjectPropertyScopeGlobal,
                sizeof(alive), &alive) ||
      alive == 0) {
    return @"the input device is gone";
  }
  Float64 rate = 0;
  if (!Property(device, kAudioDevicePropertyNominalSampleRate, kAudioObjectPropertyScopeGlobal,
                sizeof(rate), &rate) ||
      rate <= 0 || InputChannels(device) == 0) {
    return @"the input device is not ready";
  }
  return nil;
}

@implementation FogataPlayout {
  // Weak: the factory owns it. Used only on `_queue` and only before -stop returns, which
  // waits for the queue, so never after the factory (and its threads) went.
  __weak RTCAudioDeviceModule* _module;
  dispatch_queue_t _queue;
  BOOL _observing;
  AudioObjectPropertyListenerBlock _listener;

  // On `_queue` only.
  BOOL _stopped;
  BOOL _inputAvailable;  // what the module was last told
  BOOL _inputFailed;     // the input could not be opened; left out until tried again
  int _recoveries;
  uint64_t _lastRecovery;  // when the engine was last made again (Now())
  uint64_t _changes;     // the latest device change, to wait until they settle
  BOOL _checking;        // a look at a stopped engine is due
  // What the module said of the stops since the last look: whether any came, and whether
  // any was with the input (its engine meant to record).
  BOOL _stopSeen;
  BOOL _stopWithInput;
  // The microphone's own tries after it was left out: how many since the devices (or those
  // chosen) last changed, and the one waiting, by its number (a change cancels it).
  int _inputRetries;
  BOOL _retryPending;
  uint64_t _retryRound;

  // On the module's thread only, inside its changes: whether the engine about to start was
  // made in this change (the module then gives its output the chosen device itself), and
  // whether it last started with the input.
  BOOL _engineIsNew;
  BOOL _engineStartedWithInput;

  // The output chosen (`deviceId`, nil for the system's default); any thread, under the lock.
  os_unfair_lock _outputLock;
  NSString* _chosenOutput;
}

- (instancetype)initWithModule:(RTCAudioDeviceModule*)module observe:(BOOL)observe {
  self = [super init];
  if (self == nil) return nil;
  _module = module;
  _queue = dispatch_queue_create("fogata.playout", DISPATCH_QUEUE_SERIAL);
  _inputAvailable = YES;  // the module's own default
  _observing = observe;
  _outputLock = OS_UNFAIR_LOCK_INIT;
  gCurrent = self;

  // The module follows the devices itself, but a new default input does not make it try
  // the input it was refused before.
  __weak FogataPlayout* weakSelf = self;
  _listener = ^(UInt32 count, const AudioObjectPropertyAddress* addresses) {
    [weakSelf devicesChanged];
  };
  for (size_t i = 0; i < sizeof(kWatched) / sizeof(kWatched[0]); i++) {
    AudioObjectAddPropertyListenerBlock(kAudioObjectSystemObject, &kWatched[i], _queue, _listener);
  }
  if (observe) module.observer = self;

  dispatch_async(_queue, ^{
    [self refresh];
  });
  return self;
}

- (void)stop {
  for (size_t i = 0; i < sizeof(kWatched) / sizeof(kWatched[0]); i++) {
    AudioObjectRemovePropertyListenerBlock(kAudioObjectSystemObject, &kWatched[i], _queue,
                                           _listener);
  }
  RTCAudioDeviceModule* module = _module;
  if (_observing && module.observer == self) module.observer = nil;
  dispatch_sync(_queue, ^{
    self->_stopped = YES;
  });
}

+ (void)inputMayHaveChanged {
  FogataPlayout* current = gCurrent;
  if (current == nil) return;
  dispatch_async(current->_queue, ^{
    [current startOver];
  });
}

+ (void)outputChosen:(NSString*)deviceId {
  FogataPlayout* current = gCurrent;
  if (current == nil) return;
  os_unfair_lock_lock(&current->_outputLock);
  current->_chosenOutput = IsDefault(deviceId) ? nil : [deviceId copy];
  os_unfair_lock_unlock(&current->_outputLock);
  dispatch_async(current->_queue, ^{
    [current startOver];
  });
}

// On `_queue`: something changed (a device, the devices chosen, the permission): the
// input is tried again, and the engine may be made again as many times as before.
- (void)startOver {
  _inputFailed = NO;
  _recoveries = 0;
  _inputRetries = 0;
  _retryPending = NO;
  _retryRound++;
  [self refresh];
}

// On `_queue`.
- (void)devicesChanged {
  uint64_t change = ++_changes;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kSettleNs), _queue, ^{
    if (change != self->_changes) return;
    [self startOver];
  });
}

// On `_queue`: tells the module whether its input can be used, then looks at the engine.
- (void)refresh {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil) return;
  NSString* why = WhyNoInput(module);
  if (why == nil && _inputFailed) why = @"it could not be opened";
  BOOL available = why == nil;
  if (available != _inputAvailable) {
    // With WebRTC recording already, making the input available opens it here.
    NSInteger result = [module setEngineAvailability:(RTCAudioEngineAvailability){available, YES}];
    if (result == 0) {
      _inputAvailable = available;
      NSLog(@"Fogata: microphone %@%@", available ? @"usable" : @"not usable, playing only: ",
            why ?: @"");
    } else {
      NSLog(@"Fogata: microphone could not be %@ (%ld)", available ? @"opened" : @"closed",
            (long)result);
      if (available) {
        _inputFailed = YES;
        [self retryInputLater];
      }
    }
  }
  [self lookAtEngine];
}

// On `_queue`: the microphone left out is tried again after a while, a few times at most
// (kInputRetryNs); then only when something changes (-startOver).
- (void)retryInputLater {
  if (_retryPending || _stopped) return;
  if (_inputRetries >= kInputRetries) {
    NSLog(@"Fogata: the microphone was tried again %d times; tried next when the devices "
          @"change",
          kInputRetries);
    return;
  }
  int64_t wait = kInputRetryNs[_inputRetries++];
  uint64_t round = ++_retryRound;
  _retryPending = YES;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, wait), _queue, ^{
    if (round != self->_retryRound) return;
    self->_retryPending = NO;
    if (self->_stopped || !self->_inputFailed) return;
    NSLog(@"Fogata: trying the microphone again (%d of %d)", self->_inputRetries, kInputRetries);
    self->_inputFailed = NO;
    self->_recoveries = 0;
    [self refresh];
  });
}

// On `_queue`: an engine stopped while the module counts itself as playing is made again.
// Looked at twice, as the system stops it for a moment by itself. The first time it is
// made as it was, the input in it if it was: an engine stops with its input in it as well
// when an output is chosen or the system switches devices, and the microphone has no part
// in that. Only when that engine does not run either (or stops again at once) is it made
// without the input, which is then tried again by itself (-retryInputLater).
- (void)lookAtEngine {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil || _checking) return;
  if (!module.isPlaying || module.isEngineRunning) {
    // Started again: the stops heard say nothing of a later one.
    _stopSeen = NO;
    _stopWithInput = NO;
    return;
  }
  _checking = YES;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kStuckAgainNs), _queue, ^{
    self->_checking = NO;
    // An engine the module stopped with only the speaker in it (the input not wanted, or
    // already left out) did not fail for the input: the microphone is left as it is. With
    // no stop heard (another observer has the module, or an engine made again here that
    // did not start), the input may be the cause, as it is the one thing that fails this
    // way.
    BOOL inputsFault = !self->_stopSeen || self->_stopWithInput;
    self->_stopSeen = NO;
    self->_stopWithInput = NO;
    RTCAudioDeviceModule* module = self->_module;
    // The module's getters wait for its thread (WebRTC-SDK's RTCAudioDeviceModule calls
    // it with BlockingCall, on the thread every change of its engine runs on): an engine
    // still starting is seen once it has started, or failed to.
    if (self->_stopped || module == nil || !module.isPlaying || module.isEngineRunning) return;
    uint64_t now = Now();
    // The last one made ran a while: this is a new stop, not the same one again.
    if (self->_recoveries > 0 && now - self->_lastRecovery >= (uint64_t)kSteadyNs) {
      self->_recoveries = 0;
    }
    if (self->_recoveries >= kMostRecoveries) {
      NSLog(@"Fogata: the audio engine does not play, and was made again %d times already",
            kMostRecoveries);
      return;
    }
    // Made as it was first; the input is left out only when an engine made again here
    // with it stopped as well.
    BOOL leftOut = self->_inputAvailable && inputsFault && self->_recoveries > 0;
    BOOL input = self->_inputAvailable && !leftOut;
    self->_recoveries++;
    self->_lastRecovery = now;
    if (leftOut) self->_inputFailed = YES;
    // Neither side available releases the engine; a new one is made as decided.
    [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
    NSInteger result = [module setEngineAvailability:(RTCAudioEngineAvailability){input, YES}];
    self->_inputAvailable = input;
    BOOL running = module.isEngineRunning;
    NSLog(@"Fogata: the audio engine stopped while playing; made again %@ (%ld, running %d)",
          leftOut ? @"without the microphone" : @"as it was", (long)result, running);
    if (result != 0 || !running) {
      // Not running: looked at again, to be made the next way.
      [self lookAtEngine];
    } else if (leftOut) {
      [self retryInputLater];
    }
  });
}

// On the module's thread, inside its change, after it chose its devices and before the
// engine starts. The module gives the output the chosen device only to an engine it made
// in this change; when it adds the input to an engine that played alone, the system's
// voice processing takes the system's default input and output, and the others' voices go
// to the default output instead of the one chosen (WebRTC-SDK's audio_engine_device.mm,
// its step "Configure device", skipped then). The chosen output is given here instead,
// as the module does for a new engine.
- (void)keepChosenOutput:(AVAudioEngine*)engine {
  os_unfair_lock_lock(&_outputLock);
  NSString* chosen = _chosenOutput;
  os_unfair_lock_unlock(&_outputLock);
  if (chosen == nil) return;
  AudioObjectID device = DeviceOf(chosen);
  UInt32 alive = 0;
  if (device == kAudioObjectUnknown ||
      !Property(device, kAudioDevicePropertyDeviceIsAlive, kAudioObjectPropertyScopeGlobal,
                sizeof(alive), &alive) ||
      alive == 0) {
    // Gone: the module plays on the system's default then, as it does here.
    NSLog(@"Fogata: the output chosen is gone; the system's default plays");
    return;
  }
  AudioUnit unit = engine.outputNode.audioUnit;
  if (unit == NULL) return;
  OSStatus status = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice,
                                         kAudioUnitScope_Global, 0, &device, sizeof(device));
  NSLog(@"Fogata: the output chosen given again with the microphone (%d)", (int)status);
}

#pragma mark - RTCAudioDeviceModuleDelegate

// Called on the module's thread, inside its change: looked at once that change is done.
// `isRecordingEnabled` is whether the engine the module goes on to make is to record
// (WebRTC-SDK passes the state it changes to): false when the input is not wanted or not
// available.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
                 didStopEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
  if (isPlayoutEnabled) {
    dispatch_async(_queue, ^{
      self->_stopSeen = YES;
      if (isRecordingEnabled) self->_stopWithInput = YES;
      [self lookAtEngine];
    });
  }
  return 0;
}

- (void)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
    didReceiveSpeechActivityEvent:(RTCSpeechActivityEvent)speechActivityEvent {
}

// On the module's thread, inside its change.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               didCreateEngine:(AVAudioEngine*)engine {
  _engineIsNew = YES;
  _engineStartedWithInput = NO;
  return 0;
}

// On the module's thread, inside its change.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               willStartEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
  BOOL inputAdded = isPlayoutEnabled && isRecordingEnabled && !_engineIsNew &&
                    !_engineStartedWithInput;
  _engineIsNew = NO;
  _engineStartedWithInput = isRecordingEnabled;
  if (inputAdded) [self keepChosenOutput:engine];
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
              didDisableEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
             willReleaseEngine:(AVAudioEngine*)engine {
  return 0;
}

// Nothing connected: the module connects the input node itself.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
                        engine:(AVAudioEngine*)engine
      configureInputFromSource:(nullable AVAudioNode*)source
                 toDestination:(AVAudioNode*)destination
                    withFormat:(AVAudioFormat*)format
                       context:(NSDictionary*)context {
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
                        engine:(AVAudioEngine*)engine
     configureOutputFromSource:(AVAudioNode*)source
                 toDestination:(nullable AVAudioNode*)destination
                    withFormat:(AVAudioFormat*)format
                       context:(NSDictionary*)context {
  return 0;
}

- (void)audioDeviceModuleDidUpdateDevices:(RTCAudioDeviceModule*)audioDeviceModule {
}

@end
