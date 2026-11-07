#import "FogataScreenSound.h"

#import <AVFAudio/AVFAudio.h>
#import <CoreMedia/CoreMedia.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <os/lock.h>

// What the factory's module sends: 48 kHz, one channel.
static const double kRate = 48000;

// The sound waits here between capture and sending. At most 150 ms of it, so a pause on
// either side never builds up delay.
enum { kCapacity = 48000, kMostWaiting = 7200 };

typedef struct {
  os_unfair_lock lock;
  float samples[kCapacity];
  size_t first;
  size_t count;
} Waiting;

// All zeros: empty, and its lock open.
static Waiting waiting;

static void Keep(const float* samples, size_t count) {
  os_unfair_lock_lock(&waiting.lock);
  for (size_t i = 0; i < count; i++) {
    waiting.samples[(waiting.first + waiting.count) % kCapacity] = samples[i];
    if (waiting.count < kCapacity) {
      waiting.count++;
    } else {
      waiting.first = (waiting.first + 1) % kCapacity;
    }
  }
  if (waiting.count > kMostWaiting) {
    waiting.first = (waiting.first + waiting.count - kMostWaiting) % kCapacity;
    waiting.count = kMostWaiting;
  }
  os_unfair_lock_unlock(&waiting.lock);
}

// As much as is waiting, then silence.
static void Take(float* out, size_t count) {
  os_unfair_lock_lock(&waiting.lock);
  const size_t have = MIN(count, waiting.count);
  for (size_t i = 0; i < have; i++) {
    out[i] = waiting.samples[(waiting.first + i) % kCapacity];
  }
  waiting.first = (waiting.first + have) % kCapacity;
  waiting.count -= have;
  os_unfair_lock_unlock(&waiting.lock);
  for (size_t i = have; i < count; i++) out[i] = 0;
}

// The sound at 48 kHz, from whatever rate it comes in (it is asked for at 48 kHz, so
// usually it is only copied), drawing straight lines between the samples. Returns how
// many samples it made; on the capture queue only.
static size_t To48k(const float* in, size_t count, double rate, float* out) {
  // Where the next sample falls, counted from this block's first one, and the last
  // sample of the previous block.
  static double next = 0;
  static float last = 0;
  if (rate == kRate) {
    memcpy(out, in, count * sizeof(float));
    return count;
  }
  const double step = rate / kRate;
  size_t made = 0;
  while (next <= (double)count - 1) {
    const long i = (long)floor(next);
    const float a = i < 0 ? last : in[i];
    const float b = i + 1 < (long)count ? in[i + 1] : a;
    out[made++] = a + (float)((b - a) * (next - i));
    next += step;
  }
  next -= count;
  last = in[count - 1];
  return made;
}

// Whether two targets (a window's number, or nil for everything) are the same.
static BOOL SameTarget(NSString* a, NSString* b) {
  return a == b || [a isEqualToString:b];
}

// The applications whose sound goes with a window of `owner`: itself and its helpers,
// named under its own bundle id (com.google.Chrome.helper..., Electron's ...helper), and for
// Safari WebKit's processes, which play its pages' sound (they play for every application
// that shows web pages: those would be heard too while they play).
API_AVAILABLE(macos(13.0))
static NSArray<SCRunningApplication*>* SoundOf(SCRunningApplication* owner,
                                               NSArray<SCRunningApplication*>* all) {
  NSString* bundle = owner.bundleIdentifier;
  const BOOL safari = [bundle isEqualToString:@"com.apple.Safari"] ||
                      [bundle hasPrefix:@"com.apple.SafariTechnologyPreview"];
  NSMutableArray<SCRunningApplication*>* apps = [NSMutableArray arrayWithObject:owner];
  for (SCRunningApplication* app in all) {
    NSString* other = app.bundleIdentifier;
    if (app.processID == owner.processID || other.length == 0) continue;
    if ((bundle.length > 0 && [other hasPrefix:[bundle stringByAppendingString:@"."]]) ||
        (safari && [other hasPrefix:@"com.apple.WebKit."])) {
      [apps addObject:app];
    }
  }
  return apps;
}

// What a capture took, for its last line in the log: blocks of sound and their loudest
// sample. On the capture queue only.
static size_t takenBlocks = 0;
static float takenPeak = 0;

static void Forget(void) {
  os_unfair_lock_lock(&waiting.lock);
  waiting.first = 0;
  waiting.count = 0;
  os_unfair_lock_unlock(&waiting.lock);
}

@interface FogataScreenSound () <SCStreamOutput, SCStreamDelegate, RTCAudioDeviceModuleDelegate>
@end

