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

static UInt32 Channels(AudioObjectID device, AudioObjectPropertyScope scope) {
  AudioObjectPropertyAddress address = {kAudioDevicePropertyStreamConfiguration, scope,
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

static UInt32 InputChannels(AudioObjectID device) {
  return Channels(device, kAudioDevicePropertyScopeInput);
}

static UInt32 OutputChannels(AudioObjectID device) {
  return Channels(device, kAudioDevicePropertyScopeOutput);
}

static BOOL IsAlive(AudioObjectID device) {
  UInt32 alive = 0;
  return device != kAudioObjectUnknown &&
         Property(device, kAudioDevicePropertyDeviceIsAlive, kAudioObjectPropertyScopeGlobal,
                  sizeof(alive), &alive) &&
         alive != 0;
}

static Float64 RateOf(AudioObjectID device) {
  Float64 rate = 0;
  if (!Property(device, kAudioDevicePropertyNominalSampleRate, kAudioObjectPropertyScopeGlobal,
                sizeof(rate), &rate)) {
    return 0;
  }
  return rate;
}

static NSString* _Nullable StringOf(AudioObjectID device, AudioObjectPropertySelector selector) {
  CFStringRef value = NULL;
  if (!Property(device, selector, kAudioObjectPropertyScopeGlobal, sizeof(value), &value) ||
      value == NULL) {
    return nil;
  }
  return (__bridge_transfer NSString*)value;
}

static NSString* UIDOf(AudioObjectID device) {
  return StringOf(device, kAudioDevicePropertyDeviceUID) ?: @"";
}

static NSString* NameOf(AudioObjectID device) {
  return StringOf(device, kAudioObjectPropertyName) ?: [NSString stringWithFormat:@"%u", device];
}

// The device the module plays on: the one chosen, or the system's default output.
static AudioObjectID OutputDevice(NSString* _Nullable deviceId) {
  if (!IsDefault(deviceId)) return DeviceOf(deviceId);
  AudioObjectID device = kAudioObjectUnknown;
  if (!Property(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultOutputDevice,
                kAudioObjectPropertyScopeGlobal, sizeof(device), &device)) {
    return kAudioObjectUnknown;
  }
  return device;
}

// The unique ids of the aggregates made here start so; private, they are listed only in
// this process.
static NSString* const kOwnPrefix = @"fogata.route.";

// Makes a private aggregate playing on `output` and recording from `input`: the output's
// clock, the input's drift compensated, the input's device first or the output's
// (`inputFirst`). An AVAudioEngine given such an aggregate can read its output with no
// channels at all while the HAL and the audio unit itself see them, and the module then
// refuses it ("Output device not available"); the engine can read them in one order of
// the two devices and not in the other, whatever the aggregate's other settings (private
// or not, a clock device, drift compensation), and which order it reads is not fixed. So
// both are tried, and the engine's own reading decides (-giveEngine:...). nil `why` when
// made.
static AudioObjectID MakeAggregate(AudioObjectID output, AudioObjectID input, BOOL inputFirst,
                                   NSString* _Nullable* _Nonnull why) {
  static int made = 0;
  Float64 outputRate = RateOf(output), inputRate = RateOf(input);
  if (outputRate <= 0 || inputRate <= 0 || fabs(outputRate - inputRate) > 0.5) {
    *why = [NSString stringWithFormat:@"the output runs at %.0f Hz and the microphone at %.0f Hz",
                                      outputRate, inputRate];
    return kAudioObjectUnknown;
  }
  // The engine records from the aggregate's first input channels, and plays on its first
  // output channels unless mapped: the output's device first only when it records nothing.
  if (!inputFirst && InputChannels(output) > 0) {
    *why = @"the output records as well";
    return kAudioObjectUnknown;
  }
  NSString* outputUID = UIDOf(output);
  NSDictionary* outputSub = @{@kAudioSubDeviceUIDKey : outputUID};
  NSDictionary* inputSub =
      @{@kAudioSubDeviceUIDKey : UIDOf(input), @kAudioSubDeviceDriftCompensationKey : @1};
  NSString* uid = [NSString stringWithFormat:@"%@%d.%d", kOwnPrefix, getpid(), ++made];
  NSDictionary* description = @{
    @kAudioAggregateDeviceNameKey : @"Fogata",
    @kAudioAggregateDeviceUIDKey : uid,
    @kAudioAggregateDeviceIsPrivateKey : @1,
    @kAudioAggregateDeviceIsStackedKey : @0,
    @kAudioAggregateDeviceMainSubDeviceKey : outputUID,
    @kAudioAggregateDeviceSubDeviceListKey :
        inputFirst ? @[ inputSub, outputSub ] : @[ outputSub, inputSub ],
  };
  AudioObjectID aggregate = kAudioObjectUnknown;
  OSStatus status =
      AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)description, &aggregate);
  if (status != noErr || aggregate == kAudioObjectUnknown) {
    *why = [NSString stringWithFormat:@"the system did not make a device of both (%d)", (int)status];
    return kAudioObjectUnknown;
  }
  // The HAL lists its streams as soon as it is made; waited for all the same, 100 ms at
  // most: this runs on the module's thread, which the window's own calls to the module (the
  // device lists, a device chosen) wait for, as they wait already for an engine starting.
  for (int i = 0; i < 5 && (OutputChannels(aggregate) == 0 || InputChannels(aggregate) == 0);
       i++) {
    usleep(20 * 1000);
  }
  if (OutputChannels(aggregate) == 0 || InputChannels(aggregate) == 0) {
    AudioHardwareDestroyAggregateDevice(aggregate);
    *why = @"the device of both has no channels";
    return kAudioObjectUnknown;
  }
  *why = nil;
  return aggregate;
}

