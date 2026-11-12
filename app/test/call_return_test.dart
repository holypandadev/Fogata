import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

final now = DateTime.now().millisecondsSinceEpoch;

UiDirectItem item(
  String id, {
  bool mine = false,
  String text = '',
  UiDirectCall? call,
}) => UiDirectItem(
  id: id,
  seq: 0,
  mine: mine,
  atMs: now,
  text: text,
  files: const [],
  delivered: true,
  expiresMs: 0,
  pinned: false,
  call: call,
);

UiDirectPage page(List<UiDirectItem> items) => UiDirectPage(
  away: false,
  friend: sofia,
  version: 0,
  online: true,
  connected: true,
  items: items,
  more: false,
  pinned: const [],
  timerMs: 0,
  usage: const UiDirectUsage(messages: 0, pictures: 0, videos: 0, files: 0),
  altered: false,
);

UiFriend friend(UiPerson person) => UiFriend(
  person: person,
  fingerprint: '${person.short} 9PLD 3VHC',
  presence: UiPresence.inRoom,
  delivered: true,
);

String buttonText(WidgetTester tester, Finder finder) =>
    tester.widget<FgButton>(finder).text!;

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

  test('how long a call lasted, in words', () {
    const minute = 60000;
    const hour = 60 * minute;
    expect(callLength(0), 'a few seconds');
    expect(callLength(59999), 'a few seconds');
    expect(callLength(minute), '1 minute');
    expect(callLength(2 * minute - 1), '1 minute');
    expect(callLength(30 * minute + 20000), '30 minutes');
    expect(callLength(hour), '1 hour');
    expect(callLength(hour + minute), '1 hour 1 minute');
    expect(callLength(hour + 20 * minute), '1 hour 20 minutes');
    expect(callLength(3 * hour + 30000), '3 hours');
  });

  test('a call line says who started it and how long it lasted', () {
    const short = UiDirectCall(lastedMs: 4000);
    expect(
      callLine(item('1', mine: true, call: short), 'Sofía'),
      'You started a call that lasted a few seconds',
    );
    expect(
      callLine(item('2', call: const UiDirectCall(lastedMs: 4800000)), 'Sofía'),
      'Sofía started a call that lasted 1 hour 20 minutes',
    );
    // Someone else started it: a friend of the friend's who rang them both in.
    expect(
      callLine(
        item('3', call: const UiDirectCall(lastedMs: 1800000, starter: 'Ana')),
        'Sofía',
      ),
      'Ana started a call that lasted 30 minutes',
    );
  });

  testWidgets('a call that ended shows as a line between the messages', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-call-return-test')
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..nav.page = Screen.direct;
    state.direct
      ..open = sofia.id
      ..page = page([
        item('1', text: 'Shall we talk?'),
        item('2', call: const UiDirectCall(lastedMs: 1500000)),
        item('3', mine: true, text: 'Thanks for the call'),
      ]);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: DirectPage()),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 400));
    tester.takeException();
    expect(
      find.text('Sofía started a call that lasted 25 minutes'),
      findsOneWidget,
    );
    expect(
      find.byWidgetPredicate(
        (w) =>
            w.key is ValueKey<String> &&
            (w.key! as ValueKey<String>).value.startsWith('direct-call-line:'),
      ),
      findsOneWidget,
    );
    // The line is not a message of Marta's: her own message still says it arrived.
    expect(find.text('Delivered'), findsOneWidget);

    // While Sofía is still in a call Marta left, the chat's button takes her back.
    final call = find.byKey(const ValueKey('direct-call'));
    expect(buttonText(tester, call), 'Call');
    state.callState.callReturns = {sofia.id};
    state.notifyListeners();
    await tester.pump();
    expect(buttonText(tester, call), 'Return to call');
    expect(tester.widget<FgButton>(call).onTap, isNotNull);
    // Once the call is over for her, it calls again.
    state.callState.callReturns = {};
    state.notifyListeners();
    await tester.pump();
    expect(buttonText(tester, call), 'Call');
  });

  testWidgets('Friends and the card say Return to call for those still in it', (
    tester,
  ) async {
    final leo = other(0);
    final bruno = other(3);
    final state = fakeRoom(people: 1)
      ..contacts.friends = [friend(leo), friend(bruno)]
      ..nav.page = Screen.friends
      ..callState.callReturns = {bruno.id};
    expect(state.callState.callText(bruno.id), 'Return to call');
    expect(state.callState.callText(bruno.id, missed: true), 'Return to call');
    expect(state.callState.callText(leo.id, missed: true), 'Call back');
    expect(state.callState.callText(leo.id), 'Call');
    tester.view.physicalSize = const Size(1440, 900);
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
    await tester.pump(const Duration(milliseconds: 500));
    Finder rowButton(UiPerson p, String text) => find.descendant(
      of: find.byKey(ValueKey('friend-row:${p.id}')),
      matching: find.widgetWithText(FgButton, text),
    );
    expect(rowButton(bruno, 'Return to call'), findsOneWidget);
    expect(rowButton(leo, 'Call'), findsOneWidget);
    expect(rowButton(leo, 'Return to call'), findsNothing);

    state
      ..nav.friendCard = bruno.id
      ..nav.friendCardFrom = const Rect.fromLTWH(600, 200, 400, 60);
    state.notifyListeners();
    await tester.pump(const Duration(milliseconds: 500));
    final cardCall = find.byKey(const ValueKey('friend-card-call'));
    expect(buttonText(tester, cardCall), 'Return to call');
    expect(tester.widget<FgButton>(cardCall).onTap, isNotNull);
    await tester.pump(const Duration(seconds: 8));
    await tester.pumpWidget(const SizedBox());
  });
}
