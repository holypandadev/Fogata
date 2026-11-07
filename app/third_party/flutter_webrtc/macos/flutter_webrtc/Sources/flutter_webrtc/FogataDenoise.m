#import "FogataDenoise.h"

#import <dlfcn.h>
#import <stdatomic.h>

#import "AudioManager.h"
#import "AudioProcessingAdapter.h"

// What fogata-ffi's denoise.rs hands over (`DenoiseApi`); the same as fogata_denoise.h
// for Linux and Windows.
typedef struct {
  uint32_t version;
  void* (*create)(void);
  void (*process)(void* state, bool on, int32_t rate, float* samples, size_t frames);
} FogataDenoiseApi;

static const uint32_t kFogataDenoiseApiVersion = 1;

// Channels cleaned; WebRTC mixes the microphone down to one unless asked otherwise.
enum { kFogataDenoiseChannels = 2 };

// On or off, read on WebRTC's audio thread on every 10 ms.
static atomic_bool gFogataDenoiseOn;

@interface FogataDenoiseProcessor : NSObject <ExternalAudioProcessingDelegate>
- (instancetype)initWithApi:(const FogataDenoiseApi*)api;
@end

@implementation FogataDenoiseProcessor {
  const FogataDenoiseApi* _api;
  void* _states[kFogataDenoiseChannels];
}

- (instancetype)initWithApi:(const FogataDenoiseApi*)api {
  self = [super init];
  if (self) {
    _api = api;
    // Never freed: the processor stays with the audio processing module for good.
    for (int i = 0; i < kFogataDenoiseChannels; i++) _states[i] = api->create();
  }
  return self;
}

- (void)audioProcessingInitializeWithSampleRate:(size_t)sampleRateHz channels:(size_t)channels {
}

// Each channel's whole band (not split into bands), 10 ms at the rate WebRTC processes at
// (16, 32 or 48 kHz), samples as 16-bit values in floats.
- (void)audioProcessingProcess:(RTC_OBJC_TYPE(RTCAudioBuffer) *)audioBuffer {
  const bool on = atomic_load_explicit(&gFogataDenoiseOn, memory_order_relaxed);
  const size_t frames = audioBuffer.frames;
  // 10 ms at 48 kHz at most; anything else goes on untouched.
  if (frames == 0 || frames > 480) return;
  const size_t channels = MIN(audioBuffer.channels, (size_t)kFogataDenoiseChannels);
  for (size_t i = 0; i < channels; i++) {
    _api->process(_states[i], on, (int32_t)(frames * 100), [audioBuffer rawBufferForChannel:i],
                  frames);
  }
}

- (void)audioProcessingRelease {
}

@end

// The library loaded in this process that |address| lies in; NULL outside every one.
static const void* FogataImageOf(const void* address) {
  Dl_info info;
  if (dladdr(address, &info) == 0) return NULL;
  return info.dli_fbase;
}

@implementation FogataDenoise

+ (BOOL)suppress:(BOOL)on api:(int64_t)api {
  static FogataDenoiseProcessor* processor = nil;
  @synchronized(self) {
    if (processor == nil) {
      // Only a table inside a library loaded in this process, with its functions in that
      // same library: any other address is not read past finding where it lies.
      const FogataDenoiseApi* table = (const FogataDenoiseApi*)(intptr_t)api;
      const void* image = api > 0 ? FogataImageOf(table) : NULL;
      if (image == NULL || table->version != kFogataDenoiseApiVersion ||
          table->create == NULL || table->process == NULL ||
          FogataImageOf((const void*)table->create) != image ||
          FogataImageOf((const void*)table->process) != image) {
        return NO;
      }
      processor = [[FogataDenoiseProcessor alloc] initWithApi:table];
      [AudioManager.sharedInstance.capturePostProcessingAdapter addProcessing:processor];
    }
    atomic_store_explicit(&gFogataDenoiseOn, on, memory_order_relaxed);
    return YES;
  }
}

@end
