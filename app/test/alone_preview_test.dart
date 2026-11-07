// Alone in a room, this device's own camera and shared screen show on its page, as they
// do once others come in.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _show(WidgetTester tester, AppState state) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// A renderer that got its first frame.
void _framed(RTCVideoRenderer renderer) =>
    renderer.value = const RTCVideoValue(width: 1280, height: 720);

/// The pictures drawn on the page.
List<RTCVideoRenderer> _shown(WidgetTester tester) => [
  for (final view in tester.widgetList<RTCVideoView>(find.byType(RTCVideoView)))
    view.videoRenderer,
];

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets('the own shared screen shows while alone, and enlarges', (
    tester,
  ) async {
    final state = fakeRoom(people: 1, own: true);
    final call = state.callState.call!;
    _framed(call.localScreen);
    await _show(tester, state);
    expect(phaseOf(state), Phase.alone);
    expect(find.text('Only you'), findsWidgets);
    expect(_shown(tester), [call.localScreen]);
    expect(find.text('Your screen'), findsOneWidget);
    expect(find.text('Marta (you)'), findsWidgets);
    expect(
      find.text('When someone from the group comes in, they appear here.'),
      findsOneWidget,
    );
    // Nobody to send it to: it is drawn here only.
    expect(call.peers, isEmpty);
    await tester.tap(find.text('Your screen'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(call.ownScreen, isTrue);
    expect(_shown(tester), contains(call.localScreen));
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('the own camera shows while alone, with the screen or not', (
    tester,
  ) async {
    final state = fakeRoom(people: 1);
    final call = state.callState.call!..camera = true;
    _framed(call.localCamera);
    _framed(call.localScreen);
    await _show(tester, state);
    expect(phaseOf(state), Phase.alone);
    expect(_shown(tester), [call.localCamera]);
    call.sharing = true;
    state.callState.changed();
    await tester.pump(const Duration(milliseconds: 400));
    expect(
      _shown(tester),
      unorderedEquals([call.localScreen, call.localCamera]),
    );
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('a camera with no picture yet shows the avatar', (tester) async {
    final state = fakeRoom(people: 1);
    final call = state.callState.call!..camera = true;
    await _show(tester, state);
    expect(_shown(tester), isEmpty);
    expect(find.text('Marta (you)'), findsWidgets);
    _framed(call.localCamera);
    await tester.pump();
    expect(_shown(tester), [call.localCamera]);
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
