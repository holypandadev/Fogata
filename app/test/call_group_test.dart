import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

UiFriend friend(UiPerson person, {bool online = true}) => UiFriend(
  person: person,
  fingerprint: '${person.short} 9PLD 3VHC',
  presence: online ? UiPresence.online : UiPresence.offline,
  delivered: true,
);

UiCall call({
  required UiPerson with_,
  List<UiPerson> members = const [],
  List<UiPerson> adding = const [],
  List<UiPerson> away = const [],
  int? aloneSinceMs,
  String? addBlocked,
}) => UiCall(
  groupId: 'g',
  with_: with_,
  outgoing: true,
  phase: UiCallPhase.connected,
  sinceMs: DateTime.now().millisecondsSinceEpoch - 60000,
  members: members,
  adding: adding,
  away: away,
  aloneSinceMs: aloneSinceMs,
  addBlocked: addBlocked,
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

Finder addButton() => find.byWidgetPredicate(
  (w) =>
      w is Springy &&
      (w.tooltip == 'Add a friend to the call' ||
          (w.tooltip?.contains('too old') ?? false)),
);

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
    'a call of more than two: who left, who is rung, who can be added',
    (tester) async {
      final leo = other(0);
      final sofia = other(1);
      final ana = other(2);
      final bruno = other(3);
      final state = fakeRoom(people: 2)
        ..contacts.friends = [
          friend(leo),
          friend(ana),
          friend(bruno, online: false),
        ]
        ..callState.calls = [
          call(
            with_: leo,
            members: [leo, sofia, ana],
            adding: [ana],
            away: [sofia],
          ),
        ];
      await show(tester, state);
      expect(find.text('Call with Leo'), findsWidgets);
      expect(find.text('Sofía left · may come back'), findsOneWidget);
      expect(find.text('Calling Ana…'), findsOneWidget);
      expect(find.byKey(const ValueKey('call-wait')), findsOneWidget);
      expect(
        find.textContaining('When only one is left, the call waits'),
        findsOneWidget,
      );

      await tester.tap(addButton());
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Add to the call'), findsOneWidget);
      final card = find.byKey(const ValueKey('add-to-call'));
      Finder inCard(String text) =>
          find.descendant(of: card, matching: find.text(text));
      expect(inCard('In the call'), findsOneWidget);
      expect(inCard('Calling…'), findsOneWidget);
      expect(inCard('Offline'), findsOneWidget);
      // Typing narrows the friends down.
      await tester.enterText(
        find.descendant(of: card, matching: find.byType(TextField)),
        'bru',
      );
      await tester.pump();
      expect(inCard('Leo'), findsNothing);
      expect(inCard('Bruno'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('the last one left sees how long the call waits', (tester) async {
    final leo = other(0);
    final state = fakeRoom(people: 1)
      ..contacts.friends = [friend(leo)]
      ..callState.calls = [
        call(
          with_: leo,
          members: [leo],
          away: [leo],
          aloneSinceMs: DateTime.now().millisecondsSinceEpoch - 10500,
          addBlocked: 'Leo’s Fogata is too old for calls of more than two',
        ),
      ];
    await show(tester, state);
    expect(find.text('Leo left · may come back'), findsOneWidget);
    expect(
      find.textContaining('You’re the only one left. The call waits'),
      findsOneWidget,
    );
    expect(find.textContaining('0:50'), findsOneWidget);
    // Adding is off, and says why.
    final add = tester.widget<Springy>(addButton());
    expect(add.onTap, isNull);
    expect(add.tooltip, 'Leo’s Fogata is too old for calls of more than two');
    await tester.pumpWidget(const SizedBox());
  });
}