// Why the module could not open its input now, or nil when it could: what the module
// itself checks before it records (the permission, an input format with channels and a
// rate), read from the system so that nothing is opened. `deviceId` is the input chosen
// (nil: the system's default).
static NSString* _Nullable WhyNoInput(NSString* _Nullable deviceId) {
  if ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] !=
      AVAuthorizationStatusAuthorized) {
    return @"the microphone permission is not granted";
  }
  AudioObjectID device = InputDevice(deviceId);
  if (device == kAudioObjectUnknown) return @"there is no input device";
  if (!IsAlive(device)) return @"the input device is gone";
  if (RateOf(device) <= 0 || InputChannels(device) == 0) {
    return @"the input device is not ready";
  }
  return nil;
}

// Whether the device of `deviceId` is in the system now, with channels on that side.
static BOOL IsPresent(NSString* _Nullable deviceId, BOOL output) {
  if (IsDefault(deviceId)) return YES;
  AudioObjectID device = DeviceOf(deviceId);
  return IsAlive(device) && (output ? OutputChannels(device) : InputChannels(device)) > 0;
}

// The pair an engine plays and records on: the output chosen when it is there, else the
// system's default output; the input chosen when it is there, else the system's default
// input (the microphone the call uses, recording or not).
static AudioObjectID PairOutput(NSString* _Nullable chosen) {
  return IsPresent(chosen, YES) && chosen != nil ? DeviceOf(chosen) : OutputDevice(nil);
}

static AudioObjectID PairInput(NSString* _Nullable chosen) {
  return IsPresent(chosen, NO) && chosen != nil ? DeviceOf(chosen) : InputDevice(nil);
}

static NSString* PairOf(AudioObjectID output, AudioObjectID input) {
  return [NSString stringWithFormat:@"%@|%@", UIDOf(output), UIDOf(input)];
}

// Whether `output` and `input` can play and record together, from the devices' properties
// only, nothing opened: the same device, no input, or two at the same nominal rate (an
// aggregate cannot resample). nil `why` when they can.
static BOOL Combinable(AudioObjectID output, AudioObjectID input,
                       NSString* _Nullable* _Nonnull why) {
  *why = nil;
  if (output == kAudioObjectUnknown) {
    *why = @"there is no output";
    return NO;
  }
  if (input == kAudioObjectUnknown || input == output) return YES;
  Float64 outputRate = RateOf(output), inputRate = RateOf(input);
  if (outputRate <= 0 || inputRate <= 0 || fabs(outputRate - inputRate) > 0.5) {
    *why = @"rate";
    return NO;
  }
  return YES;
}

// For the call test only (flutter_webrtc's method `fogataTestRefuse`, which only
// app/integration_test/call_test.dart calls, with FOGATA_TEST_REFUSE set for it): 1, every
// pair is taken for one that cannot be put together; 2, the next device the engine reads
// a sound from is taken as refused, once. 0 otherwise, as always outside that test.
static int gTestRefuse = 0;

// The system's devices other than this process's own aggregates, each by its unique id
// and its object: a device unplugged and plugged in again between two notifications keeps
// its unique id and comes back as another object, which is a change too.
static NSSet<NSString*>* OtherDevices(void) {
  NSMutableSet<NSString*>* uids = [NSMutableSet set];
  AudioObjectPropertyAddress address = {kAudioHardwarePropertyDevices,
                                        kAudioObjectPropertyScopeGlobal,
                                        kAudioObjectPropertyElementMain};
  UInt32 size = 0;
  if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &address, 0, NULL, &size) !=
          noErr ||
      size == 0) {
    return uids;
  }
  NSMutableData* data = [NSMutableData dataWithLength:size];
  if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &address, 0, NULL, &size,
                                 data.mutableBytes) != noErr) {
    return uids;
  }
  const AudioObjectID* devices = data.bytes;
  for (UInt32 i = 0; i < size / sizeof(AudioObjectID); i++) {
    NSString* uid = UIDOf(devices[i]);
    if (uid != nil && ![uid hasPrefix:kOwnPrefix]) {
      [uids addObject:[NSString stringWithFormat:@"%@#%u", uid, (unsigned)devices[i]]];
    }
  }
  return uids;
}

@implementation FogataPlayout {
  // Weak: the factory owns it. Used only on `_queue` and only before -stop returns, which
  // waits for the queue, so never after the factory (and its threads) went.
  __weak RTCAudioDeviceModule* _module;
  dispatch_queue_t _queue;
  BOOL _observing;
  AudioObjectPropertyListenerBlock _listener;

  // On `_queue` only.
  NSSet<NSString*>* _otherDevices;  // OtherDevices() at the last change heard
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

  // On the module's thread only, inside its changes: whether a device was given to the
  // engine's unit here, even one then taken back (else it follows the system's default
  // devices, as the engine does by itself, and only a new engine can again), the aggregate
  // made here, what it is of ("<output uid>|<input uid>") and its output channels before
  // the output's (mapped away), and the device last given to this engine and read from.
  BOOL _engineRouted;
  AudioObjectID _aggregate;
  NSString* _aggregateOf;
  UInt32 _aggregateSkipped;
  AudioObjectID _given;

  // Any thread, under the lock: the output and input chosen (`deviceId`, nil for the
  // system's default) with the names the list gave them; the pairs ("<output uid>|<input
  // uid>", the input's empty for an engine that only played) the engine refused in this
  // call, until a device is chosen again; the pair given to the engine now (nil: it
  // follows the system's devices); and the choices not yet carried out.
  os_unfair_lock _lock;
  NSString* _chosenOutput;
  NSString* _chosenOutputName;
  NSString* _chosenInput;
  NSString* _chosenInputName;
  NSMutableSet<NSString*>* _refused;
  NSString* _givenPair;
  BOOL _anewAsked;  // a new engine following the system is asked and not made yet
  NSString* _outputInUse;  // the uid given, nil while following the system
  NSString* _inputInUse;   // likewise, of the input
  int _pendingChoices;
}

