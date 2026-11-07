import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// The test binding, noting where each callback after the frame was asked for.
class _CountingBinding extends AutomatedTestWidgetsFlutterBinding {
  final asked = <String>[];

  @override
  void addPostFrameCallback(FrameCallback callback, {String debugLabel = ''}) {
    // The first frame of the application's own code that asked.
    final frame = StackTrace.current
        .toString()
        .split('\n')
        .firstWhere((l) => l.contains('package:fogata/'), orElse: () => '');
    asked.add(frame);
    super.addPostFrameCallback(callback, debugLabel: debugLabel);
  }
}

/// Builds and lays out every widget of the window again, without ending the frame: each
/// part, wherever its own build scope (a LayoutBuilder's) is.
void _buildAll(WidgetTester tester) {
  final all = <Element>[];
  void collect(Element e) {
    all.add(e);
    e.visitChildren(collect);
  }

  tester.binding.rootElement!.visitChildren(collect);
  for (final e in all) {
    // A LayoutBuilder's included: it builds again when laid out.
    e.markNeedsBuild();
  }
  final owner = tester.binding.buildOwner!;
  for (final e in all) {
    if (e.mounted) owner.buildScope(e);
  }
  tester.binding.rootPipelineOwner.flushLayout();
}

void main() {
  final binding = _CountingBinding();
  setUpAll(loadFonts);

  for (final (name, make) in [
    ('the home page', () => fakeRoom(people: 3)..nav.page = Screen.home),
    (
      'a room watching a screen, with its strip of people',
      () => fakeRoom(people: 6, sharing: 1, watching: true, cameras: 2),
    ),
    ('a room with cameras', () => fakeRoom(people: 6, cameras: 3)),
  ]) {
    testWidgets(
      '$name asks for work after the frame once per frame, however many builds it has',
      (tester) async {
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
        tester.view.physicalSize = const Size(1440, 900);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final state = make();
        await tester.pumpWidget(
          AppScope(
            state: state,
            child: MaterialApp(
              theme: Fg.darkTokens.themeData,
              home: const Material(child: Shell()),
            ),
          ),
        );
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        addTearDown(mouse.removePointer);
        await mouse.moveTo(const Offset(700, 400));
        await tester.pump(const Duration(milliseconds: 600));
        // Built once more: what asks for work after the frame does.
        _buildAll(tester);
        final first = binding.asked.length;
        expect(first, greaterThan(0));
        // Built again and again in the same frame: nothing more is asked for.
        for (var i = 0; i < 3; i++) {
          _buildAll(tester);
        }
        expect(binding.asked.sublist(first), isEmpty);
        await tester.pump();
        state.callState.call = null;
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
}
