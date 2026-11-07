import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart' show defaultAlias;
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'the menus follow the devices plugged in, never the call\'s silent '
    'input',
    (tester) async {
      final sources = <Map<String, Object?>>[
        {'deviceId': 'mic-1', 'label': 'Desk microphone', 'kind': 'audioinput'},
        {'deviceId': 'quiet', 'label': 'Fogata silence', 'kind': 'audioinput'},
        {'deviceId': 'out-1', 'label': 'Headphones', 'kind': 'audiooutput'},
      ];
      var listed = 0;
      final messenger = tester.binding.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Method'),
        (call) async {
          if (call.method != 'getSources') return null;
          listed++;
          return {'sources': sources};
        },
      );
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Event'),
        (call) async => null,
      );
      addTearDown(() {
        messenger.setMockMethodCallHandler(
          const MethodChannel('FlutterWebRTC.Method'),
          null,
        );
        messenger.setMockMethodCallHandler(
          const MethodChannel('FlutterWebRTC.Event'),
          null,
        );
      });
      tester.view.physicalSize = const Size(800, 900);
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
            home: Material(
              child: Center(
                child: SizedBox(
                  width: 500,
                  child: DeviceChoices(testVideo: true),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(find.text('No camera is connected.'), findsOneWidget);

      await tester.tap(find.text('System default').first);
      await tester.pumpAndSettle();
      expect(find.text('Desk microphone'), findsOneWidget);
      expect(find.text('Fogata silence'), findsNothing);
      await tester.tapAt(Offset.zero);
      await tester.pumpAndSettle();

      // A camera plugged in while the settings show.
      sources.add({
        'deviceId': 'cam-1',
        'label': 'USB camera',
        'kind': 'videoinput',
      });
      final before = listed;
      await tester.pump(const Duration(seconds: 5));
      await tester.pump();
      expect(listed, greaterThan(before));
      expect(find.text('No camera is connected.'), findsNothing);
      expect(find.text('Only you see the preview.'), findsOneWidget);

      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  test('the system default under its own names is no device to choose', () {
    for (final id in [
      'default: Desk microphone',
      'default',
      'communications',
      '',
    ]) {
      expect(defaultAlias(id), isTrue, reason: id);
    }
    for (final id in [
      null,
      'Desk microphone',
      'Default microphone',
      '{0.0.1.00000000}.{7b3d40b9-76b7-4333-97ec-831581d1e3bd}',
    ]) {
      expect(defaultAlias(id), isFalse, reason: id);
    }
  });

  testWidgets('a device panel follows the devices plugged in while it is open, '
      'and leaves out the system default\'s other names', (tester) async {
    final sources = <Map<String, Object?>>[
      {
        'deviceId': 'default: Desk microphone',
        'label': 'default: Desk microphone',
        'kind': 'audioinput',
      },
      {'deviceId': 'default', 'label': 'Desk microphone', 'kind': 'audioinput'},
      {'deviceId': 'communications', 'label': 'Headset', 'kind': 'audioinput'},
      {'deviceId': '', 'label': '', 'kind': 'audioinput'},
      {'deviceId': 'x-1', 'label': ' ', 'kind': 'audioinput'},
      {'deviceId': 'mic-1', 'label': 'Desk microphone', 'kind': 'audioinput'},
    ];
    var listed = 0;
    final messenger = tester.binding.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        if (call.method != 'getSources') return null;
        listed++;
        return {'sources': sources};
      },
    );
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Event'),
      (call) async => null,
    );
    addTearDown(() {
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Method'),
        null,
      );
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Event'),
        null,
      );
    });
    tester.view.physicalSize = const Size(800, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = fakeRoom(people: 2)
      ..callState.call = null
      ..roomState.room = null
      ..nav.page = Screen.home;
    state.prefs.audioInput = 'usb-1';
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(
            child: Center(child: DevicePanel(kind: DeviceKind.input)),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(find.text('System default'), findsOneWidget);
    expect(find.text('Desk microphone'), findsOneWidget);
    expect(find.text('Headset'), findsNothing);
    expect(find.textContaining('default:'), findsNothing);
    expect(find.text('Unnamed'), findsNothing);
    expect(find.textContaining('is not connected'), findsOneWidget);

    // The chosen microphone plugged in while the panel is open.
    sources.add({
      'deviceId': 'usb-1',
      'label': 'USB microphone',
      'kind': 'audioinput',
    });
    final before = listed;
    await tester.pump(const Duration(seconds: 5));
    await tester.pump();
    expect(listed, greaterThan(before));
    expect(find.text('USB microphone'), findsOneWidget);
    expect(find.textContaining('is not connected'), findsNothing);

    // And taken out again.
    sources.removeLast();
    await tester.pump(const Duration(seconds: 5));
    await tester.pump();
    expect(find.text('USB microphone'), findsNothing);
    expect(find.textContaining('is not connected'), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