@implementation FogataScreenSound {
  dispatch_queue_t _queue;
  SCStream* _stream;
  NSMutableArray<void (^)(BOOL)>* _starting;
  // Stopped while a start was under way: the capture stops as soon as it has started.
  BOOL _stopWhenStarted;
  // The window the capture running, or being started, takes the sound of; nil for all.
  NSString* _window;
  NSString* _startingWindow;
  // Asked for another window while a start was under way: started once that one ends.
  NSString* _nextWindow;
  NSMutableArray<void (^)(BOOL)>* _next;
  RTCAudioTrack* _track;
  BOOL _manual;
}

+ (instancetype)shared {
  static FogataScreenSound* shared;
  static dispatch_once_t once;
  dispatch_once(&once, ^{
    shared = [[FogataScreenSound alloc] init];
  });
  return shared;
}

- (instancetype)init {
  self = [super init];
  if (self) {
    _queue = dispatch_queue_create("fogata.screen-sound", DISPATCH_QUEUE_SERIAL);
    // A screen's sound goes as it is: no echo cancelling, noise removal or gain.
    RTCAudioProcessingConfig* config = [[RTCAudioProcessingConfig alloc] init];
    config.isEchoCancellationEnabled = NO;
    config.isNoiseSuppressionEnabled = NO;
    config.isHighpassFilterEnabled = NO;
    config.isAutoGainControl1Enabled = NO;
    config.isAutoGainControl2Enabled = NO;
    RTCDefaultAudioProcessingModule* processing =
        [[RTCDefaultAudioProcessingModule alloc] initWithConfig:config
                                  capturePostProcessingDelegate:nil
                                    renderPreProcessingDelegate:nil];
    _factory = [[RTCPeerConnectionFactory alloc]
        initWithAudioDeviceModuleType:RTCAudioDeviceModuleTypeAudioEngine
                bypassVoiceProcessing:YES
                       encoderFactory:[[RTCDefaultVideoEncoderFactory alloc] init]
                       decoderFactory:[[RTCDefaultVideoDecoderFactory alloc] init]
                audioProcessingModule:processing];
    // Rendered by the module itself every 10 ms, without any device: what it records is
    // only what this object connects to its engine.
    // Otherwise it would record from the microphone: then nothing is sent at all.
    _manual = [_factory.audioDeviceModule setManualRenderingMode:YES] == 0 &&
              _factory.audioDeviceModule.manualRenderingMode;
    if (!_manual) NSLog(@"FogataScreenSound: no manual rendering");
    _factory.audioDeviceModule.observer = self;
  }
  return self;
}

- (void)startWithCompletion:(void (^)(BOOL started))completion {
  [self startForWindow:nil completion:completion];
}

- (void)startForWindow:(NSString*)window completion:(void (^)(BOOL started))completion {
  dispatch_async(dispatch_get_main_queue(), ^{
    if (!self->_manual) {
      completion(NO);
      return;
    }
    if (self->_starting != nil) {
      if (SameTarget(self->_startingWindow, window)) {
        // Asked again after a stop: the start under way stands.
        self->_stopWhenStarted = NO;
        [self->_starting addObject:completion];
      } else {
        // Another window: the start under way is let go, and this one follows it.
        self->_stopWhenStarted = YES;
        for (void (^waiting)(BOOL) in self->_next) waiting(NO);
        self->_nextWindow = window;
        self->_next = [NSMutableArray arrayWithObject:completion];
      }
      return;
    }
    if (self->_stream != nil) {
      if (SameTarget(self->_window, window)) {
        completion(YES);
        return;
      }
      SCStream* old = self->_stream;
      self->_stream = nil;
      [old stopCaptureWithCompletionHandler:^(NSError* error) {
        Forget();
        [self tellTaken];
      }];
    }
    self->_starting = [NSMutableArray arrayWithObject:completion];
    self->_startingWindow = window;
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent* content,
                                                                   NSError* error) {
      SCDisplay* display = content.displays.firstObject;
      if (error != nil || display == nil) {
        NSLog(@"FogataScreenSound: no display: %@", error);
        [self started:nil];
        return;
      }
      // The picture is not used. Sound is captured per application, so the filter names
      // applications: one made of windows brings no sound at all.
      SCContentFilter* filter = nil;
      if (window == nil) {
        // The whole computer's sound but this application's (none left out here).
        filter = [[SCContentFilter alloc] initWithDisplay:display
                                    excludingApplications:@[]
                                         exceptingWindows:@[]];
      } else {
        SCRunningApplication* owner = nil;
        for (SCWindow* candidate in content.windows) {
          if ([[NSString stringWithFormat:@"%u", candidate.windowID] isEqualToString:window]) {
            owner = candidate.owningApplication;
            break;
          }
        }
        if (owner == nil) {
          // Gone, or no application's: nothing is taken rather than everything.
          NSLog(@"FogataScreenSound: no application for window %@", window);
          [self started:nil];
          return;
        }
        NSArray<SCRunningApplication*>* apps = SoundOf(owner, content.applications);
        NSMutableArray<NSString*>* names = [NSMutableArray array];
        for (SCRunningApplication* app in apps) {
          [names addObject:app.bundleIdentifier.length > 0
                               ? app.bundleIdentifier
                               : [NSString stringWithFormat:@"%@ (%d)", app.applicationName,
                                                            app.processID]];
        }
        NSLog(@"FogataScreenSound: sound of window %@: %@", window,
              [names componentsJoinedByString:@", "]);
        filter = [[SCContentFilter alloc] initWithDisplay:display
                                    includingApplications:apps
                                         exceptingWindows:@[]];
      }
      SCStreamConfiguration* config = [[SCStreamConfiguration alloc] init];
      config.width = 2;
      config.height = 2;
      config.minimumFrameInterval = CMTimeMake(1, 1);
      config.capturesAudio = YES;
      config.excludesCurrentProcessAudio = YES;
      config.sampleRate = (NSInteger)kRate;
      // Two channels, as the system gives them; made one here.
      config.channelCount = 2;
      SCStream* stream = [[SCStream alloc] initWithFilter:filter
                                            configuration:config
                                                 delegate:self];
      NSError* outputError = nil;
      [stream addStreamOutput:self
                         type:SCStreamOutputTypeAudio
           sampleHandlerQueue:self->_queue
                        error:&outputError];
      // Somewhere for the picture to go, which the stream wants.
      if (outputError == nil) {
        [stream addStreamOutput:self
                           type:SCStreamOutputTypeScreen
             sampleHandlerQueue:self->_queue
                          error:&outputError];
      }
      if (outputError != nil) {
        NSLog(@"FogataScreenSound: %@", outputError);
        [self started:nil];
        return;
      }
      [stream startCaptureWithCompletionHandler:^(NSError* startError) {
        if (startError != nil) NSLog(@"FogataScreenSound: %@", startError);
        [self started:startError == nil ? stream : nil];
      }];
    }];
  });
}

