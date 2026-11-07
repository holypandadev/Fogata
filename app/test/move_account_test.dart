import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/account_move.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/move_account.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/state/account_move.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

const _summary = UiMoveSummary(
  profile: UiProfile(person: me, fingerprint: '7Q4M K2XR 9PLD 3VHC'),
  friends: 12,
  groups: 3,
  chats: 8,
  files: 40,
  bytes: 1400000000,
);

/// A move as the screens see it, at [step].
AccountMove _move(MoveStep step, {bool sending = false, String? failure}) =>
    AccountMove(sending: sending)
      ..step = step
      ..code = 'K7QM2-XR9PD'
      ..expiresMs = DateTime.now().millisecondsSinceEpoch + 9 * 60 * 1000
      ..computer = 'Leo’s desktop'
      ..lan = true
      ..summary = step == MoveStep.looking ? null : _summary
      ..done = 880000000
      ..total = 1400000000
      ..failure = failure;

Future<AppState> _show(
  WidgetTester tester,
  Widget child,
  AppState state,
) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(child: Center(child: child)),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
  return state;
}

/// Lets the card's countdown timer go.
Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 2));
}

void main() {
  testWidgets(
    'the new computer shows the code, how to use it and its countdown',
    (tester) async {
      final state = fakeRoom(people: 1)..accountMove = _move(MoveStep.looking);
      await _show(tester, const BringCard(), state);
      expect(find.text('Bring your account here'), findsOneWidget);
      expect(find.text('K7QM2-XR9PD'), findsOneWidget);
      expect(find.textContaining('expires in'), findsOneWidget);
      expect(find.textContaining('Settings → Move account'), findsOneWidget);
      expect(find.text('Waiting for your other computer…'), findsOneWidget);
      // A wrong code tried elsewhere is said.
      state.accountMove!
        ..wrongTries = 1
        ..changed();
      await tester.pump(const Duration(milliseconds: 500));
      expect(
        find.text('A wrong code was typed on another computer.'),
        findsOneWidget,
      );
      await _done(tester);
    },
  );

  testWidgets('the offer of the account this computer already runs says so', (
    tester,
  ) async {
    const note = 'This is the account already on this computer.';
    final state = fakeRoom(people: 1)..accountMove = _move(MoveStep.matched);
    await _show(tester, const BringCard(), state);
    expect(find.textContaining(note), findsNothing);

    state.accountMove!
      ..ownFingerprint = '7q4m k2xr 9pld 3vhc'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.textContaining(note), findsOneWidget);

    state.accountMove!
      ..ownFingerprint = 'AAAA BBBB CCCC DDDD'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.textContaining(note), findsNothing);
  });

  testWidgets('then what comes, how far it got and the welcome', (
    tester,
  ) async {
    final state = fakeRoom(people: 1)..accountMove = _move(MoveStep.matched);
    await _show(tester, const BringCard(), state);
    expect(find.text('Found your other computer'), findsOneWidget);
    expect(
      find.text('Same network · encrypted between the two'),
      findsOneWidget,
    );
    expect(find.text('12 friends'), findsOneWidget);
    expect(find.text('3 groups'), findsOneWidget);
    // This computer's own yes first, then the other's.
    expect(find.byKey(const ValueKey('move-take')), findsOneWidget);
    expect(find.text('Bring it here'), findsOneWidget);
    expect(find.text('Confirm on the other computer to start'), findsNothing);
    state.accountMove!
      ..answered = true
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('move-take')), findsNothing);
    expect(find.text('Confirm on the other computer to start'), findsOneWidget);

    state.accountMove!
      ..step = MoveStep.moving
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Bringing Marta’s account'), findsOneWidget);
    expect(find.textContaining('62%'), findsOneWidget);
    expect(find.text('880 MB of 1.4 GB'), findsOneWidget);
    // No way back while files go.
    expect(find.text('Back'), findsNothing);

    state.accountMove!
      ..step = MoveStep.done
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Welcome back, Marta'), findsOneWidget);
    expect(find.byKey(const ValueKey('move-open')), findsOneWidget);
    await _done(tester);
  });

  testWidgets('an expired code, too many wrong codes and a move cut short', (
    tester,
  ) async {
    final state = fakeRoom(people: 1)
      ..accountMove = _move(MoveStep.failed, failure: 'expired');
    await _show(tester, const BringCard(), state);
    expect(find.text('expired'), findsOneWidget);
    expect(find.textContaining('Codes last 10 minutes'), findsOneWidget);
    expect(find.byKey(const ValueKey('move-new-code')), findsOneWidget);

    state.accountMove!
      ..failure = 'too_many_tries'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('stopped working'), findsOneWidget);

    state.accountMove!
      ..failure = 'later'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.textContaining('next time Fogata starts here'), findsOneWidget);
    expect(find.textContaining('Nothing changed'), findsNothing);
    expect(find.byKey(const ValueKey('move-start-again')), findsOneWidget);

    state.accountMove!
      ..failure = 'interrupted'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('The move stopped halfway'), findsOneWidget);
    expect(find.textContaining('Nothing changed'), findsOneWidget);
    expect(find.text('Make a new profile instead'), findsOneWidget);
    await _done(tester);
  });

  testWidgets('settings: moving away, and bringing one here warns what goes', (
    tester,
  ) async {
    final state = fakeRoom(people: 2);
    await _show(
      tester,
      const SizedBox(width: 700, child: MoveAccountPage()),
      state,
    );
    expect(find.text('Move to another computer'), findsOneWidget);
    expect(
      find.text('Fogata goes offline on this computer while it moves.'),
      findsOneWidget,
    );
    expect(find.text('Replaces Marta on this computer.'), findsOneWidget);
    // Continue waits for a code, and takes one as soon as it is typed.
    FgButton button() =>
        tester.widget<FgButton>(find.byKey(const ValueKey('move-continue')));
    expect(button().onTap, isNull);
    await tester.enterText(find.byType(TextField), 'K7QM2-XR9PD');
    await tester.pump();
    expect(button().onTap, isNotNull);
    await tester.tap(find.byKey(const ValueKey('move-bring')));
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      find.textContaining('the profile on this computer now, will be replaced'),
      findsOneWidget,
    );
    expect(find.text('Replace Marta and show a code'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Replace Marta and show a code'), findsNothing);
    await _done(tester);
  });

  testWidgets('the old computer asks once, naming the new one and what goes', (
    tester,
  ) async {
    final answers = <bool>[];
    final state = fakeRoom(people: 1);
    await _show(
      tester,
      SizedBox(
        width: 460,
        child: ConfirmMove(
          move: _move(MoveStep.matched, sending: true),
          onAnswer: (yes) => answers.add(yes),
        ),
      ),
      state,
    );
    expect(find.text('Move your account to “Leo’s desktop”?'), findsOneWidget);
    expect(find.text('3 groups, with their keys'), findsOneWidget);
    expect(find.text('Your settings'), findsOneWidget);
    expect(find.textContaining('stops using your account'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('move-yes')));
    expect(answers, [true]);
    await _done(tester);
  });

  testWidgets('the old computer afterwards: moved, or waiting to confirm', (
    tester,
  ) async {
    final state = fakeRoom(people: 1)
      ..moveState = const UiMoveState(
        kind: 'moved',
        name: 'Marta',
        computer: 'Leo’s desktop',
        atMs: 0,
      );
    await _show(
      tester,
      const SizedBox(width: 1000, height: 800, child: MovedScreen()),
      state,
    );
    expect(
      find.text('Marta’s account moved to “Leo’s desktop”'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('move-forget')), findsOneWidget);
    expect(find.text('Not now'), findsOneWidget);

    state.moveState = const UiMoveState(
      kind: 'moving',
      name: 'Marta',
      computer: 'Leo’s desktop',
      atMs: 0,
    );
    await tester.pumpWidget(const SizedBox());
    await _show(
      tester,
      const SizedBox(width: 1000, height: 800, child: MovedScreen()),
      state,
    );
    expect(find.text('Waiting for “Leo’s desktop” to confirm'), findsOneWidget);
    expect(find.text('Finish the move'), findsOneWidget);
    expect(find.textContaining('keep using this one'), findsOneWidget);
    await _done(tester);
  });

  testWidgets('finishing a move from the waiting page ends on "moved"', (
    tester,
  ) async {
    final state = fakeRoom(people: 1)
      ..moveState = const UiMoveState(
        kind: 'moving',
        name: 'Marta',
        computer: 'Leo’s desktop',
        atMs: 0,
      );
    await _show(
      tester,
      const SizedBox(
        width: 1000,
        height: 800,
        child: WithMoveQuestion(child: MovedScreen()),
      ),
      state,
    );
    final field = find.descendant(
      of: find.byKey(const ValueKey('move-finish-field')),
      matching: find.byType(TextField),
    );
    await tester.enterText(field, 'K7QM2-XR9PD');
    // The code matched: the question shows over the page, and goes once answered.
    final move = _move(MoveStep.matched, sending: true);
    state
      ..accountMove = move
      ..notifyListeners();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byType(MoveQuestion), findsOneWidget);
    move.answered = true;
    state.notifyListeners();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byType(MoveQuestion), findsNothing);
    // The page under it went on as it was, with the code typed.
    expect(tester.widget<TextField>(field).controller!.text, 'K7QM2-XR9PD');
    // The move ended: the folder says "moved", and the page shows it.
    move.step = MoveStep.done;
    state
      ..moveState = const UiMoveState(
        kind: 'moved',
        name: 'Marta',
        computer: 'Leo’s desktop',
        atMs: 0,
      )
      ..notifyListeners();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('move-forget')), findsOneWidget);
    await _done(tester);
  });

  testWidgets('a code that did not match shows as the mockup says', (
    tester,
  ) async {
    final move = _move(MoveStep.looking, sending: true);
    final state = fakeRoom(people: 1)..accountMove = move;
    await _show(
      tester,
      const SizedBox(width: 700, child: MoveAccountPage()),
      state,
    );
    expect(
      find.text('Looking for the computer showing this code…'),
      findsOneWidget,
    );
    move
      ..step = MoveStep.failed
      ..failure = 'wrong_code'
      ..changed();
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('move-code-wrong')), findsOneWidget);
    expect(find.textContaining('didn’t accept that code'), findsOneWidget);
    expect(find.text('Try again'), findsOneWidget);
    // The same code again gets no second try.
    await tester.enterText(find.byType(TextField), 'k7qm2 xr9pd');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('move-continue')));
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.textContaining('already failed to match once'), findsOneWidget);
    await _done(tester);
  });

  testWidgets(
    'the person’s pill says offline while the core is stopped for a move',
    (tester) async {
      final state = fakeRoom(people: 1)
        ..callState.call = null
        ..roomState.room = null
        ..roomState.roomGroup = null
        ..nav.selectedGroup = null
        ..nav.page = Screen.friends
        ..contacts.groups = [];
      await _show(tester, const Shell(), state);
      expect(find.text('Offline'), findsNothing);
      final online = find.text('Online').evaluate().length;
      state
        ..offlineForMove = true
        ..notifyListeners();
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Offline'), findsOneWidget);
      expect(find.text('Online'), findsNWidgets(online - 1));
      await tester.pumpWidget(const SizedBox());
      // The shell's own timers (toasts, presence) run out.
      await tester.pump(const Duration(minutes: 2));
    },
  );

  test('stopping for a move never waits on the events’ stream', () async {
    // A stream that waits for its next event cannot be let go of before it comes: once
    // the core stops, none comes.
    final never = StreamController<UiEvent>(
      onCancel: () => Completer<void>().future,
    );
    final state = fakeRoom(people: 1)
      ..started = true
      ..eventsSubscription = never.stream.listen((_) {});
    await state.stopForMove().timeout(const Duration(seconds: 5));
    expect(state.offlineForMove, isTrue);
  });

  test(
    'a move whose events end before it did is over, not waited for',
    () async {
      final events = StreamController<UiMoveEvent>();
      final move = AccountMove(sending: false)..follow(events.stream);
      await events.close();
      await Future<void>.delayed(Duration.zero);
      expect(move.step, MoveStep.failed);
      expect(move.failure, 'other');
      // One that ended first stays as it ended.
      final finished = StreamController<UiMoveEvent>();
      final done = AccountMove(sending: true)..follow(finished.stream);
      done.step = MoveStep.done;
      await finished.close();
      await Future<void>.delayed(Duration.zero);
      expect(done.step, MoveStep.done);
      expect(done.failure, isNull);
      move.dispose();
      done.dispose();
    },
  );

  test('the time left reads in plain words', () {
    final move = _move(MoveStep.moving);
    expect(move.share, closeTo(.628, .001));
    expect(move.timeLeftWords, isNull, reason: 'nothing measured yet');
    expect(computerName(), isNotEmpty);
  });
}
