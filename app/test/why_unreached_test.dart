// What is said where a connection failed about why: this side's network, plainly, and
// nothing when nothing is known.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/friend_card.dart';
import 'package:fogata/src/screens/room.dart' show Phase, phaseOf;
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/user_panel.dart' show shellLayerKey;
import 'package:fogata/src/screens/why_unreached.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

const _nothing = UiReach(causes: [], fine: false);
const _fine = UiReach(causes: [], fine: true);
const _symmetric = UiReach(
  causes: [UiCause.symmetricNat, UiCause.noIpv6],
  fine: false,
);

Future<void> _pump(WidgetTester tester, Widget child) => tester.pumpWidget(
  MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Material(child: child),
  ),
);

Future<UiReach> Function() _gives(UiReach reach) =>
    () async => reach;

void main() {
  setUpAll(loadFonts);

  test('each cause has its words, the firewall first', () {
    expect(
      reachTexts(
        const UiReach(
          causes: [
            UiCause.offline,
            UiCause.udpBlocked,
            UiCause.symmetricNat,
            UiCause.noIpv6,
          ],
          fine: false,
        ),
        firewall: true,
      ),
      [
        l10n.reachFirewall,
        l10n.reachOffline,
        l10n.reachUdpBlocked,
        l10n.reachSymmetricNat,
        l10n.reachNoIpv6,
      ],
    );
  });

  test('nothing known says nothing', () {
    expect(reachTexts(null, firewall: false, friend: 'Sofía'), isEmpty);
    expect(reachTexts(_nothing, firewall: false, friend: 'Sofía'), isEmpty);
  });

  test('a network that looks fine points at the other side', () {
    expect(reachTexts(_fine, firewall: false, friend: 'Sofía'), [
      'Your network looks fine for Fogata, so what keeps you apart is probably on Sofía’s side (offline, or their network).',
    ]);
    expect(reachTexts(_fine, firewall: false), [l10n.reachFineOthers]);
    // Left out where the place already says the other one is offline; what is in the
    // way here is still said there.
    expect(
      reachTexts(_fine, firewall: false, friend: 'Sofía', fine: false),
      isEmpty,
    );
    expect(
      reachTexts(_symmetric, firewall: true, friend: 'Sofía', fine: false),
      [l10n.reachFirewall, l10n.reachSymmetricNat, l10n.reachNoIpv6],
    );
    // A firewall in the way is not "fine".
    expect(reachTexts(_fine, firewall: true, friend: 'Sofía'), [
      l10n.reachFirewall,
    ]);
  });

  test('every language says it in its own words', () {
    addTearDown(() => useLanguage('en'));
    final english = reachTexts(
      _symmetric,
      firewall: false,
      friend: 'Sofía',
    ).join();
    for (final code in languages.keys.where((c) => c != 'en')) {
      useLanguage(code);
      final said = reachTexts(_symmetric, firewall: false);
      expect(said, hasLength(2), reason: code);
      expect(said.join(), isNot(english), reason: code);
      expect(
        reachTexts(_fine, firewall: false, friend: 'Sofía').single,
        contains('Sofía'),
        reason: code,
      );
    }
  });

  testWidgets('shows what the node saw once it answers', (tester) async {
    await _pump(
      tester,
      WhyUnreached(firewall: false, friend: 'Sofía', load: _gives(_symmetric)),
    );
    expect(find.byKey(const ValueKey('why-unreached')), findsNothing);
    await tester.pump();
    expect(find.text(l10n.reachSymmetricNat), findsOneWidget);
    expect(find.text(l10n.reachNoIpv6), findsOneWidget);
    expect(find.textContaining('looks fine'), findsNothing);
  });

  testWidgets('a core that cannot answer leaves the firewall alone', (
    tester,
  ) async {
    await _pump(
      tester,
      WhyUnreached(
        firewall: true,
        load: () async => throw StateError('no core'),
      ),
    );
    await tester.pump();
    expect(find.text(l10n.reachFirewall), findsOneWidget);
    await _pump(
      tester,
      WhyUnreached(
        key: const ValueKey('again'),
        firewall: false,
        load: () async => throw StateError('no core'),
      ),
    );
    await tester.pump();
    expect(find.byKey(const ValueKey('why-unreached')), findsNothing);
  });

  testWidgets("an offline friend's card says why, an online one's does not", (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    const sofia = UiPerson(
      id: 'sofia',
      name: 'Sofía',
      short: 'C07N·50S4',
      avatar: UiAvatar(preset: 2),
    );
    UiFriend friend(UiPresence presence) => UiFriend(
      person: sofia,
      fingerprint: 'C07N 50S4',
      presence: presence,
      delivered: true,
    );
    final state = AppState('/nonexistent/fogata-why-unreached')
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..contacts.friends = [friend(UiPresence.offline)]
      ..reachability = _gives(_fine);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Material(
            child: Builder(
              builder: (context) => Stack(
                key: shellLayerKey,
                children: [
                  const Positioned.fill(child: SizedBox()),
                  if (AppScope.of(context).nav.friendCard != null)
                    const FriendCardLayer(),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    state.toggleFriendCard(sofia.id, const Rect.fromLTWH(100, 100, 40, 40));
    await tester.pumpAndSettle();
    tester.takeException();
    // Nothing in the way here: the card, which already says the friend is offline and
    // when they were last seen, adds no guess about their side.
    expect(find.byKey(const ValueKey('why-unreached')), findsNothing);
    expect(find.textContaining('looks fine'), findsNothing);
    expect(find.text(l10n.friendCardCallsOnlyOnline), findsOneWidget);
    state.closeFriendCard();
    await tester.pumpAndSettle();

    // Behind a symmetric NAT, with the firewall in the way too: both, firewall first.
    state
      ..reachability = _gives(_symmetric)
      ..notices.firewallCommand = 'sudo ufw allow 24567';
    state.toggleFriendCard(sofia.id, const Rect.fromLTWH(100, 100, 40, 40));
    await tester.pumpAndSettle();
    tester.takeException();
    final said = tester
        .widgetList<Text>(
          find.descendant(
            of: find.byKey(const ValueKey('why-unreached')),
            matching: find.byType(Text),
          ),
        )
        .map((t) => t.data)
        .toList();
    expect(said, [
      l10n.reachFirewall,
      l10n.reachSymmetricNat,
      l10n.reachNoIpv6,
    ]);
    state.closeFriendCard();
    await tester.pumpAndSettle();

    // Online: nothing to explain.
    state.contacts.friends = [friend(UiPresence.online)];
    state.toggleFriendCard(sofia.id, const Rect.fromLTWH(100, 100, 40, 40));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(find.byKey(const ValueKey('why-unreached')), findsNothing);
    state.closeFriendCard();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('asks again while shown: what it said may change', (
    tester,
  ) async {
    var reach = _fine;
    await _pump(
      tester,
      WhyUnreached(
        firewall: false,
        friend: 'Sofía',
        load: () async => reach,
        every: const Duration(seconds: 1),
      ),
    );
    await tester.pump();
    expect(find.textContaining('looks fine'), findsOneWidget);
    // The line went down: "fine" goes, and only what is seen now is said.
    reach = const UiReach(causes: [UiCause.offline], fine: false);
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(find.textContaining('looks fine'), findsNothing);
    expect(find.text(l10n.reachOffline), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  group('the room and the call', () {
    setUp(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('window_manager'),
            (call) async => false,
          );
    });

    Future<void> show(WidgetTester tester, AppState state) async {
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
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      tester.takeException();
    }

    Future<void> done(WidgetTester tester, AppState state) async {
      state.callState.call = null;
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    }

    testWidgets('a room that could not connect says why', (tester) async {
      final state = fakeRoom(people: 1)
        ..reachability = _gives(
          const UiReach(causes: [UiCause.udpBlocked], fine: false),
        );
      final memory = RoomMemory(0)
        ..othersExpected = true
        ..joinedMs = DateTime.now().millisecondsSinceEpoch - 60 * 1000;
      state.roomState.memories[state.roomState.roomGroup!] = memory;
      // Nobody else in it: the people who were are not reached.
      await show(tester, state);
      expect(phaseOf(state), Phase.failed);
      expect(find.text(l10n.roomCouldNotConnect), findsOneWidget);
      expect(find.text(l10n.reachUdpBlocked), findsOneWidget);
      await done(tester, state);
    });

    testWidgets('a call that could not reach the friend says why, only then', (
      tester,
    ) async {
      final state = fakeRoom(people: 2)..reachability = _gives(_fine);
      final group = state.roomState.roomGroup!;
      state.callState.callEnding = CallEnding(
        group,
        other(0),
        UiCallEnd.unreachable,
      );
      await show(tester, state);
      expect(find.text(l10n.callStateUnreached(other(0).name)), findsOneWidget);
      expect(find.text(l10n.reachFineOthers), findsOneWidget);
      // Not answered: the network has nothing to do with it.
      state.callState.callEnding = CallEnding(
        group,
        other(0),
        UiCallEnd.noAnswer,
      );
      state.callState.changed();
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      expect(find.text(l10n.callStateNoAnswer(other(0).name)), findsOneWidget);
      expect(find.byKey(const ValueKey('why-unreached')), findsNothing);
      await done(tester, state);
    });
  });
}
