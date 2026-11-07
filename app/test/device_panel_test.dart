import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/popover.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// The devices the system lists; [mics] microphones beside them.
List<Map<String, Object?>> _sources({int mics = 3}) => [
  for (var i = 0; i < mics; i++)
    {'deviceId': 'mic-$i', 'label': 'Microphone $i', 'kind': 'audioinput'},
  {'deviceId': 'quiet', 'label': 'Fogata silence', 'kind': 'audioinput'},
  {'deviceId': 'out-1', 'label': 'Speakers', 'kind': 'audiooutput'},
  {'deviceId': 'out-2', 'label': 'Headphones', 'kind': 'audiooutput'},
  {'deviceId': 'cam-1', 'label': 'Webcam', 'kind': 'videoinput'},
];

Future<AppState> _show(
  WidgetTester tester,
  Size size, {
  bool room = true,
  int mics = 3,
}) async {
  final messenger = tester.binding.defaultBinaryMessenger;
  // The window's buttons ask the window manager, which tests do not have.
  messenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  messenger.setMockMethodCallHandler(
    const MethodChannel('FlutterWebRTC.Method'),
    (call) async =>
        call.method == 'getSources' ? {'sources': _sources(mics: mics)} : null,
  );
  messenger.setMockMethodCallHandler(
    const MethodChannel('FlutterWebRTC.Event'),
    (call) async => null,
  );
  addTearDown(() {
    for (final name in [
      'window_manager',
      'FlutterWebRTC.Method',
      'FlutterWebRTC.Event',
    ]) {
      messenger.setMockMethodCallHandler(MethodChannel(name), null);
    }
  });
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 3)..prefs.audioInput = 'mic-1';
  if (!room) {
    state
      ..callState.call = null
      ..roomState.room = null
      ..roomState.roomGroup = null
      ..nav.selectedGroup = null
      ..nav.page = Screen.home;
  }
  await tester.pumpWidget(const SizedBox());
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
  return state;
}

Finder _panel(DeviceKind kind) =>
    find.byKey(ValueKey('device-panel:${kind.name}'));

double _turns(WidgetTester tester, Finder arrow) => tester
    .widget<AnimatedRotation>(
      find.descendant(of: arrow, matching: find.byType(AnimatedRotation)),
    )
    .turns;

/// Lets a panel finish opening or closing.
Future<void> _settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
  await tester.pump();
}