- (void)started:(SCStream*)stream {
  dispatch_async(dispatch_get_main_queue(), ^{
    NSArray<void (^)(BOOL)>* waiting = self->_starting;
    self->_starting = nil;
    const BOOL stopped = self->_stopWhenStarted;
    self->_stopWhenStarted = NO;
    if (stopped && stream != nil) {
      // Nobody wants the sound any more: it is not captured, let alone sent.
      [stream stopCaptureWithCompletionHandler:^(NSError* error) {
        Forget();
      }];
    } else {
      self->_stream = stream;
      self->_window = self->_startingWindow;
    }
    for (void (^completion)(BOOL) in waiting) completion(stream != nil && !stopped);
    // Another window was asked for meanwhile.
    NSArray<void (^)(BOOL)>* next = self->_next;
    NSString* nextWindow = self->_nextWindow;
    self->_next = nil;
    self->_nextWindow = nil;
    for (void (^completion)(BOOL) in next) {
      [self startForWindow:nextWindow completion:completion];
    }
  });
}

- (void)stop {
  dispatch_async(dispatch_get_main_queue(), ^{
    if (self->_starting != nil) self->_stopWhenStarted = YES;
    for (void (^waiting)(BOOL) in self->_next) waiting(NO);
    self->_next = nil;
    self->_nextWindow = nil;
    SCStream* stream = self->_stream;
    self->_stream = nil;
    [stream stopCaptureWithCompletionHandler:^(NSError* error) {
      Forget();
      [self tellTaken];
    }];
  });
}

- (RTCRtpTransceiver*)sendOn:(RTCPeerConnection*)connection {
  if (!_manual) return nil;
  if (_track == nil) {
    NSDictionary* plain = @{
      @"googEchoCancellation" : @"false",
      @"googAutoGainControl" : @"false",
      @"googNoiseSuppression" : @"false",
      @"googHighpassFilter" : @"false",
    };
    RTCAudioSource* source = [_factory
        audioSourceWithConstraints:[[RTCMediaConstraints alloc] initWithMandatoryConstraints:plain
                                                                          optionalConstraints:nil]];
    _track = [_factory audioTrackWithSource:source trackId:@"fogata-screen-sound"];
  }
  RTCRtpTransceiverInit* init = [[RTCRtpTransceiverInit alloc] init];
  init.direction = RTCRtpTransceiverDirectionSendOnly;
  init.streamIds = @[ @"fogata-screen-sound" ];
  return [connection addTransceiverWithTrack:_track init:init];
}

#pragma mark - SCStreamOutput, SCStreamDelegate

