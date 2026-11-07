// Cameras that come through the system's Camera portal (Linux, in a Flatpak): the camera's
// choice asks for them once when it shows, lists them once granted, and says why none is
// listed when the system refused them or has no portal. Elsewhere nothing is asked.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

const _camera = {'deviceId': 'cam-1', 'label': 'Webcam', 'kind': 'videoinput'};

/// Shows the camera's part of the settings, with the system answering [answer] when the
/// portal is asked (`request`) and listing the camera only when [listed] says so of the
/// answer given so far.
Future<List<bool>> _show(
  WidgetTester tester, {
  required CameraAccess before,
  required CameraAccess answer,
  required bool Function(CameraAccess access) listed,
}) async {
  final asked = <bool>[];
  var access = before;
  final original = readCameraAccess;
  readCameraAccess = (request) async {
    asked.add(request);
    if (request) access = answer;
    return access;
  };
  addTearDown(() => readCameraAccess = original);
  final messenger = tester.binding.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(
    const MethodChannel('FlutterWebRTC.Method'),
    (call) async => call.method == 'getSources'
        ? {
            'sources': [if (listed(access)) _camera],
          }
        : null,
  );
  addTearDown(
    () => messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    ),
  );
  tester.view.physicalSize = const Size(800, 600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 2)
    ..callState.call = null
    ..roomState.room = null
    ..nav.page = Screen.home;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(
          child: SizedBox(
            width: 500,
            child: DeviceChoices(audio: false, testVideo: true),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 100));
  return asked;
}

Future<void> _gone(WidgetTester tester) async {
  // The lists' timers stop with the widget.
  await tester.pumpWidget(const SizedBox());
}

void main() {
  setUpAll(loadFonts);

  testWidgets('granted, the cameras are asked for once and listed', (
    tester,
  ) async {
    final asked = await _show(
      tester,
      before: CameraAccess.notAsked,
      answer: CameraAccess.granted,
      listed: (access) => access == CameraAccess.granted,
    );
    expect(asked, [false, true]);
    // Listed again once granted: the preview can start.
    expect(find.text(l10n.devicesPreviewOnlyYou), findsOneWidget);
    expect(find.text(l10n.devicesNoCamera), findsNothing);
    await _gone(tester);
  });

  testWidgets('refused, it says so instead of "no camera"', (tester) async {
    final asked = await _show(
      tester,
      before: CameraAccess.notAsked,
      answer: CameraAccess.denied,
      listed: (_) => false,
    );
    expect(asked, [false, true]);
    expect(find.text(l10n.devicesCameraDenied), findsOneWidget);
    expect(find.text(l10n.devicesNoCamera), findsNothing);
    await _gone(tester);
  });

  testWidgets('a "no" given before is not asked again here', (tester) async {
    final asked = await _show(
      tester,
      before: CameraAccess.denied,
      answer: CameraAccess.denied,
      listed: (_) => false,
    );
    expect(asked, [false]);
    expect(find.text(l10n.devicesCameraDenied), findsOneWidget);
    await _gone(tester);
  });

  testWidgets('without a portal, it says so', (tester) async {
    await _show(
      tester,
      before: CameraAccess.notAsked,
      answer: CameraAccess.unavailable,
      listed: (_) => false,
    );
    expect(find.text(l10n.devicesCameraUnavailable), findsOneWidget);
    await _gone(tester);
  });

  testWidgets('cameras opened directly are never asked for', (tester) async {
    final asked = await _show(
      tester,
      before: CameraAccess.direct,
      answer: CameraAccess.direct,
      listed: (_) => false,
    );
    expect(asked, [false]);
    expect(find.text(l10n.devicesNoCamera), findsOneWidget);
    await _gone(tester);
  });
}
