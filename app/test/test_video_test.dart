import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/camera_preview.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/shown.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'camera_preview_test.dart' show FakeFeed;
import 'fake_room.dart';

/// A preview whose camera is never real.
(CameraPreview, List<FakeFeed>) _fakePreview() {
  final feeds = <FakeFeed>[];
  final preview = CameraPreview(
    open: (id) async {
      final feed = FakeFeed(id);
      feeds.add(feed);
      return feed;
    },
  );
  return (preview, feeds);
}

/// A preview that counts what it was asked to do.
class _Counted extends CameraPreview {
  _Counted(List<FakeFeed> feeds)
    : super(
        open: (id) async {
          final feed = FakeFeed(id);
          feeds.add(feed);
          return feed;
        },
      );
  int starts = 0;
  int stops = 0;

  @override
  Future<void> start(String? id) {
    starts++;
    return super.start(id);
  }

  @override
  Future<void> stop() {
    stops++;
    return super.stop();
  }
}

AppState _state() => fakeRoom(people: 2)
  ..callState.call = null
  ..roomState.room = null
  ..nav.page = Screen.home;

Future<void> _pump(
  WidgetTester tester,
  AppState state,
  CameraPreview preview, {
  bool hasCamera = true,
}) async {
  tester.view.physicalSize = const Size(800, 600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Center(
            child: SizedBox(
              width: 500,
              child: TestVideo(
                loaded: true,
                hasCamera: hasCamera,
                preview: preview,
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

FgButton _button(WidgetTester tester) =>
    tester.widget<FgButton>(find.byKey(const ValueKey('test-video-button')));

void main() {
  setUpAll(loadFonts);

  testWidgets('the camera opens only on Test video, and closes on Stop', (
    tester,
  ) async {
    final (preview, feeds) = _fakePreview();
    final state = _state()..prefs.videoInput = 'cam-2';
    await _pump(tester, state, preview);
    expect(feeds, isEmpty);
    expect(find.text('Test video'), findsOneWidget);
    expect(find.text('Only you see the preview.'), findsOneWidget);
    await tester.tap(find.text('Test video'));
    await tester.pump();
    expect(feeds.single.id, 'cam-2');
    expect(find.byKey(const ValueKey('fake-camera')), findsOneWidget);
    expect(find.text('Stop video'), findsOneWidget);
    await tester.tap(find.text('Stop video'));
    await tester.pump();
    expect(feeds.single.stopped, isTrue);
    expect(find.byKey(const ValueKey('fake-camera')), findsNothing);
    expect(find.text('Test video'), findsOneWidget);
  });

  testWidgets('leaving the part closes the camera', (tester) async {
    final (preview, feeds) = _fakePreview();
    await _pump(tester, _state(), preview);
    await tester.tap(find.text('Test video'));
    await tester.pump();
    expect(feeds.single.stopped, isFalse);
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
    expect(feeds.single.stopped, isTrue);
    expect(preview.on, isFalse);
  });

  testWidgets('a hidden window closes the camera', (tester) async {
    final (preview, feeds) = _fakePreview();
    await _pump(tester, _state(), preview);
    await tester.tap(find.text('Test video'));
    await tester.pump();
    windowShown.value = false;
    addTearDown(() => windowShown.value = true);
    await tester.pump();
    expect(feeds.single.stopped, isTrue);
  });

  testWidgets('choosing another camera shows that one', (tester) async {
    final (preview, feeds) = _fakePreview();
    final state = _state()..prefs.videoInput = 'cam-1';
    await _pump(tester, state, preview);
    await tester.tap(find.text('Test video'));
    await tester.pump();
    state.prefs.videoInput = 'cam-2';
    state.notifyListeners();
    await tester.pump();
    await tester.pump();
    expect(feeds.map((f) => f.id), ['cam-1', 'cam-2']);
    expect(feeds.first.stopped, isTrue);
    expect(feeds.last.stopped, isFalse);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'another camera chosen stops and starts the preview once, however often it builds',
    (tester) async {
      final feeds = <FakeFeed>[];
      final preview = _Counted(feeds);
      final state = _state()..prefs.videoInput = 'cam-1';
      await _pump(tester, state, preview);
      await tester.tap(find.text('Test video'));
      await tester.pump();
      expect((preview.starts, preview.stops), (1, 0));
      state.prefs.videoInput = 'cam-2';
      // Built several times before the frame ends, as a layout or an ancestor can.
      final element = tester.element(find.byType(TestVideo));
      for (var i = 0; i < 3; i++) {
        element.markNeedsBuild();
        tester.binding.buildOwner!.buildScope(element);
      }
      await tester.pump();
      await tester.pump();
      await tester.pump();
      expect((preview.starts, preview.stops), (2, 1));
      expect(feeds.map((f) => f.id), ['cam-1', 'cam-2']);
      expect(feeds.last.stopped, isFalse);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('no camera: the button is off and says why', (tester) async {
    final (preview, feeds) = _fakePreview();
    await _pump(tester, _state(), preview, hasCamera: false);
    expect(find.text('No camera is connected.'), findsOneWidget);
    expect(_button(tester).onTap, isNull);
    await tester.tap(find.text('Test video'), warnIfMissed: false);
    await tester.pump();
    expect(feeds, isEmpty);
  });

  testWidgets('with the camera on in a call the preview waits', (tester) async {
    final (preview, feeds) = _fakePreview();
    final state = fakeRoom(people: 2);
    state.callState.call!.camera = true;
    await _pump(tester, state, preview);
    expect(find.text('The camera is on in your call.'), findsOneWidget);
    expect(_button(tester).onTap, isNull);
    expect(feeds, isEmpty);
  });

  testWidgets('a camera that cannot open says so', (tester) async {
    final preview = CameraPreview(open: (id) async => throw StateError('busy'));
    await _pump(tester, _state(), preview);
    await tester.tap(find.text('Test video'));
    await tester.pump();
    expect(find.textContaining('Could not use the camera.'), findsOneWidget);
    expect(find.text('Test video'), findsOneWidget);
  });

  testWidgets('Voice & video: the camera has its own box under the sound', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    for (final size in const [Size(960, 640), Size(1920, 1080)]) {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      final state = _state();
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      state.openSettings();
      state.showSettingsSection(SettingsSection.voice);
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      // Tests have no devices to list.
      tester.takeException();
      double top(Finder f) => tester.getRect(f).top;
      expect(find.text('Microphone and speaker'), findsOneWidget);
      expect(find.text('Microphone'), findsOneWidget);
      expect(find.text('Speaker'), findsOneWidget);
      expect(
        top(find.text('Speaker')),
        lessThan(top(find.text('Camera').first)),
      );
      // Test video first in the camera's box, then the choice of camera.
      expect(
        top(find.byKey(const ValueKey('test-video'))),
        lessThan(top(find.text('Camera').last)),
      );
      // The preview and its button in the middle of the box.
      final box = tester.getRect(find.byKey(const ValueKey('test-video')));
      final button = tester.getRect(
        find.byKey(const ValueKey('test-video-button')),
      );
      final panel = tester.getRect(
        find
            .ancestor(
              of: find.byKey(const ValueKey('test-video')),
              matching: find.byType(Panel),
            )
            .first,
      );
      expect(box.center.dx, moreOrLessEquals(panel.center.dx, epsilon: 1));
      expect(button.center.dx, moreOrLessEquals(panel.center.dx, epsilon: 1));
      expect(button.top, greaterThan(box.bottom));
      // Until the devices are listed, nothing can start (the rigs' pictures show the
      // listed "no camera").
      expect(_button(tester).onTap, isNull);
      expect(cameraPreview.on, isFalse);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    }
    tester.view.reset();
  });
}
