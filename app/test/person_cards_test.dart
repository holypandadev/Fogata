// The people in a voice room, listed under it in the side column, open their cards beside
// them: a friend's card, the person's own panel, and for a member who is not a friend a
// card of the same look that offers a friend request, or says where one stands.

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/friend_card.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/user_panel.dart' show UserPanelLayer;
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

UiMember _member(UiPerson p, {bool online = true, bool me = false}) => UiMember(
  person: p,
  rank: 1,
  manager: false,
  me: me,
  path: online ? UiPath.direct : null,
  away: false,
  waitingForKey: false,
  fingerprint: '${p.short} 0000',
);

UiRequest _request(UiPerson p) =>
    UiRequest(person: p, fingerprint: p.short, sinceMs: 1);

/// A room of group `g` with this device, a friend (`p0`) and a member who is not one
/// (`p1`), shown in the shell.
Future<AppState> _show(WidgetTester tester) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 3)
    ..contacts.friends = [
      UiFriend(
        person: other(0),
        fingerprint: other(0).short,
        presence: UiPresence.inRoom,
        delivered: true,
      ),
    ];
  state.contacts.members['g'] = [
    _member(me, me: true),
    _member(other(0)),
    _member(other(1)),
  ];
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 400));
  tester.takeException();
  return state;
}

Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 1));
}

Finder _side(String id) => find.byKey(ValueKey('side-person:$id'));

Future<void> _tap(WidgetTester tester, Finder target) async {
  await tester.tap(target);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
  tester.takeException();
}

/// A click where [target] is, under what floats over it now.
Future<void> _tapOver(WidgetTester tester, Finder target) async {
  await tester.tapAt(tester.getCenter(target));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
  tester.takeException();
}

