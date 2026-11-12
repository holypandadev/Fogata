#import "FogataPlayout.h"

#import <AVFoundation/AVFoundation.h>
#import <CoreAudio/CoreAudio.h>

// The one being watched (the call factory's module), for +inputMayHaveChanged.
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

// The device the module records from: the one chosen, or the system's default input
// (the module's first input, "default", has an empty id when there is no default input).
static AudioObjectID InputDevice(NSString* _Nullable deviceId) {
  AudioObjectID device = kAudioObjectUnknown;
  if (deviceId.length == 0 || [deviceId isEqualToString:@"default"]) {
    if (!Property(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultInputDevice,
                  kAudioObjectPropertyScopeGlobal, sizeof(device), &device)) {
      return kAudioObjectUnknown;
    }
    return device;
  }
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
  BOOL _inputFailed;     // the input could not be opened; not tried until something changes
  int _recoveries;
  uint64_t _changes;     // the latest device change, to wait until they settle
  BOOL _checking;        // a look at a stopped engine is due
  // What the module said of the stops since the last look: whether any came, and whether
  // any was with the input (its engine meant to record).
  BOOL _stopSeen;
  BOOL _stopWithInput;
}

- (instancetype)initWithModule:(RTCAudioDeviceModule*)module observe:(BOOL)observe {
  self = [super init];
  if (self == nil) return nil;
  _module = module;
  _queue = dispatch_queue_create("fogata.playout", DISPATCH_QUEUE_SERIAL);
  _inputAvailable = YES;  // the module's own default
  _observing = observe;
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
    current->_inputFailed = NO;
    current->_recoveries = 0;
    [current refresh];
  });
}

// On `_queue`.
- (void)devicesChanged {
  uint64_t change = ++_changes;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kSettleNs), _queue, ^{
    if (change != self->_changes) return;
    self->_inputFailed = NO;
    self->_recoveries = 0;
    [self refresh];
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
      if (available) _inputFailed = YES;
    }
  }
  [self lookAtEngine];
}

// On `_queue`: an engine stopped while the module counts itself as playing is made again,
// without the input when the input may be the cause. Looked at twice, as the system stops
// it for a moment by itself.
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
    // no stop heard (another observer has the module), the input is taken for the cause,
    // as it is the one thing that fails this way.
    BOOL inputsFault = !self->_stopSeen || self->_stopWithInput;
    self->_stopSeen = NO;
    self->_stopWithInput = NO;
    RTCAudioDeviceModule* module = self->_module;
    // The module's getters wait for its thread (WebRTC-SDK's RTCAudioDeviceModule calls
    // it with BlockingCall, on the thread every change of its engine runs on): an engine
    // still starting is seen once it has started, or failed to.
    if (self->_stopped || module == nil || !module.isPlaying || module.isEngineRunning) return;
    if (self->_recoveries >= kMostRecoveries) {
      NSLog(@"Fogata: the audio engine does not play, and was made again %d times already",
            kMostRecoveries);
      return;
    }
    self->_recoveries++;
    if (inputsFault) self->_inputFailed = YES;
    BOOL input = !inputsFault && self->_inputAvailable;
    // Neither side available releases the engine; a new one is made with the speaker, and
    // the input only if it was not the cause.
    [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
    NSInteger result = [module setEngineAvailability:(RTCAudioEngineAvailability){input, YES}];
    self->_inputAvailable = input;
    NSLog(@"Fogata: the audio engine stopped while playing; made again %@ (%ld, running %d)",
          inputsFault ? @"without the microphone" : @"as it was", (long)result,
          module.isEngineRunning);
  });
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

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               didCreateEngine:(AVAudioEngine*)engine {
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               willStartEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
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
