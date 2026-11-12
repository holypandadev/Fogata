#import "FlutterWebRTCPlugin.h"

@interface RTCPeerConnection (Flutter) <FlutterStreamHandler>
@property(nonatomic, strong, nonnull) NSMutableDictionary<NSString*, RTCDataChannel*>* dataChannels;
@property(nonatomic, strong, nonnull)
    NSMutableDictionary<NSString*, RTCMediaStream*>* remoteStreams;
@property(nonatomic, strong, nonnull)
    NSMutableDictionary<NSString*, RTCMediaStreamTrack*>* remoteTracks;
@property(nonatomic, strong, nonnull) NSString* flutterId;
@property(nonatomic, strong, nullable) FlutterEventSink eventSink;
@property(nonatomic, strong, nullable) FlutterEventChannel* eventChannel;
// Set when the plugin has closed this connection. A closed connection stays
// registered until it is disposed so that its event channel handler can be
// released at that point.
@property(nonatomic, assign) BOOL closedByPlugin;

// Fogata: `remoteTracks` and `remoteStreams` are written by the connection's delegate
// methods, which WebRTC calls on its signalling thread, and read by method calls on the
// main thread: they are used only through these, under a lock. The lock is never held
// while anything else is called, and what leaves the maps is let go of after it: freeing
// a received track or stream may wait for the signalling thread, which may be waiting
// for the lock.
- (nullable RTCMediaStreamTrack*)fogataRemoteTrack:(nonnull NSString*)trackId;
- (nullable RTCMediaStream*)fogataRemoteStream:(nonnull NSString*)streamId;
- (void)fogataKeepRemoteTrack:(nonnull RTCMediaStreamTrack*)track;
- (void)fogataKeepRemoteStream:(nonnull RTCMediaStream*)stream;
- (void)fogataForgetRemoteTrack:(nonnull NSString*)trackId;
// How many entries of `remoteStreams` hold `stream`.
- (NSUInteger)fogataRemoteStreamEntries:(nonnull RTCMediaStream*)stream;
// Empties both maps.
- (void)fogataForgetRemote;
@end

@interface FlutterWebRTCPlugin (RTCPeerConnection)

- (void)peerConnectionCreateOffer:(nonnull NSDictionary*)constraints
                   peerConnection:(nonnull RTCPeerConnection*)peerConnection
                           result:(nonnull FlutterResult)result;

- (void)peerConnectionCreateAnswer:(nonnull NSDictionary*)constraints
                    peerConnection:(nonnull RTCPeerConnection*)peerConnection
                            result:(nonnull FlutterResult)result;

- (void)peerConnectionSetLocalDescription:(nonnull RTCSessionDescription*)sdp
                           peerConnection:(nonnull RTCPeerConnection*)peerConnection
                                   result:(nonnull FlutterResult)result;

- (void)peerConnectionSetRemoteDescription:(nonnull RTCSessionDescription*)sdp
                            peerConnection:(nonnull RTCPeerConnection*)peerConnection
                                    result:(nonnull FlutterResult)result;

- (void)peerConnectionAddICECandidate:(nonnull RTCIceCandidate*)candidate
                       peerConnection:(nonnull RTCPeerConnection*)peerConnection
                               result:(nonnull FlutterResult)result;

- (void)peerConnectionGetStats:(nonnull RTCPeerConnection*)peerConnection
                        result:(nonnull FlutterResult)result;

- (void)peerConnectionGetStatsForTrackId:(nonnull NSString*)trackID
                          peerConnection:(nonnull RTCPeerConnection*)peerConnection
                                  result:(nonnull FlutterResult)result;

- (nonnull RTCMediaConstraints*)parseMediaConstraints:(nonnull NSDictionary*)constraints;

- (void)peerConnectionSetConfiguration:(nonnull RTCConfiguration*)configuration
                        peerConnection:(nonnull RTCPeerConnection*)peerConnection;

- (void)peerConnectionGetRtpReceiverCapabilities:(nonnull NSDictionary*)argsMap
                                          result:(nonnull FlutterResult)result;

- (void)peerConnectionGetRtpSenderCapabilities:(nonnull NSDictionary*)argsMap
                                        result:(nonnull FlutterResult)result;

- (void)transceiverSetCodecPreferences:(nonnull NSDictionary*)argsMap
                                result:(nonnull FlutterResult)result;

- (nullable NSString*)stringForSignalingState:(RTCSignalingState)state;

- (nullable NSString*)stringForICEGatheringState:(RTCIceGatheringState)state;

- (nullable NSString*)stringForICEConnectionState:(RTCIceConnectionState)state;

- (nullable NSString*)stringForPeerConnectionState:(RTCPeerConnectionState)state;

@end