+ (void)testRefuse:(int)mode {
  // Only in a process the call test started (FOGATA_TEST_REFUSE is set for it, and for
  // nothing else): elsewhere, whatever asks, nothing changes.
  if (getenv("FOGATA_TEST_REFUSE") == NULL) {
    NSLog(@"Fogata: test refusal asked outside the call test: nothing done");
    return;
  }
  gTestRefuse = mode;
  NSLog(@"Fogata: test: %@", mode == 1   ? @"no pair can be put together"
                             : mode == 2 ? @"the next device read is taken as refused"
                                         : mode == 3 ? @"the next start with the pair fails"
                                                     : @"nothing refused");
}

+ (void)newCall {
  FogataPlayout* current = gCurrent;
  gTestRefuse = 0;
  if (current == nil) return;
  // A call starts clean: what an earlier call chose or saw refused is not this one's (its
  // own choices follow at once).
  os_unfair_lock_lock(&current->_lock);
  current->_chosenOutput = nil;
  current->_chosenOutputName = nil;
  current->_chosenInput = nil;
  current->_chosenInputName = nil;
  [current->_refused removeAllObjects];
  current->_givenPair = nil;
  current->_outputInUse = nil;
  current->_inputInUse = nil;
  os_unfair_lock_unlock(&current->_lock);
}
- (instancetype)initWithModule:(RTCAudioDeviceModule*)module observe:(BOOL)observe {
  self = [super init];
  if (self == nil) return nil;
  _module = module;
  _queue = dispatch_queue_create("fogata.playout", DISPATCH_QUEUE_SERIAL);
  _inputAvailable = YES;  // the module's own default
  _observing = observe;
  _lock = OS_UNFAIR_LOCK_INIT;
  _aggregate = kAudioObjectUnknown;
  gCurrent = self;

  // The module follows the devices itself, but a new default input does not make it try
  // the input it was refused before.
  __weak FogataPlayout* weakSelf = self;
  _listener = ^(UInt32 count, const AudioObjectPropertyAddress* addresses) {
    FogataPlayout* me = weakSelf;
    if (me == nil) return;
    // A change of the list that is only this process's own aggregates, made or destroyed,
    // is not a change of the devices: it would start the counts over (-startOver), and an
    // engine whose aggregate is made anew at each remake would be made again forever.
    BOOL listOnly = YES;
    for (UInt32 i = 0; i < count; i++) {
      if (addresses[i].mSelector != kAudioHardwarePropertyDevices) listOnly = NO;
    }
    NSSet<NSString*>* others = OtherDevices();
    if (listOnly && [others isEqualToSet:me->_otherDevices]) return;
    me->_otherDevices = others;
    [me devicesChanged];
  };
  for (size_t i = 0; i < sizeof(kWatched) / sizeof(kWatched[0]); i++) {
    AudioObjectAddPropertyListenerBlock(kAudioObjectSystemObject, &kWatched[i], _queue, _listener);
  }
  if (observe) module.observer = self;

  dispatch_async(_queue, ^{
    self->_otherDevices = OtherDevices();
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
  // The aggregate made last stays until the process ends (a private one goes then): the
  // module's engine may still be on it.
}

+ (BOOL)choosesDevices {
  FogataPlayout* current = gCurrent;
  return current != nil && current->_observing;
}

+ (void)inputMayHaveChanged {
  FogataPlayout* current = gCurrent;
  if (current == nil) return;
  dispatch_async(current->_queue, ^{
    [current startOver];
  });
}

+ (void)outputChosen:(NSString*)deviceId name:(nullable NSString*)name {
  [gCurrent choose:deviceId name:name output:YES];
}

+ (void)inputChosen:(NSString*)deviceId name:(nullable NSString*)name {
  [gCurrent choose:deviceId name:name output:NO];
}

+ (void)state:(void (^)(NSDictionary<NSString*, id>* state))answer {
  FogataPlayout* current = gCurrent;
  if (current == nil) {
    answer(@{});
    return;
  }
  dispatch_async(current->_queue, ^{
    answer([current facts]);
  });
}

+ (BOOL)isOwnDevice:(NSString*)deviceId {
  return [deviceId hasPrefix:kOwnPrefix];
}

// On `_queue`: what +state: answers, facts only (see the header).
- (NSDictionary<NSString*, id>*)facts {
  os_unfair_lock_lock(&_lock);
  NSString* chosenOutput = _chosenOutput;
  NSString* chosenOutputName = _chosenOutputName;
  NSString* chosenInput = _chosenInput;
  NSString* chosenInputName = _chosenInputName;
  NSString* outputInUse = _outputInUse;
  NSString* inputInUse = _inputInUse;
  BOOL changing = _pendingChoices > 0;
  os_unfair_lock_unlock(&_lock);
  NSMutableDictionary* facts = [NSMutableDictionary dictionary];
  // What follows the system is told as the system's devices now, by their own ids.
  AudioObjectID output = outputInUse != nil ? DeviceOf(outputInUse) : OutputDevice(nil);
  AudioObjectID input = inputInUse != nil ? DeviceOf(inputInUse) : InputDevice(nil);
  RTCAudioDeviceModule* module = _module;
  // No engine: nothing plays, and nothing is in use.
  BOOL engine = !_stopped && module != nil && module.isEngineRunning;
  if (!engine) output = kAudioObjectUnknown;
  if (chosenOutput != nil) facts[@"chosen"] = chosenOutput;
  if (chosenOutputName != nil) facts[@"chosenName"] = chosenOutputName;
  facts[@"chosenPresent"] = @(IsPresent(chosenOutput, YES));
  if (output != kAudioObjectUnknown) {
    facts[@"inUse"] = UIDOf(output);
    facts[@"inUseName"] = NameOf(output);
  }
  if (chosenInput != nil) facts[@"inputChosen"] = chosenInput;
  if (chosenInputName != nil) facts[@"inputChosenName"] = chosenInputName;
  facts[@"inputChosenPresent"] = @(IsPresent(chosenInput, NO));
  BOOL asked = !_stopped && module != nil && module.isRecording;
  BOOL recording = asked && _inputAvailable && engine;
  // The input in use is a fact only while something records.
  if (recording && input != kAudioObjectUnknown) {
    facts[@"inputInUse"] = UIDOf(input);
    facts[@"inputInUseName"] = NameOf(input);
  }
  // Whether the pair the devices chosen make can be put together (R1 in FOGATA.md), and
  // why not when the rates differ.
  if (chosenOutput != nil || chosenInput != nil) {
    NSString* why = nil;
    facts[@"combinable"] = @([self combinableOutput:chosenOutput input:chosenInput why:&why]);
    if ([why isEqualToString:@"rate"]) facts[@"reason"] = @"rate";
  }
  facts[@"changing"] = @(changing);
  facts[@"recordingAsked"] = @(asked);
  facts[@"recording"] = @(recording);
  facts[@"inputUsable"] = @(_inputAvailable);
  return facts;
}

// Any thread: whether the pair of `chosenOutput` and `chosenInput` (nil: the system's)
// can be put together: from the devices' properties (Combinable), and not refused by the
// engine in this call. `why`: "rate" when the rates differ.
- (BOOL)combinableOutput:(nullable NSString*)chosenOutput
                   input:(nullable NSString*)chosenInput
                     why:(NSString* _Nullable* _Nonnull)why {
  AudioObjectID output = PairOutput(chosenOutput);
  AudioObjectID input = PairInput(chosenInput);
  if (!Combinable(output, input, why)) return NO;
  if (gTestRefuse == 1) {
    *why = @"the test says so";
    return NO;
  }
  NSString* pair = PairOf(output, input);
  NSString* alone = PairOf(output, kAudioObjectUnknown);
  os_unfair_lock_lock(&_lock);
  BOOL refused = [_refused containsObject:pair] || [_refused containsObject:alone];
  os_unfair_lock_unlock(&_lock);
  if (refused) *why = @"the engine refused it in this call";
  return !refused;
}

// Any thread: keeps the device chosen and, on `_queue`, has the module make its engine
// again for it. Choosing a device, the same one again included, always tries it, and
// forgets what the engine refused before; only the system's default chosen when it
// already was changes nothing (a call's start).
- (void)choose:(NSString*)deviceId name:(nullable NSString*)name output:(BOOL)output {
  NSString* chosen = IsDefault(deviceId) ? nil : [deviceId copy];
  os_unfair_lock_lock(&_lock);
  NSString* before = output ? _chosenOutput : _chosenInput;
  if (output) {
    _chosenOutput = chosen;
    _chosenOutputName = chosen != nil ? [name copy] : nil;
  } else {
    _chosenInput = chosen;
    _chosenInputName = chosen != nil ? [name copy] : nil;
  }
  BOOL remake = chosen != nil || before != nil;
  if (remake) {
    _pendingChoices++;
    [_refused removeAllObjects];
  }
  os_unfair_lock_unlock(&_lock);
  dispatch_async(_queue, ^{
    if (remake) {
      [self remakeFor:output chosen:chosen before:before];
      os_unfair_lock_lock(&self->_lock);
      self->_pendingChoices--;
      os_unfair_lock_unlock(&self->_lock);
    }
    [self startOver];
  });
}

// On `_queue`: a device was chosen. The module is told of it only while it has nothing
// enabled, and back to the system's devices at once: it then marks the change for the
// echo canceller (Apple patch 0008, WebRTC patch 0011) and never gives a device to its
// engine itself. Its engine, if it had one, is made again, and given the devices chosen
// in -route:input:. When it does not run with them, the pair is taken as refused and a
// new engine follows the system's devices, the microphone in it.
- (void)remakeFor:(BOOL)output chosen:(nullable NSString*)chosen before:(nullable NSString*)before {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil || !_observing) return;
  [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
  NSString* marked = chosen ?: before;
  if (marked != nil) {
    NSArray<RTCIODevice*>* devices = output ? module.outputDevices : module.inputDevices;
    for (RTCIODevice* device in devices) {
      if (![device.deviceId isEqualToString:marked]) continue;
      if (output) {
        [module trySetOutputDevice:device];
        [module trySetOutputDevice:nil];
      } else {
        [module trySetInputDevice:device];
        [module trySetInputDevice:nil];
      }
      break;
    }
  }
  NSInteger result =
      [module setEngineAvailability:(RTCAudioEngineAvailability){_inputAvailable, YES}];
  BOOL failed = result != 0 || ([self wantsEngine:module] && !module.isEngineRunning);
  NSLog(@"Fogata: the %@ chosen (%@) given to the engine made again (%ld, %@)",
        output ? @"output" : @"input", chosen ?: @"the system's default", (long)result,
        failed ? @"not running" : @"running");
  if (failed && [self refuseGiven]) {
    [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
    result = [module setEngineAvailability:(RTCAudioEngineAvailability){_inputAvailable, YES}];
    NSLog(@"Fogata: the engine made again following the system's devices (%ld, running %d)",
          (long)result, module.isEngineRunning);
  }
}

// On `_queue`, once the module's change that started an engine given `pair` (with the
// microphone) is done: an engine that does not run then refused that pair (R4).
- (void)checkStarted:(NSString*)pair {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil || module.isEngineRunning) return;
  os_unfair_lock_lock(&_lock);
  BOOL still = [pair isEqualToString:_givenPair];
  os_unfair_lock_unlock(&_lock);
  if (!still) return;
  NSLog(@"Fogata: the engine given the pair did not start");
  if ([self refuseGiven]) [self followAnewOnce];
}

// On `_queue`: -followAnew, unless one is asked already.
- (void)followAnewOnce {
  os_unfair_lock_lock(&_lock);
  BOOL ask = !_anewAsked;
  _anewAsked = YES;
  os_unfair_lock_unlock(&_lock);
  if (ask) [self followAnew];
}

// Any thread: the pair given to the engine now is taken as refused for this call (R4), so
// that the next engine follows the system's devices; NO when the engine already did.
- (BOOL)refuseGiven {
  os_unfair_lock_lock(&_lock);
  NSString* pair = _givenPair;
  if (pair != nil) {
    if (_refused == nil) _refused = [NSMutableSet set];
    [_refused addObject:pair];
    _givenPair = nil;
  }
  os_unfair_lock_unlock(&_lock);
  if (pair != nil) NSLog(@"Fogata: the engine refused %@; following the system's devices", pair);
  return pair != nil;
}

// On `_queue`: a new engine, following the system's devices: the module's engine is
// released and made again (availability off, then on), as at a choice. Asked from the
// module's thread when its engine must follow the system but was given a device (only a
// new engine can follow it again); once until it is done.
- (void)followAnew {
  RTCAudioDeviceModule* module = _module;
  if (!_stopped && module != nil) {
    [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
    NSInteger result =
        [module setEngineAvailability:(RTCAudioEngineAvailability){_inputAvailable, YES}];
    NSLog(@"Fogata: a new engine following the system's devices (%ld, running %d)",
          (long)result, module.isEngineRunning);
  }
  // Made: a later need is asked again.
  os_unfair_lock_lock(&_lock);
  _anewAsked = NO;
  os_unfair_lock_unlock(&_lock);
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

// On `_queue`. Once the devices settle, a device chosen that is there again and can be
// put together, while the engine plays or records on another, is given back to it (R6),
// as if chosen again.
- (void)devicesChanged {
  uint64_t change = ++_changes;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kSettleNs), _queue, ^{
    if (change != self->_changes) return;
    [self startOver];
    [self regainChosen];
  });
}

// On `_queue`: see -devicesChanged.
- (void)regainChosen {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil || ![self devicesChosen] || !module.isEngineRunning) return;
  os_unfair_lock_lock(&_lock);
  NSString* chosenOutput = _chosenOutput;
  NSString* chosenInput = _chosenInput;
  NSString* outputInUse = _outputInUse;
  NSString* inputInUse = _inputInUse;
  os_unfair_lock_unlock(&_lock);
  BOOL outputBack = chosenOutput != nil && IsPresent(chosenOutput, YES) &&
                    ![chosenOutput isEqualToString:outputInUse];
  BOOL inputBack = chosenInput != nil && IsPresent(chosenInput, NO) &&
                   ![chosenInput isEqualToString:inputInUse] && module.isRecording;
  NSString* why = nil;
  if (!(outputBack || inputBack) ||
      ![self combinableOutput:chosenOutput input:chosenInput why:&why]) {
    return;
  }
  NSLog(@"Fogata: the %@ chosen is there again: given back to the engine",
        outputBack ? @"output" : @"input");
  os_unfair_lock_lock(&_lock);
  _pendingChoices++;
  os_unfair_lock_unlock(&_lock);
  [self remakeFor:outputBack chosen:outputBack ? chosenOutput : chosenInput before:nil];
  os_unfair_lock_lock(&_lock);
  _pendingChoices--;
  os_unfair_lock_unlock(&_lock);
}
// Any thread: whether a device is chosen (given to the engine here).
- (BOOL)devicesChosen {
  if (!_observing) return NO;
  os_unfair_lock_lock(&_lock);
  BOOL chosen = _chosenOutput != nil || _chosenInput != nil;
  os_unfair_lock_unlock(&_lock);
  return chosen;
}

// On `_queue`: tells the module whether its input can be used, then looks at the engine.
- (void)refresh {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil) return;
  NSString* chosenInput = nil;
  if (_observing) {
    os_unfair_lock_lock(&_lock);
    chosenInput = _chosenInput;
    os_unfair_lock_unlock(&_lock);
  } else {
    // The module chooses its input itself then.
    chosenInput = module.inputDevice.deviceId;
  }
  NSString* why = WhyNoInput(chosenInput);
  if (why == nil && _inputFailed) why = @"it could not be opened";
  BOOL available = why == nil;
  if (available != _inputAvailable) {
    // With WebRTC recording already, making the input available opens it here.
    NSInteger result = [module setEngineAvailability:(RTCAudioEngineAvailability){available, YES}];
    // Not opened with devices chosen: the pair given is taken as refused, and a new
    // engine follows the system's devices, the microphone in it (R4).
    if (result != 0 && available && [self devicesChosen] && [self refuseGiven]) {
      [module setEngineAvailability:(RTCAudioEngineAvailability){NO, NO}];
      result = [module setEngineAvailability:(RTCAudioEngineAvailability){YES, YES}];
    }
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

// On `_queue`: whether the module, as it counts itself, should have an engine running: it
// plays; with devices chosen, also when it only records (a call where nobody else sends
// sound) with its input available.
- (BOOL)wantsEngine:(RTCAudioDeviceModule*)module {
  if (module.isPlaying) return YES;
  return [self devicesChosen] && _inputAvailable && module.isRecording;
}

// On `_queue`: an engine stopped while the module counts itself as playing (or, with
// devices chosen, recording) is made again. Looked at twice, as the system stops it for a
// moment by itself. The first time it is made as it was, the input in it if it was: an
// engine stops with its input in it as well when the system switches devices, and the
// microphone has no part in that. When that one stops too, the devices chosen are left
// out before the microphone is; the microphone is left out only when an engine on the
// system's devices does not run either, and is then tried again by itself
// (-retryInputLater).
- (void)lookAtEngine {
  RTCAudioDeviceModule* module = _module;
  if (_stopped || module == nil || _checking) return;
  if (![self wantsEngine:module] || module.isEngineRunning) {
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
    if (self->_stopped || module == nil || ![self wantsEngine:module] ||
        module.isEngineRunning) {
      return;
    }
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
    BOOL again = self->_recoveries > 0;
    // An engine made again with the devices chosen that stopped again refused them (R4):
    // they go before the microphone does. A single stop is not taken for a refusal (a
    // device unplugged, the system switching devices stop an engine too).
    BOOL devicesOut = again && [self devicesChosen] && [self refuseGiven];
    BOOL leftOut = self->_inputAvailable && inputsFault && again && !devicesOut;
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
          leftOut      ? @"without the microphone"
          : devicesOut ? @"without the devices chosen"
                       : @"as it was",
          (long)result, running);
    if (result != 0 || !running) {
      // Not running: looked at again, to be made the next way.
      [self lookAtEngine];
    } else if (leftOut) {
      [self retryInputLater];
    }
  });
}