void main() {
  setUpAll(loadFonts);

  testWidgets("a friend in the voice room opens the friend's card", (
    tester,
  ) async {
    final state = await _show(tester);
    await _tap(tester, _side('p0'));
    expect(state.nav.friendCard, 'p0');
    expect(state.nav.memberCard, isFalse);
    expect(find.byType(FriendCardLayer), findsOneWidget);
    expect(find.byKey(const ValueKey('friend-card-message')), findsOneWidget);
    expect(find.byKey(const ValueKey('member-card-ask')), findsNothing);
    // Beside the row it was opened from.
    final card = tester.getTopLeft(
      find.byKey(const ValueKey('friend-card-message')),
    );
    expect(card.dx, greaterThan(tester.getTopLeft(_side('p0')).dx));
    // A click on the same row, beside the card, closes it.
    await _tapOver(tester, _side('p0'));
    expect(state.nav.friendCard, isNull);
    await _done(tester);
  });

  testWidgets('a member who is not a friend gets a card that offers a friend '
      'request, or says where one stands', (tester) async {
    final state = await _show(tester);
    await _tap(tester, _side('p1'));
    expect(state.nav.friendCard, 'p1');
    expect(state.nav.memberCard, isTrue);
    final card = find.byType(FriendCardLayer);
    expect(
      find.descendant(of: card, matching: find.text(other(1).name)),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: card,
        matching: find.text(l10n.memberCardInGroup('Friday games')),
      ),
      findsOneWidget,
    );
    final ask = find.byKey(const ValueKey('member-card-ask'));
    expect(ask, findsOneWidget);
    expect(tester.widget<FgButton>(ask).onTap, isNotNull);
    expect(find.byKey(const ValueKey('friend-card-call')), findsNothing);

    // A request sent and waiting: said so instead of the button.
    state.contacts.requests = UiRequests(
      incoming: const [],
      outgoing: [_request(other(1))],
    );
    state.contacts.changed();
    await tester.pump();
    expect(ask, findsNothing);
    expect(find.byKey(const ValueKey('member-card-sent')), findsOneWidget);
    expect(find.text(l10n.memberCardRequestSent), findsOneWidget);

    // A request of theirs: accepted from the card.
    state.contacts.requests = UiRequests(
      incoming: [_request(other(1))],
      outgoing: const [],
    );
    state.contacts.changed();
    await tester.pump();
    expect(find.byKey(const ValueKey('member-card-accept')), findsOneWidget);
    expect(find.text(l10n.memberCardAsksYou(other(1).name)), findsOneWidget);

    // Offline: no request can go, and the card says why.
    state.contacts.requests = UiRequests(
      incoming: const [],
      outgoing: const [],
    );
    state.contacts.members['g'] = [
      _member(me, me: true),
      _member(other(0)),
      _member(other(1), online: false),
    ];
    state.contacts.changed();
    await tester.pump();
    expect(tester.widget<FgButton>(ask).onTap, isNull);
    expect(find.text(l10n.memberCardOnlyOnline(other(1).name)), findsOneWidget);

    // Once friends, the card is a friend's.
    state.contacts.friends = [
      ...state.contacts.friends,
      UiFriend(
        person: other(1),
        fingerprint: other(1).short,
        presence: UiPresence.inRoom,
        delivered: true,
      ),
    ];
    state.contacts.changed();
    await tester.pump();
    expect(find.byKey(const ValueKey('friend-card-message')), findsOneWidget);
    expect(ask, findsNothing);
    state.closeFriendCard();
    await _done(tester);
  });

  testWidgets('someone in a direct call who is neither a friend nor a member '
      'gets a card, without a friend request', (tester) async {
    final state = await _show(tester);
    // A direct call: p1 came with the friend, and shares no group with this device.
    state.contacts.members.remove('g');
    state.callState.calls = [
      UiCall(
        groupId: 'g',
        with_: other(0),
        outgoing: true,
        phase: UiCallPhase.connected,
        sinceMs: 1,
        members: [me, other(0), other(1)],
        adding: const [],
        away: const [],
      ),
    ];
    state.contacts.changed();
    state.callState.changed();
    await tester.pump(const Duration(milliseconds: 400));
    tester.takeException();
    await _tap(tester, _side('p1'));
    expect(state.nav.friendCard, 'p1');
    final card = find.byType(FriendCardLayer);
    expect(
      find.descendant(of: card, matching: find.text(other(1).name)),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('member-card-in-call')), findsOneWidget);
    // The other side takes a request only from a member of a group it is in.
    expect(find.byKey(const ValueKey('member-card-ask')), findsNothing);
    await _done(tester);
  });

  testWidgets('this device in the voice room opens its own panel beside it', (
    tester,
  ) async {
    final state = await _show(tester);
    await _tap(tester, _side(me.id));
    expect(state.nav.friendCard, isNull);
    expect(state.nav.userPanelFrom, isNotNull);
    expect(state.nav.userPanelBeside, isTrue);
    expect(find.byType(UserPanelLayer), findsOneWidget);
    final edit = find.text(l10n.userPanelEditProfile);
    expect(edit, findsOneWidget);
    expect(
      tester.getTopLeft(edit).dx,
      greaterThan(tester.getTopLeft(_side(me.id)).dx),
    );
    await _tapOver(tester, _side(me.id));
    expect(state.nav.userPanelFrom, isNull);
    await _done(tester);
  });

  testWidgets('every row of the members page opens a card', (tester) async {
    final state = await _show(tester);
    state.showMembers('g');
    await tester.pump(const Duration(milliseconds: 400));
    tester.takeException();
    Finder row(String id) => find.byKey(ValueKey('member-row:$id'));
    await _tap(tester, row('p1'));
    expect(state.nav.memberCard, isTrue);
    expect(state.nav.memberCardGroup, 'g');
    expect(find.byKey(const ValueKey('member-card-ask')), findsOneWidget);
    state.closeFriendCard();
    await tester.pump(const Duration(milliseconds: 400));
    await _tap(tester, row('p0'));
    expect(find.byKey(const ValueKey('friend-card-message')), findsOneWidget);
    state.closeFriendCard();
    await tester.pump(const Duration(milliseconds: 400));
    await _tap(tester, row(me.id));
    expect(state.nav.userPanelBeside, isTrue);
    expect(find.text(l10n.userPanelEditProfile), findsOneWidget);
    state.closeUserPanel();
    await _done(tester);
  });

  testWidgets("a writer's name in the room's chat opens their card", (
    tester,
  ) async {
    final state = await _show(tester);
    final room = state.roomState.room!;
    state.roomState.room = UiRoom(
      startedMs: room.startedMs,
      joined: true,
      participants: room.participants,
      chat: [
        UiChatLine(
          id: 'c1',
          sender: 'p1',
          nick: other(1).name,
          text: 'Hello',
          timestampMs: DateTime.now().millisecondsSinceEpoch,
          mine: false,
        ),
      ],
      files: const [],
    );
    state.roomState.changed();
    await tester.pump(const Duration(milliseconds: 400));
    tester.takeException();
    final name = find.text(other(1).name);
    // The name over the line, in the nearest column that holds both.
    final line = find
        .ancestor(of: find.text('Hello'), matching: find.byType(Column))
        .evaluate()
        .firstWhere(
          (column) => find
              .descendant(
                of: find.byElementPredicate((e) => e == column),
                matching: name,
              )
              .evaluate()
              .isNotEmpty,
        );
    await _tap(
      tester,
      find.descendant(
        of: find.byElementPredicate((e) => e == line),
        matching: name,
      ),
    );
    expect(state.nav.friendCard, 'p1');
    expect(state.nav.memberCard, isTrue);
    state.closeFriendCard();
    await _done(tester);
  });

  testWidgets(
    "a friend's card says who they are and offers both actions in a row",
    (tester) async {
      final state = await _show(tester);
      state.contacts.friends = [
        UiFriend(
          person: other(0),
          fingerprint: '${other(0).short} 1234',
          presence: UiPresence.online,
          delivered: true,
        ),
        UiFriend(
          person: other(1),
          fingerprint: other(1).short,
          presence: UiPresence.online,
          delivered: true,
        ),
      ];
      state.contacts.mutualFriends['p0'] = ['p1'];
      await _tap(tester, _side('p0'));
      final card = find.byType(FriendCardLayer);
      Finder inCard(Finder f) => find.descendant(of: card, matching: f);
      // Their code under the name, and online in words.
      final code = inCard(find.byKey(const ValueKey('friend-card-code')));
      expect(tester.widget<Text>(code).data, other(0).short);
      final presence = inCard(
        find.byKey(const ValueKey('friend-card-presence')),
      );
      expect(tester.widget<Text>(presence).data, l10n.friendsOnline);
      expect(
        tester.getTopLeft(presence).dy,
        greaterThan(tester.getTopLeft(code).dy),
      );
      // Call and Message side by side, each half of the card's width.
      final call = inCard(find.byKey(const ValueKey('friend-card-call')));
      final message = inCard(find.byKey(const ValueKey('friend-card-message')));
      expect(tester.getTopLeft(call).dy, tester.getTopLeft(message).dy);
      expect(
        tester.getTopLeft(message).dx,
        greaterThan(tester.getTopRight(call).dx),
      );
      expect(
        tester.getSize(call).width,
        closeTo(tester.getSize(message).width, .5),
      );
      // The friends in common under them.
      final mutual = inCard(find.text(l10n.friendCardMutualFriends(1)));
      expect(mutual, findsOneWidget);
      expect(
        tester.getTopLeft(mutual).dy,
        greaterThan(tester.getBottomLeft(call).dy),
      );
      await _done(tester);
    },
  );

  testWidgets(
    "a friend's card puts its actions one under the other when their words "
    'would not fit in a row',
    (tester) async {
      await _show(tester);
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      await tester.pump();
      await _tap(tester, _side('p0'));
      final card = find.byType(FriendCardLayer);
      final call = find.descendant(
        of: card,
        matching: find.byKey(const ValueKey('friend-card-call')),
      );
      final message = find.descendant(
        of: card,
        matching: find.byKey(const ValueKey('friend-card-message')),
      );
      expect(
        tester.getTopLeft(message).dy,
        greaterThanOrEqualTo(tester.getBottomLeft(call).dy),
      );
      expect(
        tester.getSize(call).width,
        closeTo(tester.getSize(message).width, .5),
      );
      // No word of either is cut.
      for (final button in [call, message]) {
        for (final text in tester.widgetList<RichText>(
          find.descendant(of: button, matching: find.byType(RichText)),
        )) {
          final box = tester.renderObject<RenderParagraph>(find.byWidget(text));
          expect(box.didExceedMaxLines, isFalse);
        }
      }
      await _done(tester);
    },
  );
}
