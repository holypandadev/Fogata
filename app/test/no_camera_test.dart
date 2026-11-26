// A computer without a camera: opening one says so instead of giving a stream that never
// shows a picture.

import 'package:flutter/foundation.dart';
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

  test('a camera another application holds is told apart', () async {
    answer(
      (_) async => throw PlatformException(
        code: 'NotReadableError',
        message:
            'NotReadableError: another application is using the camera '
            '(fogataCameraBusy): /dev/video2: VIDIOC_S_FMT: Device or resource busy',
      ),
    );
    final printed = <String>[];
    final printBefore = debugPrint;
    debugPrint = (message, {wrapWidth}) => printed.add(message ?? '');
    addTearDown(() => debugPrint = printBefore);
    await expectLater(openCameraStream(null), throwsA(isA<CameraBusy>()));
    // The cause, for the day's log: the device and the system's answer.
    expect(
      printed,
      contains(contains('/dev/video2: VIDIOC_S_FMT: Device or resource busy')),
    );
  });

  test(
    'a camera through the portal another application holds is told apart',
    () async {
      // What the plugin answers when PipeWire's stream was told the link failed with
      // EBUSY (fogata_portal_camera.h), in the sound server's language.
      const said =
          'error set output format: -16 (Dispositivo o recurso ocupado)';
      answer(
        (_) async => throw PlatformException(
          code: 'NotReadableError',
          message:
              'NotReadableError: another application is using the camera '
              '(fogataCameraBusy): $said',
        ),
      );
      final printed = <String>[];
      final print = debugPrint;
      debugPrint = (message, {wrapWidth}) => printed.add(message ?? '');
      addTearDown(() => debugPrint = print);
      await expectLater(openCameraStream(null), throwsA(isA<CameraBusy>()));
      expect(printed, contains(contains(said)));
    },
  );

  test('a camera through the portal that did not start leaves its cause in the log', () async {
    const said =
        'NotReadableError: the camera did not start: '
        'Format negotiation failed';
    answer(
      (_) async =>
          throw PlatformException(code: 'NotReadableError', message: said),
    );
    final printed = <String>[];
    final print = debugPrint;
    debugPrint = (message, {wrapWidth}) => printed.add(message ?? '');
    addTearDown(() => debugPrint = print);
    Object? failure;
    try {
      await openCameraStream(null);
    } catch (e) {
      failure = e;
    }
    expect(failure, isA<PlatformException>());
    expect(printed, contains(contains('PipeWire stream paused')));
    // Told as a camera that failed, not as one of the failures with words of their own.
    expect(
      cameraFailure(failure),
      isNot(
        anyOf(
          l10n.devicesNoCamera,
          l10n.devicesCameraDenied,
          l10n.devicesCameraUnavailable,
          l10n.devicesCameraBusy,
        ),
      ),
    );
  });

  test('each failure has its own words', () {
    expect(cameraFailure(NoCamera()), l10n.devicesNoCamera);
    expect(cameraFailure(CameraDenied()), l10n.devicesCameraDenied);
    expect(cameraFailure(CameraUnavailable()), l10n.devicesCameraUnavailable);
    expect(cameraFailure(CameraBusy()), l10n.devicesCameraBusy);
    expect(
      cameraFailure(Exception('x')),
      isNot(
        anyOf(
          l10n.devicesNoCamera,
          l10n.devicesCameraDenied,
          l10n.devicesCameraUnavailable,
          l10n.devicesCameraBusy,
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