// On the module's thread: gives `engine`'s unit `device`, which plays on `output` (after
// `skipped` output channels of another device, mapped away) and records from `input`
// (kAudioObjectUnknown: it does not record). YES when the unit has it and the engine reads
// an output, and an input when it records, with channels and a rate from it: what the
// module checks next ("Output device not available", "Input device not available"). Else
// the unit is given back the device it had.
- (BOOL)giveEngine:(AVAudioEngine*)engine
              unit:(AudioUnit)unit
            device:(AudioObjectID)device
            output:(AudioObjectID)output
             input:(AudioObjectID)input
           skipped:(UInt32)skipped {
  AudioObjectID current = kAudioObjectUnknown;
  UInt32 size = sizeof(current);
  if (AudioUnitGetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global,
                           0, &current, &size) != noErr) {
    current = kAudioObjectUnknown;
  }
  if (current == device && device == _given) return YES;
  OSStatus status = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice,
                                         kAudioUnitScope_Global, 0, &device, sizeof(device));
  if (status != noErr) {
    NSLog(@"Fogata: the engine refused the device %@ (%d)", NameOf(device), (int)status);
    return NO;
  }
  // From here the engine no longer follows the system's devices by itself, even if this
  // device is given back: what it plays and records on is given here from now on.
  _engineRouted = YES;
  AVAudioFormat* played = [engine.outputNode outputFormatForBus:0];
  BOOL playable = played.channelCount > 0 && played.sampleRate > 0;
  // Read only when the output is, and the engine records: the module reads it next
  // itself. The microphone's permission was granted (WhyNoInput), and nothing records
  // before the engine starts.
  AVAudioFormat* recorded = playable && input != kAudioObjectUnknown
                                ? [engine.inputNode outputFormatForBus:0]
                                : nil;
  BOOL recordable =
      input == kAudioObjectUnknown || (recorded.channelCount > 0 && recorded.sampleRate > 0);
  // The channel map only once the device is read (R7): one refused leaves none behind.
  BOOL mapped = YES;
  if (playable && recordable && skipped > 0) {
    // The input's device plays too and comes first: the engine's sound goes to the
    // output's first two channels alone. Not mapped, it would play on the microphone's
    // device instead: refused then.
    UInt32 channels = OutputChannels(device);
    UInt32 first = MIN(OutputChannels(output), (UInt32)2);
    mapped = channels > 0 && channels <= 64;
    if (mapped) {
      SInt32 map[64];
      for (UInt32 i = 0; i < channels; i++) {
        map[i] = i >= skipped && i < skipped + first ? 0 : -1;
      }
      mapped = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_ChannelMap,
                                    kAudioUnitScope_Input, 0, map,
                                    channels * (UInt32)sizeof(SInt32)) == noErr;
    }
    if (!mapped) NSLog(@"Fogata: the channels of the device of both were not mapped");
  }
  if (playable && recordable && mapped) {
    _given = device;
    return YES;
  }
  NSLog(@"Fogata: the engine reads from %@ an output of %u channels at %.0f Hz%@",
        NameOf(device), (unsigned)played.channelCount, played.sampleRate,
        recorded != nil
            ? [NSString stringWithFormat:@" and an input of %u channels at %.0f Hz",
                                         (unsigned)recorded.channelCount, recorded.sampleRate]
            : @"");
  // The one it had, as far as it takes it back; the engine is made anew after a refusal
  // anyway (-route:input:).
  if (current != kAudioObjectUnknown) {
    AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0,
                         &current, sizeof(current));
  }
  _given = kAudioObjectUnknown;
  return NO;
}

