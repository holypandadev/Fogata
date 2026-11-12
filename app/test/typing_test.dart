import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/typing.dart';

const _group = 'aaaa';
const _voice = AppState.firstRoom;

UiEvent _typing(String device, {bool on = true, String? group = _group}) =>
    UiEvent(
      kind: on ? UiEventKind.typing : UiEventKind.stoppedTyping,
      device: device,
      group: group,
      voice: group == null ? null : _voice,
      received: 0,
      total: 0,
    );

UiEvent _room(UiRoomChangeKind kind, String device) => UiEvent(
  kind: UiEventKind.room,
  group: _group,
  voice: _voice,
  room: UiRoomChange(
    kind: kind,
    device: device,
    onPurpose: true,
    fileSize: 0,
    startedMs: 0,
  ),
  received: 0,
  total: 0,
);

Widget _app(Widget child) => MaterialApp(
  theme: Fg.darkTokens.themeData,
  home: Scaffold(body: child),
);

void main() {
  test(
    'someone shows until a while after their last signal, or until they stop',
    () {
      final board = TypingBoard();
      const chat = 'room:g:v';
      board.heard(chat, 'ana', 1000);
      board.heard(chat, 'leo', 2000);
      expect(board.who(chat, 2000), ['ana', 'leo']);
      expect(board.nextEnd, 1000 + TypingBoard.showFor.inMilliseconds);
      // Ana again: she keeps her place and her time starts over.
      board.heard(chat, 'ana', 4000);
      expect(board.who(chat, 7500), ['ana', 'leo']);
      expect(board.who(chat, 8500), ['ana']);
      expect(board.prune(8500), isTrue);
      expect(board.prune(8500), isFalse);
      board.stopped(chat, 'ana');
      expect(board.who(chat, 8500), isEmpty);
      expect(board.isEmpty, isTrue, reason: 'nothing is kept of an empty chat');
      expect(board.nextEnd, isNull);
    },
  );

  test('someone who goes shows in no chat; a closed room shows nobody', () {
    final board = TypingBoard();
    board.heard('dm:ana', 'ana', 0);
    board.heard('room:g:v', 'ana', 0);
    board.heard('room:g:v', 'leo', 0);
    board.gone('ana');
    expect(board.who('dm:ana', 1), isEmpty);
    expect(board.who('room:g:v', 1), ['leo']);
    board.clear('room:g:v');
    expect(board.isEmpty, isTrue);
  });

  test(
    'typing goes when the text changes, at most every 3 s, and stops once',
    () {
      final sent = <bool>[];
      final sender = TypingSender(sent.add);
      sender.changed('h', nowMs: 0);
      sender.changed('ho', nowMs: 1000);
      sender.changed('hol', nowMs: 2999);
      expect(sent, [true]);
      sender.changed('hello', nowMs: 3000);
      expect(sent, [true, true]);
      // The same text (the cursor moved) is no typing.
      sender.changed('hello', nowMs: 9000);
      expect(sent, [true, true]);
      // Spaces only are nothing to send: stopped.
      sender.changed('   ', nowMs: 9100);
      expect(sent, [true, true, false]);
      sender.changed('', nowMs: 9200);
      sender.stopped();
      expect(sent, [true, true, false], reason: 'one "stopped" is enough');
      // Typing again goes at once.
      sender.changed('y', nowMs: 9300);
      expect(sent, [true, true, false, true]);
      sender.stopped();
      expect(sent, [true, true, false, true, false]);
    },
  );

  test('an empty box never says typing', () {
    final sent = <bool>[];
    final sender = TypingSender(sent.add);
    sender.changed('', nowMs: 0);
    sender.changed(' ', nowMs: 5000);
    sender.stopped();
    expect(sent, isEmpty);
  });

  test('one, two or several people', () {
    const bold = TextStyle();
    expect(typingText(['Ana'], bold).toPlainText(), 'Ana is typing…');
    expect(
      typingText(['Ana', 'Leo'], bold).toPlainText(),
      'Ana and Leo are typing…',
    );
    expect(
      typingText(['Ana', 'Leo', 'Bea'], bold).toPlainText(),
      'Several people are typing…',
    );
  });

  test('signals of the core show and go in the app', () {
    final state = AppState('/nonexistent/fogata-typing-test');
    String room() => TypingBoard.room(_group, _voice);
    int now() => DateTime.now().millisecondsSinceEpoch;
    state.onEvent(_typing('ana'));
    state.onEvent(_typing('leo'));
    expect(state.typing.who(room(), now()), ['ana', 'leo']);
    // Leo's message arrives: he no longer shows; Ana says she stopped.
    state.onEvent(_room(UiRoomChangeKind.chat, 'leo'));
    expect(state.typing.who(room(), now()), ['ana']);
    state.onEvent(_typing('ana', on: false));
    expect(state.typing.who(room(), now()), isEmpty);
    // Leaving the room, or going away, ends it too.
    state.onEvent(_typing('bea'));
    state.onEvent(_room(UiRoomChangeKind.left, 'bea'));
    expect(state.typing.who(room(), now()), isEmpty);

    // A friend in a direct chat: until the message comes, or the friend goes.
    state.onEvent(_typing('marta', group: null));
    expect(state.direct.typingHere('marta'), isTrue);
    state.onEvent(
      const UiEvent(
        kind: UiEventKind.directMessage,
        device: 'marta',
        text: 'hello',
        received: 0,
        total: 0,
      ),
    );
    expect(state.direct.typingHere('marta'), isFalse);
    state.onEvent(_typing('marta', group: null));
    state.onEvent(
      const UiEvent(
        kind: UiEventKind.friendOffline,
        device: 'marta',
        received: 0,
        total: 0,
      ),
    );
    expect(state.direct.typingHere('marta'), isFalse);
    state.onEvent(_typing('marta', group: null));
    state.onEvent(
      const UiEvent(
        kind: UiEventKind.memberDisconnected,
        group: 'chat',
        device: 'marta',
        received: 0,
        total: 0,
      ),
    );
    expect(state.direct.typingHere('marta'), isFalse);
    expect(state.typing.isEmpty, isTrue);
    state.dispose();
  });

  test('this device appearing offline sends no typing', () {
    final state = AppState('/nonexistent/fogata-typing-test')
      ..started = true
      ..prefs.appearOffline = true
      ..roomState.roomGroup = _group;
    // Would throw without the library loaded if it tried to send.
    state.roomTyping(true);
    state.roomTyping(false);
    state.direct.typing('marta', true);
    state.direct.typing('marta', false);
    state.prefs.appearOffline = false;
    expect(() => state.roomTyping(true), throwsStateError, reason: 'a send');
    expect(() => state.direct.typing('marta', true), throwsStateError);
    state.dispose();
  });

  testWidgets('the line under the chat: names, dots, and the same height', (
    tester,
  ) async {
    Future<Size> show(List<String> names) async {
      await tester.pumpWidget(
        _app(
          Center(
            child: SizedBox(width: 400, child: TypingLine(names: names)),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 400));
      return tester.getSize(find.byType(TypingLine));
    }

    final empty = await show(const []);
    expect(find.byKey(const ValueKey('typing-line')), findsNothing);
    expect(find.byType(TypingDots), findsNothing);
    final one = await show(const ['Ana']);
    expect(find.text('Ana is typing…', findRichText: true), findsOneWidget);
    expect(find.byType(TypingDots), findsOneWidget);
    expect(one, empty, reason: 'the messages above never jump');
    await show(const ['Ana', 'Leo']);
    expect(
      find.text('Ana and Leo are typing…', findRichText: true),
      findsOneWidget,
    );
    await show(const ['Ana', 'Leo', 'Bea']);
    expect(
      find.text('Several people are typing…', findRichText: true),
      findsOneWidget,
    );
    // A very long name is cut, not wrapped.
    await show(['Ana ${'de la Fuente ' * 20}']);
    expect(tester.takeException(), isNull);
    expect(tester.getSize(find.byType(TypingLine)).height, TypingLine.height);
    // The dots keep moving.
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pumpWidget(const SizedBox());
  });

  test('someone typing leaves the time outs being watched', () {
    final now = DateTime.now().millisecondsSinceEpoch;
    final state = AppState('/nonexistent/fogata-typing-test')
      ..contacts.groups = [
        UiGroup(
          id: _group,
          name: 'Call',
          localName: '',
          icon: 0,
          members: 2,
          online: 2,
          manager: false,
          rank: 1,
          maxMembers: 20,
          maxInRoom: 8,
          voiceRooms: [
            UiVoiceRoom(
              id: _voice,
              name: 'General',
              barrier: UiBarrier(
                kind: UiBarrierKind.timedOut,
                by: 'ana',
                sinceMs: now,
                untilMs: now + 10 * 60 * 1000,
              ),
            ),
          ],
          openInvites: 0,
          needsUpdate: false,
          outranks: const [],
        ),
      ];
    state.timeOutsTick();
    expect(state.watchingTimeOuts, isTrue);
    state.onEvent(_typing('ana'));
    state.onEvent(_typing('ana', on: false));
    expect(state.watchingTimeOuts, isTrue);
    state.dispose();
  });
}
