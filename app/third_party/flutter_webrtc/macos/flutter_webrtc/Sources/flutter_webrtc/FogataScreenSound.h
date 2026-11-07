#import <Foundation/Foundation.h>
#import <WebRTC/WebRTC.h>

NS_ASSUME_NONNULL_BEGIN

// The other applications' sound, for a shared screen: everything the computer plays
// but this application's own (the call); for a shared window, only its application's. WebRTC hands what an audio module records to
// every channel of sound that sends, so this sound goes out on connections made by a
// factory of its own, whose module records nothing else: it never opens the microphone
// or the speakers, and no voice gets in.
API_AVAILABLE(macos(13.0))
@interface FogataScreenSound : NSObject

+ (instancetype)shared;

// Makes connections that carry this sound.
@property(nonatomic, readonly) RTCPeerConnectionFactory* factory;

// Starts taking the sound; the answer says whether it could (the screen recording
// permission is needed). On the main queue.
- (void)startWithCompletion:(void (^)(BOOL started))completion;

// The same for a shared window, named by its window number: only the sound of the
// application it belongs to (and of that application's helpers, which play for browsers
// and the like). Nil takes everything but this application's sound, as for a screen.
- (void)startForWindow:(nullable NSString*)window
            completion:(void (^)(BOOL started))completion;

- (void)stop;

// Adds a channel that sends this sound to a connection made by `factory`.
- (nullable RTCRtpTransceiver*)sendOn:(RTCPeerConnection*)connection;

@end

NS_ASSUME_NONNULL_END