// On the module's thread: gives `engine`'s unit what plays on `output` and records from
// `input` (kAudioObjectUnknown: it does not record): the output alone, the one device when
// it plays and records on it, or an aggregate of both: the one made before for the same
// two, else one made now, the input's device first and then the output's (MakeAggregate).
// The device given, or kAudioObjectUnknown with why.
- (AudioObjectID)giveEngine:(AVAudioEngine*)engine
                       unit:(AudioUnit)unit
                     output:(AudioObjectID)output
                      input:(AudioObjectID)input
                        why:(NSString* _Nullable* _Nonnull)why {
  *why = nil;
  if (gTestRefuse == 2 && input != kAudioObjectUnknown) {
    // For the call test (FOGATA_TEST_REFUSE=unmute): this pair, in either order, refused.
    gTestRefuse = 0;
    NSLog(@"Fogata: test: the pair %@ and %@ taken as refused", NameOf(output), NameOf(input));
    *why = @"the test refused it";
    return kAudioObjectUnknown;
  }
  if (input == kAudioObjectUnknown || input == output) {
    if ([self giveEngine:engine unit:unit device:output output:output input:input skipped:0]) {
      return output;
    }
    *why = @"the engine reads no sound from it";
    return kAudioObjectUnknown;
  }
  NSString* of = [NSString stringWithFormat:@"%@|%@", UIDOf(output), UIDOf(input)];
  if (_aggregate != kAudioObjectUnknown && [of isEqualToString:_aggregateOf] &&
      IsAlive(_aggregate) &&
      [self giveEngine:engine
                  unit:unit
                device:_aggregate
                output:output
                 input:input
               skipped:_aggregateSkipped]) {
    return _aggregate;
  }
  for (int inputFirst = 1; inputFirst >= 0; inputFirst--) {
    NSString* refused = nil;
    AudioObjectID made = MakeAggregate(output, input, inputFirst, &refused);
    if (made == kAudioObjectUnknown) {
      if (*why == nil) *why = refused;
      continue;
    }
    UInt32 skipped = inputFirst ? OutputChannels(input) : 0;
    if ([self giveEngine:engine unit:unit device:made output:output input:input skipped:skipped]) {
      // The unit is off the one made before (the engine is stopped here).
      [self dropAggregateUnless:kAudioObjectUnknown];
      _aggregate = made;
      _aggregateOf = of;
      _aggregateSkipped = skipped;
      NSLog(@"Fogata: a device of both made, the %@ first", inputFirst ? @"input" : @"output");
      return made;
    }
    AudioHardwareDestroyAggregateDevice(made);
    if (*why == nil) *why = @"the engine reads no sound from a device of both";
  }
  return kAudioObjectUnknown;
}

