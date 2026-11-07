// A computer without a camera: opening one says so instead of giving a stream that never
// shows a picture.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/camera_preview.dart';
import 'package:fogata/src/l10n.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('FlutterWebRTC.Method');
  final calls = <String>[];

  void answer(
    Future<Object?> Function(MethodCall call) getUserMedia, {
    Future<Object?> Function()? createVideoRenderer,
  }) {
    calls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call.method);
          return switch (call.method) {
            'getUserMedia' => getUserMedia(call),
            'createVideoRenderer' when createVideoRenderer != null =>
              createVideoRenderer(),
            _ => null,
          };
        });
  }

  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );

  test('a stream without a picture (Linux, Windows) is no camera', () async {
    answer(
      (_) async => {'streamId': 's', 'audioTracks': [], 'videoTracks': []},
    );
    await expectLater(openCameraStream(null), throwsA(isA<NoCamera>()));
    // The empty stream is let go.
    expect(calls, containsAllInOrder(['getUserMedia', 'streamDispose']));
  });

  test('NotFoundError (macOS) is no camera', () async {
    answer(
      (_) async => throw PlatformException(
        code: 'Error DOMException',
        message: 'NotFoundError',
      ),
    );
    await expectLater(openCameraStream(null), throwsA(isA<NoCamera>()));
  });

  test('a camera that cannot be used is another failure', () async {
    answer(
      (_) async => throw PlatformException(
        code: 'Error DOMException',
        message: 'NotAllowedError',
      ),
    );
    await expectLater(openCameraStream(null), throwsA(isNot(isA<NoCamera>())));
  });

  test(
    'cameras the Camera portal refused (Linux, Flatpak) are denied',
    () async {
      // As the plugin answers: getUserMedia's message, which Dart passes on.
      answer(
        (_) async => throw PlatformException(
          code: 'NotAllowedError',
          message: 'NotAllowedError: the system refused the camera (fogataCameraDenied)',
        ),
      );
      await expectLater(openCameraStream(null), throwsA(isA<CameraDenied>()));
    },
  );

  test('no Camera portal to reach the cameras is told apart', () async {
    answer(
      (_) async => throw PlatformException(
        code: 'NotReadableError',
        message:
            'NotReadableError: no camera portal to reach the cameras '
            '(fogataCameraUnavailable)',
      ),
    );
    await expectLater(
      openCameraStream(null),
      throwsA(isA<CameraUnavailable>()),
    );
  });

  test('each failure has its own words', () {
    expect(cameraFailure(NoCamera()), l10n.devicesNoCamera);
    expect(cameraFailure(CameraDenied()), l10n.devicesCameraDenied);
    expect(cameraFailure(CameraUnavailable()), l10n.devicesCameraUnavailable);
    expect(
      cameraFailure(Exception('x')),
      isNot(
        anyOf(
          l10n.devicesNoCamera,
          l10n.devicesCameraDenied,
          l10n.devicesCameraUnavailable,
        ),
      ),
    );
    expect(noCameraNote(CameraAccess.direct), l10n.devicesNoCamera);
    expect(noCameraNote(CameraAccess.granted), l10n.devicesNoCamera);
    expect(noCameraNote(CameraAccess.denied), l10n.devicesCameraDenied);
    expect(
      noCameraNote(CameraAccess.unavailable),
      l10n.devicesCameraUnavailable,
    );
  });

  test('a camera with a picture opens', () async {
    answer(
      (_) async => {
        'streamId': 's',
        'audioTracks': [],
        'videoTracks': [
          {'id': 't', 'label': 't', 'kind': 'video', 'enabled': true},
        ],
      },
    );
    final stream = await openCameraStream('cam-1');
    expect(stream.getVideoTracks(), hasLength(1));
  });

  Future<Object?> aCamera(MethodCall _) async => {
    'streamId': 's',
    'audioTracks': [],
    'videoTracks': [
      {'id': 't', 'label': 't', 'kind': 'video', 'enabled': true},
    ],
  };

  test('a preview whose picture cannot be made lets the camera go', () async {
    answer(
      aCamera,
      createVideoRenderer: () async =>
          throw PlatformException(code: 'no texture'),
    );
    await expectLater(
      openWebrtcCamera(null),
      throwsA(isA<PlatformException>()),
    );
    // The camera opened for it is stopped and its stream let go of.
    expect(
      calls,
      containsAllInOrder(['getUserMedia', 'trackDispose', 'streamDispose']),
    );
  });

  test('a preview given no texture lets the camera go', () async {
    answer(aCamera, createVideoRenderer: () async => <String, Object?>{});
    await expectLater(openWebrtcCamera(null), throwsA(anything));
    expect(
      calls,
      containsAllInOrder(['getUserMedia', 'trackDispose', 'streamDispose']),
    );
  });
}
