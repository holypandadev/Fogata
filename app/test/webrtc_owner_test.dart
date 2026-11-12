import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
// The plugin's own classes, as the call gets them from a connection.
// ignore: implementation_imports
import 'package:flutter_webrtc/src/native/media_stream_impl.dart';
// ignore: implementation_imports
import 'package:flutter_webrtc/src/native/media_stream_track_impl.dart';

// What the WebRTC plugin is told of a track or a stream names its owner, so that a track a
// connection received is never taken for one of this device's own of the same id: a
// stream made to show a participant's picture, let go of, stopped the camera that had
// the same id, and muting a received sound muted the microphone.
void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  final calls = <MethodCall>[];

  setUp(() {
    calls.clear();
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        calls.add(call);
        if (call.method == 'createLocalMediaStream') {
          return <String, dynamic>{'streamId': 'made'};
        }
        return <String, dynamic>{};
      },
    );
  });
  tearDown(
    () => messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    ),
  );

  Map<Object?, Object?> argumentsOf(String method) =>
      calls.lastWhere((c) => c.method == method).arguments as Map;

  final received = MediaStreamTrackNative('cam', 'cam', 'video', true, 'pc-1');

  test(
    'a stream made here is owned by this device, whatever its label',
    () async {
      final stream = await createLocalMediaStream('a000000000000000-1');
      expect(stream.ownerTag, 'local');
      await stream.dispose();
      expect(argumentsOf('streamDispose'), {
        'streamId': 'made',
        'ownerTag': 'local',
      });
    },
  );

  test(
    'a received track added to a stream is named with its connection',
    () async {
      final stream = await createLocalMediaStream('shown');
      await stream.addTrack(received);
      expect(argumentsOf('mediaStreamAddTrack'), {
        'streamId': 'made',
        'trackId': 'cam',
        'peerConnectionId': 'pc-1',
      });
      await stream.removeTrack(received);
      expect(argumentsOf('mediaStreamRemoveTrack')['peerConnectionId'], 'pc-1');
    },
  );

  test('a received track stopped is named with its connection', () async {
    await received.stop();
    expect(argumentsOf('trackDispose'), {
      'trackId': 'cam',
      'peerConnectionId': 'pc-1',
    });
  });

  test('a stream a connection received is let go of as its own', () async {
    await MediaStreamNative('theirs', 'pc-1').dispose();
    expect(argumentsOf('streamDispose'), {
      'streamId': 'theirs',
      'ownerTag': 'pc-1',
    });
  });
}