// On the module's thread, inside its change, before the module reads the engine's devices
// (`input`: the engine is to record) and again before the engine starts: gives the engine's
// unit the devices chosen (FOGATA.md, "The devices chosen, with the microphone", R1 to R4).
// When the pair they make can be put together, the output alone while the engine does
// not record, the pair while it does; when it cannot, or nothing is chosen, nothing is
// given and the engine follows the system's devices by itself, as with nothing chosen. A
// pair the engine refuses here is taken as refused for this call, and a new engine
// follows the system; so does one when an engine given a device must follow it again.
- (void)route:(AVAudioEngine*)engine input:(BOOL)input {
  os_unfair_lock_lock(&_lock);
  NSString* chosenOutput = _chosenOutput;
  NSString* chosenInput = _chosenInput;
  os_unfair_lock_unlock(&_lock);
  NSString* why = nil;
  BOOL chosen = chosenOutput != nil || chosenInput != nil;
  BOOL combinable = chosen && [self combinableOutput:chosenOutput input:chosenInput why:&why];
  AudioObjectID output = PairOutput(chosenOutput);
  AudioObjectID inputDevice = input ? PairInput(chosenInput) : kAudioObjectUnknown;
  // With only a microphone chosen, an engine that does not record has nothing of it.
  BOOL applies =
      combinable && (input || (chosenOutput != nil && IsPresent(chosenOutput, YES)));
  if (!applies) {
    if (chosen && !combinable) {
      NSLog(@"Fogata: the devices chosen cannot be put together (%@): the engine follows the "
            @"system's devices",
            why);
    }
    [self follow];
    return;
  }
  AudioUnit unit = engine.outputNode.audioUnit;
  if (unit == NULL) return;
  NSString* refused = nil;
  AudioObjectID device =
      [self giveEngine:engine unit:unit output:output input:inputDevice why:&refused];
  NSString* pair = PairOf(output, inputDevice);
  if (device == kAudioObjectUnknown) {
    // R4: refused at run time: not for this call any more, and a new engine follows.
    NSLog(@"Fogata: the output %@ cannot be used with the microphone %@: %@", NameOf(output),
          inputDevice != kAudioObjectUnknown ? NameOf(inputDevice) : @"(none)",
          refused ?: @"the engine refused it");
    os_unfair_lock_lock(&_lock);
    if (_refused == nil) _refused = [NSMutableSet set];
    [_refused addObject:pair];
    os_unfair_lock_unlock(&_lock);
    [self follow];
    return;
  }
  _engineRouted = YES;
  // The aggregate of the pair chosen stays while the engine only plays (R3 in FOGATA.md):
  // a microphone turned on uses it again, with no device made or gone.
  NSString* key = [self chosenPairKey];
  [self keepAggregateFor:key];
  if (inputDevice == kAudioObjectUnknown) [self makeAggregateFor:key output:output];
  NSLog(@"Fogata: playing on %@%@%@", NameOf(output),
        inputDevice != kAudioObjectUnknown
            ? [NSString stringWithFormat:@", recording from %@", NameOf(inputDevice)]
            : @"",
        device != output ? @", through a device of both" : @"");
  os_unfair_lock_lock(&_lock);
  _givenPair = pair;
  _outputInUse = UIDOf(output);
  _inputInUse = inputDevice != kAudioObjectUnknown ? UIDOf(inputDevice) : nil;
  os_unfair_lock_unlock(&_lock);
}