- (void)stream:(SCStream*)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
                   ofType:(SCStreamOutputType)type {
  if (type != SCStreamOutputTypeAudio) return;
  const AudioStreamBasicDescription* format = CMAudioFormatDescriptionGetStreamBasicDescription(
      CMSampleBufferGetFormatDescription(sampleBuffer));
  const BOOL usable = CMSampleBufferDataIsReady(sampleBuffer) && format != NULL &&
                      format->mFormatID == kAudioFormatLinearPCM &&
                      (format->mFormatFlags & kAudioFormatFlagIsFloat) &&
                      format->mBitsPerChannel == 32 && format->mChannelsPerFrame > 0 &&
                      format->mSampleRate > 0;
  // The list of buffers is as long as the block needs: asked first, then filled.
  const uint32_t flags = kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment;
  size_t size = 0;
  OSStatus err = usable ? CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
                              sampleBuffer, &size, NULL, 0, NULL, NULL, flags, NULL)
                        : kAudio_ParamError;
  AudioBufferList* list = err == noErr && size > 0 ? malloc(size) : NULL;
  CMBlockBufferRef block = NULL;
  if (list != NULL) {
    err = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, NULL, list, size,
                                                                  NULL, NULL, flags, &block);
  } else if (err == noErr) {
    err = kAudio_MemFullError;
  }
  if (err != noErr) {
    // A few are told, should they ever come in a form not taken.
    static int told = 0;
    if (told++ < 3) {
      NSLog(@"FogataScreenSound: a block refused, error %d: %.0f Hz, flags %u, %u bits, "
            @"%u channels",
            (int)err, format ? format->mSampleRate : 0, format ? format->mFormatFlags : 0,
            format ? format->mBitsPerChannel : 0, format ? format->mChannelsPerFrame : 0);
    }
    free(list);
    if (block != NULL) CFRelease(block);
    return;
  }
  const size_t frames = (size_t)CMSampleBufferGetNumSamples(sampleBuffer);
  const BOOL apart = (format->mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0;
  const size_t channels =
      apart ? MIN(format->mChannelsPerFrame, list->mNumberBuffers) : format->mChannelsPerFrame;
  // Every channel mixed into one, at 48 kHz; on this queue only, so the room for it can
  // be shared.
  static float mono[1024];
  static float even[8192];
  for (size_t done = 0; done < frames;) {
    const size_t n = MIN(frames - done, (size_t)1024);
    for (size_t i = 0; i < n; i++) {
      float sum = 0;
      for (size_t c = 0; c < channels; c++) {
        sum += apart ? ((const float*)list->mBuffers[c].mData)[done + i]
                     : ((const float*)list->mBuffers[0].mData)[(done + i) * channels + c];
      }
      mono[i] = sum / channels;
      takenPeak = MAX(takenPeak, fabsf(mono[i]));
    }
    Keep(even, To48k(mono, n, MAX(format->mSampleRate, 8000), even));
    done += n;
  }
  takenBlocks++;
  free(list);
  if (block != NULL) CFRelease(block);
}

// Says what the capture that stopped took, and starts the count again.
- (void)tellTaken {
  dispatch_async(_queue, ^{
    NSLog(@"FogataScreenSound: stopped after %zu blocks of sound, loudest %.4f", takenBlocks,
          takenPeak);
    takenBlocks = 0;
    takenPeak = 0;
  });
}

- (void)stream:(SCStream*)stream didStopWithError:(NSError*)error {
  NSLog(@"FogataScreenSound: stopped: %@", error);
  dispatch_async(dispatch_get_main_queue(), ^{
    if (self->_stream == stream) self->_stream = nil;
  });
}

#pragma mark - RTCAudioDeviceModuleDelegate

// The engine is made again each time sending starts, so each one gets a node of its own.
- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
                        engine:(AVAudioEngine*)engine
      configureInputFromSource:(AVAudioNode*)source
                 toDestination:(AVAudioNode*)destination
                    withFormat:(AVAudioFormat*)format
                       context:(NSDictionary*)context {
  AVAudioFormat* mono = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:kRate channels:1];
  AVAudioSourceNode* node = [[AVAudioSourceNode alloc]
      initWithFormat:mono
         renderBlock:^OSStatus(BOOL* isSilence, const AudioTimeStamp* timestamp,
                               AVAudioFrameCount frameCount, AudioBufferList* output) {
           Take((float*)output->mBuffers[0].mData, frameCount);
           return noErr;
         }];
  [engine attachNode:node];
  [engine connect:node to:destination format:mono];
  return 0;
}

- (NSInteger)audioDeviceModule:(RTCAudioDeviceModule*)audioDeviceModule
                        engine:(AVAudioEngine*)engine
     configureOutputFromSource:(AVAudioNode*)source
                 toDestination:(AVAudioNode*)destination
                    withFormat:(AVAudioFormat*)format
                       context:(NSDictionary*)context {
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
                 didStopEngine:(AVAudioEngine*)engine
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

- (void)audioDeviceModuleDidUpdateDevices:(RTCAudioDeviceModule*)audioDeviceModule {
}

@end
