// What is written in a room's chat and not sent yet stays: through the files tab, another
// room, and a send that did not go.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// The room chat's message box.
final _box = find.descendant(
  of: find.byWidgetPredicate(
    (w) =>
        w.runtimeType.toString() == 'FgField' &&
        (w as dynamic).label == 'Message',
  ),
  matching: find.byType(EditableText),
);

String _written(WidgetTester tester) =>
    tester.widget<EditableText>(_box).controller.text;

Future<AppState> _show(WidgetTester tester) async {
  final state = fakeRoom(people: 2);
  tester.view.physicalSize = const Size(1920, 1080);
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
  await _settle(tester);
  return state;
}

Future<void> _done(WidgetTester tester) async {
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

  testWidgets('a message being written is still there after the files tab', (
    tester,
  ) async {
    await _show(tester);
    await tester.enterText(_box, 'Half a thought');
    await _settle(tester);
    await tester.tap(find.text('Files'));
    await _settle(tester);
    expect(_box, findsNothing);
    await tester.tap(find.text('Chat'));
    await _settle(tester);
    expect(_written(tester), 'Half a thought');
    expect(tester.takeException(), isNull);
    await _done(tester);
  });

  testWidgets('each room keeps its own draft', (tester) async {
    final state = await _show(tester);
    await tester.enterText(_box, 'For the first room');
    await _settle(tester);
    state.roomState.roomVoice = 'another-voice-room';
    state.roomState.changed();
    await _settle(tester);
    expect(_written(tester), '');
    await tester.enterText(_box, 'For the second');
    await _settle(tester);
    state.roomState.roomVoice = AppState.firstRoom;
    state.roomState.changed();
    await _settle(tester);
    expect(_written(tester), 'For the first room');
    expect(tester.takeException(), isNull);
    await _done(tester);
  });

  testWidgets('a message that did not go is back in the box', (tester) async {
    // No core in tests: writing in the room fails, as it does with the room gone.
    final state = await _show(tester);
    await tester.enterText(_box, 'Lost?');
    await tester.tap(find.byTooltip('Send'));
    await _settle(tester);
    // It was sent, and it did not go.
    expect(state.toasts.list.where((t) => t.error), hasLength(1));
    expect(_written(tester), 'Lost?');
    expect(state.roomState.drafts.values, ['Lost?']);
    expect(tester.takeException(), isNull);
    await _done(tester);
  });

  testWidgets(
    'a message that did not go while its chat was closed waits there',
    (tester) async {
      final state = await _show(tester);
      await tester.tap(find.text('Files'));
      await _settle(tester);
      expect(await state.sendChat('Sent while away'), isFalse);
      expect(
        state.roomState.drafts[state.roomState.draftHere],
        'Sent while away',
      );
      await tester.tap(find.text('Chat'));
      await _settle(tester);
      expect(_written(tester), 'Sent while away');
      await _done(tester);
    },
  );
}
