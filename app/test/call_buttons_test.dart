import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

UiFriend friend(UiPerson person, {String? room}) => UiFriend(
  person: person,
  fingerprint: '${person.short} 9PLD 3VHC',
  presence: room == null ? UiPresence.online : UiPresence.inRoom,
  roomGroup: room,
  roomGroupName: room == null ? null : 'Friday games',
  delivered: true,
);

Future<void> show(WidgetTester tester, AppState state) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
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
}

FgButton button(WidgetTester tester, Finder finder) =>
    tester.widget<FgButton>(finder);

void main() {
  setUpAll(loadFonts);

  setUp(() {
    // The window's buttons ask the window manager, which tests do not have.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets('a friend in the same voice room is not rung', (tester) async {
    // Leo is in the room with this device; Bruno, a friend too, is not.
    final leo = other(0);
    final bruno = other(3);
    final state = fakeRoom(people: 3)
      ..contacts.friends = [friend(leo, room: 'g'), friend(bruno)]
      ..nav.page = Screen.friends;
    expect(state.sharedRoom(leo.id), 'Friday games');
    expect(state.sharedRoom(bruno.id), isNull);
    await show(tester, state);

    final rows = find.byKey(ValueKey('friend-row:${leo.id}'));
    final leoCall = find.descendant(
      of: rows,
      matching: find.widgetWithText(FgButton, 'Call'),
    );
    expect(button(tester, leoCall).onTap, isNull);
    expect(button(tester, leoCall).tooltip, 'You’re both in Friday games.');
    final brunoCall = find.descendant(
      of: find.byKey(ValueKey('friend-row:${bruno.id}')),
      matching: find.widgetWithText(FgButton, 'Call'),
    );
    expect(button(tester, brunoCall).onTap, isNotNull);

    // The friend's card says why, under its Call button.
    state
      ..nav.friendCard = leo.id
      ..nav.friendCardFrom = const Rect.fromLTWH(600, 200, 400, 60);
    state.notifyListeners();
    await tester.pump(const Duration(milliseconds: 500));
    final cardCall = find.byKey(const ValueKey('friend-card-call'));
    expect(button(tester, cardCall).onTap, isNull);
    expect(find.text('You’re both in Friday games.'), findsOneWidget);

    // Any other way in (a missed call's Call back, say) is turned down the same way.
    final before = state.toasts.list.length;
    await state.callFriend(leo.id);
    expect(state.toasts.list.skip(before).map((t) => t.text), [
      'You’re both in Friday games.',
    ]);
    expect(state.callState.activeCall, isNull);
    await tester.pump(const Duration(seconds: 8));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('the missed call note goes by itself, and the bell keeps it', (
    tester,
  ) async {
    final leo = other(0);
    final state = fakeRoom(people: 2)..contacts.friends = [friend(leo)];
    void miss() {
      state
        ..notices.missedCalls[leo.id] = DateTime.now().millisecondsSinceEpoch
        ..notices.missedNote = leo.id;
      // As a missed call does: the note's time starts.
      state.notices.holdMissedNote(false);
      state.notifyListeners();
    }

    final note = find.textContaining('Missed call from Leo');
    bool missedInBell() =>
        state.notices.list.any((n) => n.kind == NoticeKind.missedCall);

    miss();
    await show(tester, state);
    expect(note, findsOneWidget);
    await tester.pump(Notices.missedNoteFor - const Duration(seconds: 1));
    expect(note, findsOneWidget);
    await tester.pump(const Duration(seconds: 2));
    await tester.pump(const Duration(milliseconds: 400));
    expect(note, findsNothing);
    expect(state.notices.missedNote, isNull);
    expect(missedInBell(), isTrue);

    // A click on it takes it away too.
    miss();
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(note);
    await tester.pump(const Duration(milliseconds: 400));
    expect(note, findsNothing);
    expect(missedInBell(), isTrue);

    // It stays while the pointer is on it.
    miss();
    await tester.pump(const Duration(milliseconds: 500));
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: tester.getCenter(note));
    await tester.pump();
    await tester.pump(Notices.missedNoteFor * 2);
    expect(note, findsOneWidget);
    await mouse.moveTo(Offset.zero);
    await tester.pump(Notices.missedNoteFor + const Duration(seconds: 1));
    await tester.pump(const Duration(milliseconds: 400));
    expect(note, findsNothing);
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'a friend who rang during a call says so, in the corner and the bell',
    (tester) async {
      final leo = other(0);
      final state = fakeRoom(people: 2)..contacts.friends = [friend(leo)];
      state
        ..notices.missedCalls[leo.id] = DateTime.now().millisecondsSinceEpoch
        ..notices.missedInCall.add(leo.id)
        ..notices.missedNote = leo.id;
      state.notices.holdMissedNote(false);
      await show(tester, state);
      expect(find.text('Leo called while you were in a call'), findsOneWidget);
      final notice = state.notices.list.singleWhere(
        (n) => n.kind == NoticeKind.missedCall,
      );
      expect(
        state.notices.missedCallTitle(notice.person!),
        'Leo called while you were in a call',
      );
      // A later call missed the usual way says so instead.
      state.notices.missedInCall.remove(leo.id);
      state.notifyListeners();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.text('Missed call from Leo'), findsOneWidget);
      await tester.pump(Notices.missedNoteFor + const Duration(seconds: 1));
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('answering in a room this device is alone in asks first', (
    tester,
  ) async {
    final leo = other(0);
    final state = fakeRoom(people: 1)
      ..contacts.friends = [friend(leo)]
      ..callState.calls = [
        UiCall(
          groupId: 'c',
          with_: leo,
          outgoing: false,
          phase: UiCallPhase.incoming,
          sinceMs: DateTime.now().millisecondsSinceEpoch,
          members: [leo],
          adding: const [],
          away: const [],
        ),
      ];
    await show(tester, state);
    expect(find.text('Answering leaves Friday games'), findsOneWidget);
    await tester.tap(find.text('Leave and answer'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Leave and close the room?'), findsOneWidget);
    // Staying keeps the room, and the call rings on.
    await tester.tap(find.text('Stay'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Leave and close the room?'), findsNothing);
    expect(state.callState.incomingCall?.groupId, 'c');
    expect(state.contacts.joinedGroup?.id, 'g');
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('calling someone while a friend rings asks before declining', (
    tester,
  ) async {
    final leo = other(0);
    final bruno = other(3);
    final state = fakeRoom(people: 3)
      ..contacts.friends = [friend(leo), friend(bruno)]
      ..nav.page = Screen.friends
      ..callState.calls = [
        UiCall(
          groupId: 'c',
          with_: leo,
          outgoing: false,
          phase: UiCallPhase.incoming,
          sinceMs: DateTime.now().millisecondsSinceEpoch,
          members: [leo],
          adding: const [],
          away: const [],
        ),
      ];
    await show(tester, state);
    final brunoCall = find.descendant(
      of: find.byKey(ValueKey('friend-row:${bruno.id}')),
      matching: find.widgetWithText(FgButton, 'Call'),
    );
    await tester.tap(brunoCall);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Decline Leo’s call?'), findsOneWidget);
    expect(
      find.text(
        'Leo is calling you. You can be in one call at a time: calling Bruno declines it.',
      ),
      findsOneWidget,
    );
    // Going back leaves the ring as it was.
    await tester.tap(find.widgetWithText(FgButton, 'Cancel'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Decline Leo’s call?'), findsNothing);
    expect(state.callState.incomingCall?.groupId, 'c');
    expect(state.callState.activeCall, isNull);
    await tester.pump(const Duration(seconds: 8));
    await tester.pumpWidget(const SizedBox());
  });
}
