// This device's own camera is drawn mirrored, or as it is, as the person chose in the
// settings: in the room's tile, enlarged and in the settings' preview. Others' cameras are
// never mirrored, and what is sent is the camera's picture as it is (the mirror is only how
// RTCVideoView draws it).

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/camera_preview.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'camera_preview_test.dart' show FakeFeed;
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

void _framed(RTCVideoRenderer renderer) =>
    renderer.value = const RTCVideoValue(width: 1280, height: 720);

/// Whether each picture drawn on the page is mirrored, by its renderer.
Map<RTCVideoRenderer, bool> _mirrored(WidgetTester tester) => {
  for (final view in tester.widgetList<RTCVideoView>(find.byType(RTCVideoView)))
    view.videoRenderer: view.mirror,
};

/// The choice, as the settings make it (without writing app.json, which tests cannot).
void _choose(AppState state, bool mirror) => state.prefs
  ..mirrorCamera = mirror
  ..changed();

Future<void> _end(WidgetTester tester, AppState state) async {
  state.callState.call = null;
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets(
    'the own camera is mirrored by default, and not once turned off',
    (tester) async {
      final state = fakeRoom(people: 1);
      final call = state.callState.call!..camera = true;
      _framed(call.localCamera);
      expect(state.prefs.mirrorCamera, isTrue);
      await _show(tester, state);
      expect(_mirrored(tester), {call.localCamera: true});
      _choose(state, false);
      await tester.pump();
      expect(_mirrored(tester), {call.localCamera: false});
      // Enlarged, it follows the choice too.
      call.focus(me.id);
      state.callState.changed();
      await tester.pump(const Duration(milliseconds: 400));
      expect(_mirrored(tester)[call.localCamera], isFalse);
      _choose(state, true);
      await tester.pump(const Duration(milliseconds: 400));
      expect(_mirrored(tester)[call.localCamera], isTrue);
      await _end(tester, state);
    },
  );

  testWidgets('the others\' cameras are never mirrored, whatever the choice', (
    tester,
  ) async {
    final state = fakeRoom(people: 2, cameras: 1);
    final call = state.callState.call!..camera = true;
    final peer = call.peers.values.single;
    _framed(call.localCamera);
    _framed(peer.camera);
    for (final mirror in [true, false]) {
      _choose(state, mirror);
      await _show(tester, state);
      final drawn = _mirrored(tester);
      expect(drawn[peer.camera], isFalse, reason: 'mirror: $mirror');
      expect(drawn[call.localCamera], mirror);
    }
    await _end(tester, state);
  });

  testWidgets('the settings\' switch changes it, and the preview follows', (
    tester,
  ) async {
    final feeds = <FakeFeed>[];
    final preview = CameraPreview(
      open: (id) async {
        final feed = FakeFeed(id);
        feeds.add(feed);
        return feed;
      },
    );
    final state = fakeRoom(people: 2)..callState.call = null;
    final semantics = tester.ensureSemantics();
    tester.view.physicalSize = const Size(800, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Material(
            child: SizedBox(
              width: 500,
              child: Column(
                children: [
                  TestVideo(loaded: true, hasCamera: true, preview: preview),
                  const MirrorCameraSwitch(),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    expect(find.text('Mirror my camera'), findsOneWidget);
    expect(
      find.text(
        'Only how you see yourself here. Others always see your camera as it is.',
      ),
      findsOneWidget,
    );
    expect(
      find.bySemanticsLabel(RegExp('Mirror my camera on')),
      findsOneWidget,
    );
    await tester.tap(find.text('Test video'));
    await tester.pump();
    expect(feeds.single.mirrored, isTrue);
    expect(find.byKey(const ValueKey('mirrored')), findsOneWidget);
    await tester.tap(find.text('Mirror my camera'));
    await tester.pump();
    expect(state.prefs.mirrorCamera, isFalse);
    expect(feeds.single.mirrored, isFalse);
    expect(find.byKey(const ValueKey('as it is')), findsOneWidget);
    expect(
      find.bySemanticsLabel(RegExp('Mirror my camera off')),
      findsOneWidget,
    );
    await tester.pumpWidget(const SizedBox());
    semantics.dispose();
  });

  test('app.json keeps the choice; on when it says nothing', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-mirror');
    addTearDown(() => dir.delete(recursive: true));
    final prefs = Prefs(dir.path);
    await prefs.load();
    expect(prefs.mirrorCamera, isTrue);
    File('${dir.path}/app.json').writeAsStringSync('{"mirrorCamera": false}');
    final off = Prefs(dir.path);
    await off.load();
    expect(off.mirrorCamera, isFalse);
  });
}