// On the module's thread: the engine is to follow the system's devices. One never given a
// device does by itself; one that was given a device cannot be put back to following, so
// a new engine is asked for (-followAnew), once until it is made.
- (void)follow {
  os_unfair_lock_lock(&_lock);
  _givenPair = nil;
  _outputInUse = nil;
  _inputInUse = nil;
  BOOL ask = _engineRouted && !_anewAsked;
  if (ask) _anewAsked = YES;
  os_unfair_lock_unlock(&_lock);
  // An aggregate kept for a pair that cannot be used now goes (an engine following the
  // system has none); one for the pair chosen stays (-keepAggregateFor:).
  if (!_engineRouted) [self keepAggregateFor:[self chosenPairKey]];
  if (ask) {
    dispatch_async(_queue, ^{
      [self followAnew];
    });
  }
}

// On the module's thread: "<output uid>|<input uid>" of the pair the devices chosen make
// (the microphone the call uses, recording or not), when it can be put together; else nil.
- (nullable NSString*)chosenPairKey {
  os_unfair_lock_lock(&_lock);
  NSString* chosenOutput = _chosenOutput;
  NSString* chosenInput = _chosenInput;
  os_unfair_lock_unlock(&_lock);
  NSString* why = nil;
  if ((chosenOutput == nil && chosenInput == nil) ||
      ![self combinableOutput:chosenOutput input:chosenInput why:&why]) {
    return nil;
  }
  return PairOf(PairOutput(chosenOutput), PairInput(chosenInput));
}

