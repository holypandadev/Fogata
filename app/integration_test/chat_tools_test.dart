// Pictures of the emoji picker in a direct chat and in a room's chat, the ":name"
// suggestions, and the quick switcher, in both themes, at the smallest window Fogata
// allows and on a large screen, with the system's own emoji font. Nothing connects.
//
//   FOGATA_SHOTS=<folder> flutter test integration_test/chat_tools_test.dart -d linux
//
// Under FONTCONFIG_FILE with no colour emoji font the picker says so in its foot.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:integration_test/integration_test.dart';

import '../test/fake_room.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _shot(WidgetTester tester, String name) async {
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(
    Offset.zero & (view.size * tester.view.devicePixelRatio),
  );
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

const _sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);
const _leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB1C·D1EF',
  avatar: UiAvatar(preset: 3),
);
const _ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: 'AB2C·D2EF',
  avatar: UiAvatar(preset: 4),
);

final _now = DateTime.now().millisecondsSinceEpoch;

UiDirectItem _line(String id, String text, {bool mine = false, int ago = 0}) =>
    UiDirectItem(
      id: id,
      seq: 0,
      mine: mine,
      atMs: _now - ago * 60000,
      text: text,
      files: const [],
      delivered: true,
      expiresMs: 0,
      pinned: false,
    );

/// A state with groups, friends and a chat with Sofía, on [page].
AppState _state(Screen page) {
  final room = fakeRoom(people: 3);
  final state = page == Screen.room
      ? room
      : (AppState('/nonexistent/fogata-chat-tools')
          ..contacts.profile = room.contacts.profile
          ..contacts.groups = [
            room.contacts.groups.first,
            UiGroup(
              id: 'g2',
              name: 'Book club',
              localName: '',
              icon: 3,
              members: 6,
              online: 2,
              manager: false,
              rank: 1,
              outranks: const [],
              maxMembers: 20,
              maxInRoom: 8,
              voiceRooms: const [
                UiVoiceRoom(id: AppState.firstRoom, name: ''),
                UiVoiceRoom(id: 'quiet', name: 'Quiet corner'),
              ],
              openInvites: 0,
              needsUpdate: false,
              raiseAnyway: false,
            ),
          ]
          ..nav.page = page);
  state
    ..contacts.friends = [
      for (final p in [_sofia, _leo, _ana])
        UiFriend(
          person: p,
          fingerprint: p.short,
          presence: p == _ana ? UiPresence.offline : UiPresence.online,
          delivered: true,
        ),
    ]
    ..prefs.recentEmoji = ['👍', '😂', '❤️', '🎉', '🔥', '👋'];
  state.direct.chats = [
    UiDirectChat(
      away: false,
      friend: _sofia,
      online: true,
      last: 'See you at 8 🎉',
      lastMine: false,
      lastMs: _now,
      unread: 0,
      timerMs: 0,
    ),
    UiDirectChat(
      away: false,
      friend: _leo,
      online: true,
      last: 'ok',
      lastMine: true,
      lastMs: _now - 3600000,
      unread: 2,
      timerMs: 0,
    ),
  ];
  if (page == Screen.direct) {
    state.direct
      ..open = _sofia.id
      ..page = UiDirectPage(
        away: false,
        friend: _sofia,
        version: 0,
        online: true,
        connected: true,
        items: [
          _line('1', 'Hey 👋 are we still on for tonight?', ago: 12),
          _line('2', 'Yes! See you at 8 🎉🔥', mine: true, ago: 10),
          _line('3', '❤️ ☺️ ✔️ 🇪🇸 👍🏽 👩🏽‍💻', ago: 2),
        ],
        more: false,
        pinned: const [],
        timerMs: 0,
        usage: const UiDirectUsage(
          messages: 4000,
          pictures: 0,
          videos: 0,
          files: 0,
        ),
        altered: false,
      );
  }
  if (page == Screen.room) {
    state.roomState.room = UiRoom(
      startedMs: _now - 900000,
      joined: true,
      participants: state.roomState.room!.participants,
      chat: [
        UiChatLine(
          id: 'c1',
          sender: 'p0',
          nick: 'Leo',
          text: 'gg 🔥🔥',
          timestampMs: _now - 60000,
          mine: false,
        ),
        UiChatLine(
          id: 'c2',
          sender: 'me',
          nick: 'Marta',
          text: 'One more? 🎮',
          timestampMs: _now - 30000,
          mine: true,
        ),
      ],
      files: const [],
    );
  }
  return state;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('emoji picker and quick switcher', (tester) async {
    final problems = <String>[];
    void check(String what) {
      final e = tester.takeException();
      if (e != null) problems.add('$what: $e');
    }

    for (final (theme, dark) in [
      (Fg.darkTokens, 'dark'),
      (Fg.lightTokens, 'light'),
    ]) {
      for (final size in const [Size(960, 640), Size(1920, 1080)]) {
        final at = '$dark-${size.width.round()}x${size.height.round()}';
        tester.view.physicalSize = size;
        tester.view.devicePixelRatio = 1;
        Future<AppState> show(Screen page) async {
          final state = _state(page);
          await tester.pumpWidget(
            AppScope(
              state: state,
              child: MaterialApp(
                theme: theme.themeData,
                home: const Material(child: Shell()),
              ),
            ),
          );
          await _wait(tester, const Duration(milliseconds: 600));
          return state;
        }

        // A direct chat: the picker, the skin tones, the ":name" suggestions.
        await show(Screen.direct);
        await _shot(tester, 'dm-$at');
        await tester.tap(find.byKey(const ValueKey('emoji-button')));
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'dm-picker-$at');
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await _wait(tester, const Duration(milliseconds: 300));
        await _shot(tester, 'dm-picker-keys-$at');
        await tester.tap(find.byKey(const ValueKey('emoji-tone')));
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'dm-tones-$at');
        await tester.tap(find.byKey(const ValueKey('tone-4')));
        await _wait(tester, const Duration(milliseconds: 300));
        await tester.enterText(
          find.byKey(const ValueKey('emoji-search')),
          'hand',
        );
        await _wait(tester, const Duration(milliseconds: 400));
        await _shot(tester, 'dm-search-$at');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _wait(tester, const Duration(milliseconds: 400));
        await tester.enterText(
          find.byKey(const ValueKey('direct-message')),
          'so good :smi',
        );
        await _wait(tester, const Duration(milliseconds: 500));
        await _shot(tester, 'dm-complete-$at');
        check('direct chat at $at');

        // A room's chat.
        final room = await show(Screen.room);
        // A small window starts the call with its chat hidden: shown by hand.
        await room.setRoomChat(true);
        await _wait(tester, const Duration(milliseconds: 600));
        await tester.tap(find.byKey(const ValueKey('emoji-button')));
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'room-picker-$at');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _wait(tester, const Duration(milliseconds: 300));
        check('room at $at');
        room.callState.call = null;

        // The quick switcher from home: nothing typed, and something typed.
        await show(Screen.home);
        await _shot(tester, 'home-$at');
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'switcher-$at');
        await tester.enterText(
          find.byKey(const ValueKey('switcher-field')),
          'b',
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await _wait(tester, const Duration(milliseconds: 400));
        await _shot(tester, 'switcher-typed-$at');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _wait(tester, const Duration(milliseconds: 300));
        check('switcher at $at');
        await tester.pumpWidget(const SizedBox());
        // Toasts of the core calls that tests do not have go by themselves.
        await _wait(tester, const Duration(milliseconds: 200));
      }
    }
    expect(problems, isEmpty, reason: problems.join('\n'));
  });
}
