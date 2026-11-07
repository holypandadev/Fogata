// The camera through the system's Camera portal, as the Flatpak reaches it on Linux
// (tools/beta/test-flatpak-camera.sh runs it in a Flatpak with the application's own
// permissions, in a desktop session of its own with a made-up camera): the settings ask
// for the cameras as they do when the camera's choice shows, and then
//   granted (FOGATA_TEST_CAMERA_PORTAL=granted, the default): the camera
//     FOGATA_TEST_CAMERA_NAME is listed, with the id FOGATA_TEST_CAMERA_ID when given, and
//     opens as a call opens it, its pictures reaching Flutter at the size asked;
//   denied: no camera is listed, the settings say the system refused it, and opening one
//     fails with [CameraDenied].
// It writes "passed", or why it failed, to $FOGATA_TEST_OUT/camera-result.txt.
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/l10n.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the camera through the Camera portal', (tester) async {
    final out = _env['FOGATA_TEST_OUT'] ?? Directory.systemTemp.path;
    final result = File('$out/camera-result.txt');
    try {
      await _camera(tester);
      result.writeAsStringSync('passed\n');
    } catch (e, trace) {
      result.writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

void _log(String line) => stderr.writeln('camera test: $line');

Future<void> _camera(WidgetTester tester) async {
  final expected = _env['FOGATA_TEST_CAMERA_PORTAL'] ?? 'granted';
  final access = await askCameras();
  _log('access: ${access.name}');
  expect(access.name, expected);
  final cameras = [
    for (final d in await navigator.mediaDevices.enumerateDevices())
      if (d.kind == 'videoinput') d,
  ];
  _log('cameras: ${[for (final c in cameras) '${c.label} (${c.deviceId})']}');

  if (expected == 'denied') {
    expect(cameras, isEmpty);
    expect(noCameraNote(access), l10n.devicesCameraDenied);
    Object? failure;
    try {
      await openCameraStream(null);
    } catch (e) {
      failure = e;
    }
    expect(failure, isA<CameraDenied>());
    expect(cameraFailure(failure), l10n.devicesCameraDenied);
    return;
  }

  final name = _env['FOGATA_TEST_CAMERA_NAME'] ?? 'Fogata test camera';
  final camera = cameras.where((c) => c.label == name).toList();
  expect(camera, hasLength(1), reason: '$name is listed');
  final id = _env['FOGATA_TEST_CAMERA_ID'];
  if (id != null) expect(camera.first.deviceId, id);

  final stream = await openCameraStream(camera.first.deviceId);
  final renderer = RTCVideoRenderer();
  await renderer.initialize();
  renderer.srcObject = stream;
  await tester.pumpWidget(
    MaterialApp(
      home: Center(
        child: SizedBox(width: 640, height: 360, child: RTCVideoView(renderer)),
      ),
    ),
  );
  for (var i = 0; i < 100 && renderer.videoWidth == 0; i++) {
    await Future<void>.delayed(const Duration(milliseconds: 100));
    await tester.pump();
  }
  _log('picture: ${renderer.videoWidth}x${renderer.videoHeight}');
  expect((renderer.videoWidth, renderer.videoHeight), (1280, 720));
  renderer.srcObject = null;
  for (final track in stream.getTracks()) {
    await track.stop();
  }
  await stream.dispose();
  await renderer.dispose();
}