// On the module's thread: the aggregate made goes unless it is of `pair`: it is kept, with
// no engine on it, while the pair chosen stays the same and can be used, so that turning
// the microphone off and on makes and destroys no device (each would tell WebRTC the
// devices changed, and its echo canceller would be made anew at every unmute). An
// aggregate with no engine on it runs no device: the HAL starts its devices only for IO.
- (void)keepAggregateFor:(nullable NSString*)pair {
  if (_aggregate == kAudioObjectUnknown || [pair isEqualToString:_aggregateOf]) return;
  [self dropAggregateUnless:kAudioObjectUnknown];
}

// On the module's thread, while the engine only plays on `output`: the aggregate of the
// pair chosen (`key`) is made now, with no engine on it, if it is not there yet. Made at
// an unmute instead, the HAL would tell of it only after WebRTC read its count of device
// changes for that unmute, and the next unmute would make the echo canceller anew once
// more. The engine reads it then; if it reads nothing from it, both orders are made anew
// as before.
- (void)makeAggregateFor:(nullable NSString*)key output:(AudioObjectID)output {
  if (key == nil || _aggregate != kAudioObjectUnknown) return;
  // Without the permission the microphone's device is not touched: once it is granted, the
  // first unmute makes the aggregate as before.
  if ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] !=
      AVAuthorizationStatusAuthorized) {
    return;
  }
  os_unfair_lock_lock(&_lock);
  NSString* chosenInput = _chosenInput;
  os_unfair_lock_unlock(&_lock);
  AudioObjectID input = PairInput(chosenInput);
  if (input == kAudioObjectUnknown || input == output) return;
  NSString* why = nil;
  AudioObjectID made = MakeAggregate(output, input, YES, &why);
  if (made == kAudioObjectUnknown) return;
  _aggregate = made;
  _aggregateOf = key;
  _aggregateSkipped = OutputChannels(input);
  NSLog(@"Fogata: a device of both made ahead of the microphone, the input first");
}

// On the module's thread: the aggregate made before goes unless it is `kept` (the engine's
// unit is on another device, or the engine it was on is gone).
- (void)dropAggregateUnless:(AudioObjectID)kept {
  if (_aggregate == kAudioObjectUnknown || _aggregate == kept) return;
  AudioHardwareDestroyAggregateDevice(_aggregate);
  _aggregate = kAudioObjectUnknown;
  _aggregateOf = nil;
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

// On the module's thread, inside its change: a new engine follows the system's default
// devices until it is given one.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               didCreateEngine:(AVAudioEngine*)engine {
  _engineRouted = NO;
  _given = kAudioObjectUnknown;
  return 0;
}

// On the module's thread, inside its change, when it enables the output or the input,
// before it reads their formats from the engine: the input it reads must be on the device
// the engine's unit records from.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
              willEnableEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled
      isVoiceProcessingEnabled:(BOOL)isVoiceProcessingEnabled {
  if (_observing && isPlayoutEnabled) [self route:engine input:isRecordingEnabled];
  return 0;
}

// On the module's thread, inside its change.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
               willStartEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
  if (_observing && isPlayoutEnabled) [self route:engine input:isRecordingEnabled];
  if (!_observing || !isRecordingEnabled) return 0;
  os_unfair_lock_lock(&_lock);
  NSString* pair = _givenPair;
  os_unfair_lock_unlock(&_lock);
  if (pair == nil) return 0;
  // An engine given the pair that then does not start (R4): looked at once this change is
  // done; the module waits for nothing of it.
  dispatch_async(_queue, ^{
    [self checkStarted:pair];
  });
  if (gTestRefuse == 3) {
    // For the call test (FOGATA_TEST_REFUSE=start): this start fails, once, the formats
    // having been read: the module undoes its change, as when the engine does not start.
    gTestRefuse = 0;
    NSLog(@"Fogata: test: the engine's start with the pair fails");
    return -1;
  }
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
              didDisableEngine:(AVAudioEngine*)engine
              isPlayoutEnabled:(BOOL)isPlayoutEnabled
            isRecordingEnabled:(BOOL)isRecordingEnabled {
  return 0;
}

// On the module's thread: the engine goes, and what it was given with it.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
             willReleaseEngine:(AVAudioEngine*)engine {
  os_unfair_lock_lock(&_lock);
  _givenPair = nil;
  _outputInUse = nil;
  _inputInUse = nil;
  os_unfair_lock_unlock(&_lock);
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
