// Each part of the state tells only what shows it: a toast, a line in the room's chat,
// what the call tells, a volume being dragged or a direct chat changing builds again what
// shows that, and not the whole window. Counted by the type of each widget built.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// The widgets counted, by the name of their type.
const _watched = [
  'Shell',
  'Rail',
  'SideColumn',
  '_PhaseWatch',
  '_Chat',
  '_Tiles',
  'Settings',
  'DirectPage',
  'Toasts',
];

/// How many times each of [_watched] builds while [change] happens and the frames after it
/// are drawn.
Future<Map<String, int>> _builds(
  WidgetTester tester,
  Future<void> Function() change,
) async {
  final builds = <String, int>{};
  debugOnRebuildDirtyWidget = (element, _) {
    final name = element.widget.runtimeType.toString();
    if (_watched.contains(name)) builds[name] = (builds[name] ?? 0) + 1;
  };
  try {
    await change();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
  } finally {
    debugOnRebuildDirtyWidget = null;
  }
  return builds;
}

Future<AppState> _shown(WidgetTester tester, {bool settings = false}) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1920, 1080);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 3);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
        builder: appFrame,
      ),
    ),
  );
  if (settings) state.openSettings(SettingsSection.voice);
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  // The pages ask the core for what a test does not have.
  tester.takeException();
  return state;
}

Future<void> _done(WidgetTester tester, AppState state) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
  state.dispose();
}

UiEvent _chatLine(String id) => UiEvent(
  kind: UiEventKind.room,
  group: 'g',
  voice: AppState.firstRoom,
  device: other(0).id,
  room: UiRoomChange(
    kind: UiRoomChangeKind.chat,
    device: other(0).id,
    onPurpose: false,
    chat: UiChatLine(
      id: id,
      sender: other(0).id,
      nick: other(0).name,
      text: 'hi',
      timestampMs: 0,
      mine: false,
    ),
    fileSize: 0,
    startedMs: 0,
  ),
  received: 0,
  total: 0,
);

void main() {
  setUpAll(loadFonts);

  testWidgets('in a room, each change builds only what shows it', (
    tester,
  ) async {
    final state = await _shown(tester);
    expect(find.byType(RoomPage), findsOneWidget);

    // What every change built before the state was told apart in parts.
    final whole = await _builds(tester, () async => state.notifyListeners());
    expect(whole['SideColumn'], greaterThan(0));
    expect(whole['_Chat'], greaterThan(0));

    final toast = await _builds(tester, () async => state.say('Saved'));
    final chat = await _builds(
      tester,
      () async => state.onEvent(_chatLine('1')),
    );
    final call = await _builds(
      tester,
      () async => state.callState.call!.notifyListeners(),
    );
    final volume = await _builds(
      tester,
      () => state.setVolume(other(0).id, 150, save: false),
    );
    final chats = await _builds(tester, () async => state.direct.changed());
    // ignore: avoid_print
    print(
      'builds in a room: whole $whole\n  toast $toast\n  chat line $chat\n'
      '  call $call\n  volume $volume\n  direct chats $chats',
    );
    // A toast builds only the toasts.
    expect(toast.keys, ['Toasts']);
    // A line in the room's chat builds the room, not the column beside it.
    expect(chat['_Chat'], greaterThan(0));
    expect(chat['SideColumn'], isNull);
    expect(chat['Rail'], isNull);
    // The call's own changes build the room's tiles, not the chat.
    expect(call['_Tiles'], greaterThan(0));
    expect(call['_Chat'], isNull);
    // Direct chats changing build nothing of a room.
    expect(chats['_PhaseWatch'], isNull);
    expect(chats['SideColumn'], isNull);
    await _done(tester, state);
  });

  testWidgets(
    'with the settings open, the room behind them does not hold them up',
    (tester) async {
      final state = await _shown(tester, settings: true);
      expect(find.byType(Settings), findsOneWidget);
      final chat = await _builds(
        tester,
        () async => state.onEvent(_chatLine('2')),
      );
      final call = await _builds(
        tester,
        () async => state.callState.call!.notifyListeners(),
      );
      final section = await _builds(
        tester,
        () async => state.showSettingsSection(SettingsSection.sounds),
      );
      // ignore: avoid_print
      print(
        'builds with the settings open: chat line $chat\n  call $call\n'
        '  section $section',
      );
      expect(chat['Settings'], isNull);
      expect(call['Settings'], isNull);
      expect(section['Settings'], greaterThan(0));
      expect(section['_Chat'], isNull);
      expect(section['_Tiles'], isNull);
      await _done(tester, state);
    },
  );

  testWidgets('a direct chat changing builds the chat, not the rest', (
    tester,
  ) async {
    final state = await _shown(tester);
    state.callState.call = null;
    state.roomState.room = null;
    state.nav.page = Screen.direct;
    state.notifyListeners();
    // The column's highlight moves once the frame after is drawn.
    for (var i = 0; i < 3; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    tester.takeException();
    expect(find.byType(DirectPage), findsOneWidget);
    final chats = await _builds(tester, () async => state.direct.changed());
    final chat = await _builds(
      tester,
      () async => state.onEvent(_chatLine('3')),
    );
    // ignore: avoid_print
    print(
      'builds on a direct chat: direct chats $chats\n  room chat line $chat',
    );
    expect(chats['DirectPage'], greaterThan(0));
    expect(chats['SideColumn'], isNull);
    expect(chat['DirectPage'], isNull);
    await _done(tester, state);
  });
}