Future<void> _open(WidgetTester tester, Finder arrow) async {
  await tester.tap(arrow);
  // The listing comes back, then the spring settles.
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

/// Checks the panel of [kind] hangs over [anchor], [gap] above it, and stays inside the
/// window.
void _expectOver(
  WidgetTester tester,
  DeviceKind kind,
  Rect anchor,
  double gap,
  Size window,
) {
  final panel = tester.getRect(_panel(kind));
  expect(panel.bottom, moreOrLessEquals(anchor.top - gap, epsilon: .5));
  expect(panel.left, greaterThanOrEqualTo(popoverMargin - .5));
  expect(panel.top, greaterThanOrEqualTo(popoverMargin - .5));
  expect(panel.right, lessThanOrEqualTo(window.width - popoverMargin + .5));
  // Its left edge on the anchor's, unless the window's edge is closer.
  final left = anchor.left.clamp(
    popoverMargin,
    window.width - panel.width - popoverMargin,
  );
  expect(panel.left, moreOrLessEquals(left, epsilon: .5));
}

void main() {
  setUpAll(loadFonts);

  for (final size in const [Size(960, 640), Size(1920, 1080)]) {
    testWidgets(
      'each arrow opens its own panel over its button, ${size.width}',
      (tester) async {
        await _show(tester, size);
        expect(find.text('Microphone, speaker and camera'), findsNothing);
        for (final kind in DeviceKind.values) {
          final arrows = find.byTooltip(DeviceArrow.tooltip(kind));
          // The pill's (microphone and speaker) come first, then the control bar's.
          expect(arrows, findsNWidgets(kind == DeviceKind.camera ? 1 : 2));
          final bar = arrows.last;
          final pill = arrows.first;
          final sources = [
            (
              bar,
              // The pill of the button and its arrow, around the arrow.
              () => tester.getRect(
                find
                    .ancestor(of: bar, matching: find.byType(AnimatedContainer))
                    .first,
              ),
              16.0,
            ),
            if (kind != DeviceKind.camera)
              (
                pill,
                () => tester.getRect(find.byKey(const ValueKey('own-pill'))),
                8.0,
              ),
          ];
          for (final (arrow, anchor, gap) in sources) {
            expect(_turns(tester, arrow), 0);
            await _open(tester, arrow);
            expect(_panel(kind), findsOneWidget);
            for (final other in DeviceKind.values) {
              if (other != kind) expect(_panel(other), findsNothing);
            }
            // It points up while open.
            expect(_turns(tester, arrow), .5);
            _expectOver(tester, kind, anchor(), gap, size);
            // A click beside it closes it, and the arrow turns back.
            await tester.tapAt(const Offset(700, 120));
            await _settle(tester);
            expect(_panel(kind), findsNothing);
            expect(_turns(tester, arrow), 0);
          }
        }
        // Never started, so nothing to stop.
        AppScope.read(tester.element(find.byType(Shell))).callState.call = null;
      },
    );
  }

  testWidgets('each panel lists only its own devices', (tester) async {
    await _show(tester, const Size(1280, 800));
    final expected = {
      DeviceKind.input: ['Microphone 0', 'Microphone 1', 'Microphone 2'],
      DeviceKind.output: ['Speakers', 'Headphones'],
      DeviceKind.camera: ['Webcam'],
    };
    for (final kind in DeviceKind.values) {
      await _open(tester, find.byTooltip(DeviceArrow.tooltip(kind)).last);
      final panel = _panel(kind);
      Finder inPanel(String text) =>
          find.descendant(of: panel, matching: find.text(text));
      expect(inPanel('System default'), findsOneWidget);
      for (final (other, names) in expected.entries.map(
        (e) => (e.key, e.value),
      )) {
        for (final name in names) {
          expect(inPanel(name), other == kind ? findsOneWidget : findsNothing);
        }
      }
      expect(inPanel('Fogata silence'), findsNothing);
      expect(
        inPanel('Noise suppression'),
        kind == DeviceKind.input ? findsOneWidget : findsNothing,
      );
      expect(inPanel('Voice & video settings'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await _settle(tester);
      expect(panel, findsNothing);
    }
    AppScope.read(tester.element(find.byType(Shell))).callState.call = null;
  });

  testWidgets('a choice applies, and the settings open from the panel', (
    tester,
  ) async {
    final state = await _show(tester, const Size(1280, 800), room: false);
    final arrow = find.byTooltip(DeviceArrow.tooltip(DeviceKind.input));
    await _open(tester, arrow);
    final handle = tester.ensureSemantics();
    await tester.pump();
    expect(
      tester.getSemantics(find.bySemanticsLabel(RegExp(r'^Microphone 1\b'))),
      isSemantics(isSelected: true),
    );
    handle.dispose();
    await tester.tap(find.text('Microphone 2'));
    await tester.pump();
    expect(state.prefs.audioInput, 'mic-2');
    await tester.tap(find.text('Voice & video settings'));
    await _settle(tester);
    expect(_panel(DeviceKind.input), findsNothing);
    expect(state.nav.settingsOpen, isTrue);
    expect(state.nav.settingsSection, SettingsSection.voice);
  });

  testWidgets('the keyboard opens, goes through and closes a panel', (
    tester,
  ) async {
    await _show(tester, const Size(1280, 800), room: false);
    final handle = tester.ensureSemantics();
    final arrow = find.byTooltip(DeviceArrow.tooltip(DeviceKind.output));
    final said = find.bySemanticsLabel(DeviceArrow.tooltip(DeviceKind.output));
    Focus.of(
      tester.element(
        find.descendant(of: arrow, matching: find.byType(AnimatedRotation)),
      ),
    ).requestFocus();
    await tester.pump();
    final arrowFocus = FocusManager.instance.primaryFocus;
    expect(
      tester.getSemantics(said),
      isSemantics(isButton: true, hasExpandedState: true, isExpanded: false),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    final panel = _panel(DeviceKind.output);
    expect(panel, findsOneWidget);
    // Under the panel the page is hidden from screen readers; the arrow says it is open
    // once they reach it again.
    expect(
      tester
          .widget<Pressable>(
            find.ancestor(of: arrow, matching: find.byType(Pressable)).first,
          )
          .expanded,
      isTrue,
    );
    // Tab goes into the panel and stays there.
    for (var i = 0; i < 8; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final at = FocusManager.instance.primaryFocus!.context!;
      expect(
        find.descendant(of: panel, matching: find.byWidget(at.widget)),
        findsWidgets,
        reason: 'Tab $i',
      );
    }
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(panel, findsNothing);
    // The focus is back on the arrow, which says it is closed.
    expect(FocusManager.instance.primaryFocus, arrowFocus);
    expect(
      tester.getSemantics(said),
      isSemantics(hasExpandedState: true, isExpanded: false),
    );
    handle.dispose();
  });

  testWidgets('a panel with more devices than room scrolls inside the window', (
    tester,
  ) async {
    const size = Size(960, 640);
    await _show(tester, size, mics: 40);
    await _open(
      tester,
      find.byTooltip(DeviceArrow.tooltip(DeviceKind.input)).last,
    );
    final panel = tester.getRect(_panel(DeviceKind.input));
    expect(panel.top, greaterThanOrEqualTo(popoverMargin - .5));
    expect(panel.bottom, lessThanOrEqualTo(size.height - popoverMargin));
    expect(tester.takeException(), isNull);
    // The last microphone is reached by scrolling.
    await tester.scrollUntilVisible(
      find.text('Microphone 39'),
      200,
      scrollable: find.descendant(
        of: _panel(DeviceKind.input),
        matching: find.byType(Scrollable),
      ),
    );
    expect(find.text('Microphone 39'), findsOneWidget);
    AppScope.read(tester.element(find.byType(Shell))).callState.call = null;
  });

  group('a popover stays inside the window', () {
    const window = Size(960, 640);
    Rect place(PopoverPlace p, Size child) {
      final box = p.getConstraintsForChild(BoxConstraints.loose(window));
      final fitted = box.constrain(child);
      return p.getPositionForChild(window, fitted) & fitted;
    }

    void inside(Rect r) {
      expect(r.left, greaterThanOrEqualTo(popoverMargin));
      expect(r.top, greaterThanOrEqualTo(popoverMargin));
      expect(r.right, lessThanOrEqualTo(window.width - popoverMargin));
      expect(r.bottom, lessThanOrEqualTo(window.height - popoverMargin));
    }

    test('above a button at the bottom, its left edge on the button', () {
      final r = place(
        PopoverPlace(const Rect.fromLTWH(400, 580, 60, 40), above: true),
        const Size(300, 200),
      );
      expect(r.left, 400);
      expect(r.bottom, 580 - 8);
      inside(r);
    });

    test('at the right edge, moved in', () {
      final r = place(
        PopoverPlace(const Rect.fromLTWH(900, 580, 40, 40), above: true),
        const Size(300, 200),
      );
      expect(r.right, window.width - popoverMargin);
      inside(r);
    });

    test('right-aligned under a button near the left edge, moved in', () {
      final r = place(
        PopoverPlace(
          const Rect.fromLTWH(20, 100, 32, 32),
          above: false,
          alignRight: true,
        ),
        const Size(260, 150),
      );
      expect(r.left, popoverMargin);
      expect(r.top, 132 + 8);
      inside(r);
    });

    test('taller than the room above: only as tall as that room', () {
      final r = place(
        PopoverPlace(const Rect.fromLTWH(400, 580, 60, 40), above: true),
        const Size(300, 2000),
      );
      expect(r.height, 580 - 8 - popoverMargin);
      inside(r);
    });

    test('wider than the window: as wide as the window lets it', () {
      final r = place(
        PopoverPlace(const Rect.fromLTWH(400, 300, 60, 40), above: false),
        const Size(2000, 100),
      );
      expect(r.width, window.width - 2 * popoverMargin);
      inside(r);
    });

    test('with almost no room on either side, still inside', () {
      final r = place(
        PopoverPlace(const Rect.fromLTWH(400, 10, 60, 620), above: false),
        const Size(300, 300),
      );
      inside(r);
    });
  });
}
