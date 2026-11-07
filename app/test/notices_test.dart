import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/format.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/bell.dart';
import 'package:fogata/src/theme.dart';

const marta = UiPerson(
  id: 'marta',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

void main() {
  test('a missed call dismissed from the bell is forgotten everywhere', () {
    final state = AppState('/nonexistent/fogata-notices-test')
      ..contacts.friends = [
        const UiFriend(
          person: marta,
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ]
      ..notices.missedCalls['marta'] = 1000
      ..notices.missedNote = 'marta'
      ..notices.missedUnseen = true;
    final notice = state.notices.list.single;
    expect(notice.kind, NoticeKind.missedCall);
    state.dismissNotice(notice);
    expect(state.notices.missedCalls, isEmpty);
    expect(state.notices.missedNote, isNull);
    expect(state.notices.missedUnseen, isFalse);
    expect(state.notices.list, isEmpty);
  });

  test('the bell chimes for a new notice, not for those it held', () {
    const leo = UiPerson(
      id: 'leo',
      name: 'Leo',
      short: '3H8N·W5PA',
      avatar: UiAvatar(preset: 2),
    );
    final state = AppState('/nonexistent/fogata-notices-test')
      ..contacts.friends = [
        const UiFriend(
          person: marta,
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ]
      ..notices.missedCalls['marta'] = 1000;
    // Before the first refreshes nothing sounds; then what was waiting stays quiet.
    state.notices.soundForNew();
    state.notices.listen();
    state.notices.soundForNew();
    expect(state.played, isEmpty);
    state.contacts.requests = const UiRequests(
      incoming: [
        UiRequest(person: leo, fingerprint: '3H8N W5PA', sinceMs: 2000),
      ],
      outgoing: [],
    );
    state.notices.soundForNew();
    state.notices.soundForNew();
    expect(state.played, [UiSound.notice]);
    // Another one at once is not a second chime.
    state.notices.missedCalls['marta'] = 3000;
    state.notices.soundForNew();
    expect(state.played, [UiSound.notice]);
  });

  test('no chime with the sounds off', () {
    final state = AppState('/nonexistent/fogata-notices-test')
      ..prefs.sounds = false
      ..notices.listen()
      ..contacts.requests = const UiRequests(
        incoming: [
          UiRequest(person: marta, fingerprint: '7Q4M K2XR', sinceMs: 1),
        ],
        outgoing: [],
      );
    state.notices.soundForNew();
    expect(state.played, isEmpty);
  });

  test('a message from a friend whose clock is behind still shows as new', () {
    final state = AppState('/nonexistent/fogata-notices-test');
    UiDirectChat chat(int unread, int lastMs) => UiDirectChat(
      away: false,
      friend: marta,
      online: true,
      last: 'hello',
      lastMine: false,
      lastMs: lastMs,
      unread: unread,
      timerMs: 0,
    );
    state.direct.take([chat(0, 1000)], nowMs: 1000);
    // The bell was opened at 50 000; Marta's clock is a minute behind this one's, so her
    // message written at 61 000 here says 1 000.
    state.prefs.noticesSeenMs = 50000;
    state.direct.take([chat(1, 1000)], nowMs: 61000);
    expect(state.notices.list.single.kind, NoticeKind.directMessage);
    expect(state.notices.unseen, 1);
  });

  testWidgets('the identity open on another computer stays in the bell too', (
    tester,
  ) async {
    const said =
        'This identity is open on another computer too. Close Fogata there: '
        'one identity works on one computer at a time.';
    final state = AppState('/nonexistent/fogata-notices-test')
      ..notices.listen();
    state.onEvent(
      const UiEvent(
        kind: UiEventKind.otherSession,
        text: said,
        received: 0,
        total: 0,
      ),
    );
    // The toast as before, and the bell, with its chime.
    expect(state.toasts.list.single.text, said);
    expect(state.notices.list.single.kind, NoticeKind.otherSession);
    expect(state.notices.unseen, 1);
    expect(state.played, [UiSound.notice]);

    state.notices.bellFrom = const Rect.fromLTWH(700, 10, 32, 32);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Stack(children: [BellLayer()])),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Open on another computer too'), findsOneWidget);
    expect(find.text(said), findsOneWidget);
    // Nothing to do about it here but take it out.
    expect(find.text('Dismiss'), findsOneWidget);
    await tester.tap(find.text('Dismiss'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.notices.list, isEmpty);
    expect(find.text('Nothing new'), findsOneWidget);
    // Not kept for the next runs: only what is said again shows again.
    expect(state.prefs.dismissedNotices, isEmpty);
    // The toast goes by itself.
    await tester.pump(const Duration(seconds: 8));
    expect(state.toasts.list, isEmpty);
  });

  testWidgets('a declined invitation shows in a toast and in the bell', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-notices-test')
      ..contacts.friends = [
        const UiFriend(
          person: marta,
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ]
      ..notices.listen();
    UiEvent declined(String device, String who) => UiEvent(
      kind: UiEventKind.invitationDeclined,
      group: 'g1',
      device: device,
      name: 'Hill crew',
      text: who,
      received: 0,
      total: 0,
    );
    state.onEvent(declined('marta', 'Marta'));
    const said = 'Marta declined the invitation to “Hill crew”';
    expect(state.toasts.list.single.text, '$said.');
    expect(state.toasts.list.single.error, isFalse);
    final notice = state.notices.list.single;
    expect(notice.kind, NoticeKind.invitationDeclined);
    expect(notice.person, marta);
    expect(state.notices.unseen, 1);
    expect(state.played, [UiSound.notice]);
    // Someone not known here by now: the name the core gave.
    state.onEvent(declined('bea', 'Bea'));
    expect(
      state.notices.list.map((n) => n.declined?.said),
      contains('Bea declined the invitation to “Hill crew”'),
    );
    state.dismissNotice(
      state.notices.list.firstWhere((n) => n.declined?.person.id == 'bea'),
    );

    state.notices.bellFrom = const Rect.fromLTWH(700, 10, 32, 32);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Stack(children: [BellLayer()])),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text(said), findsOneWidget);
    expect(find.text('Dismiss'), findsOneWidget);
    await tester.tap(find.text('Dismiss'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.notices.list, isEmpty);
    expect(find.text('Nothing new'), findsOneWidget);
    // Said for this run: nothing kept for the next ones.
    expect(state.prefs.dismissedNotices, isEmpty);
    await tester.pump(const Duration(seconds: 8));
    expect(state.toasts.list, isEmpty);
  });

  test('what the bell showed before a restart is not new again', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-bell-seen');
    File('${dir.path}/app.json')
        .writeAsStringSync(jsonEncode({'noticesSeen': 5000}));
    final state = AppState(dir.path);
    await state.boot();
    state.contacts.requests = const UiRequests(
      incoming: [
        UiRequest(person: marta, fingerprint: '7Q4M K2XR', sinceMs: 4000),
      ],
      outgoing: [],
    );
    expect(state.notices.list, hasLength(1));
    expect(state.notices.unseen, 0);
    await dir.delete(recursive: true);
  });

  test('times on another day say which', () {
    final now = DateTime(2026, 10, 24, 12);
    int at(DateTime t) => t.millisecondsSinceEpoch;
    expect(dayClock(at(DateTime(2026, 10, 24, 9, 5)), now: now), '09:05');
    expect(
      dayClock(at(DateTime(2026, 10, 23, 23, 23)), now: now),
      'yesterday 23:23',
    );
    expect(dayClock(at(DateTime(2026, 10, 2, 8, 0)), now: now), 'Oct 2, 08:00');
    expect(
      dayClock(at(DateTime(2025, 12, 31, 8, 0)), now: now),
      'Dec 31 2025, 08:00',
    );
  });

  test('a call missed during another call or while appearing offline, and one '
      'taken on another computer', () async {
    UiEvent ended(String group, UiCallEnd reason) => UiEvent(
      kind: UiEventKind.callEnded,
      group: group,
      device: 'marta',
      callEnd: reason,
      received: 0,
      total: 0,
    );
    final state = AppState('/nonexistent/fogata-notices-test')
      ..contacts.friends = [
        const UiFriend(
          person: marta,
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
      ];
    state.onEvent(ended('c1', UiCallEnd.missedInCall));
    await pumpEventQueue();
    expect(state.notices.missedCalls.keys, ['marta']);
    expect(
      state.notices.missedCallTitle(marta),
      'Marta called while you were in a call',
    );
    // One declined by itself while this device appeared offline.
    state.onEvent(ended('c0', UiCallEnd.missedOffline));
    await pumpEventQueue();
    expect(state.notices.missedCalls.keys, ['marta']);
    expect(
      state.notices.missedCallTitle(marta),
      'Marta called while you appeared offline',
    );
    // A ring answered on another computer with this identity is no missed call.
    state.notices.missedCalls.clear();
    state.onEvent(ended('c2', UiCallEnd.elsewhere));
    await pumpEventQueue();
    expect(state.notices.missedCalls, isEmpty);
    expect(state.toasts.list, isEmpty);
    // Answered here too late, the call's page says where it went.
    state
      ..roomState.roomGroup = 'c3'
      ..nav.page = Screen.room;
    state.onEvent(ended('c3', UiCallEnd.elsewhere));
    await pumpEventQueue();
    expect(state.toasts.list.single.text, 'Answered on another computer');
    expect(state.roomState.roomGroup, isNull);
    expect(state.notices.missedCalls, isEmpty);
  });

  testWidgets('Clear all leaves the bell empty, older notices too', (
    tester,
  ) async {
    final people = [
      for (var i = 0; i < 5; i++)
        UiPerson(
          id: 'p$i',
          name: 'Friend $i',
          short: 'AB${i}C·D${i}EF',
          avatar: const UiAvatar(preset: 1),
        ),
    ];
    final state = AppState('/nonexistent/fogata-notices-test')
      ..contacts.friends = [
        for (final p in people)
          UiFriend(
            person: p,
            fingerprint: '7Q4M K2XR',
            presence: UiPresence.online,
            delivered: true,
          ),
      ];
    for (final (i, p) in people.indexed) {
      state.notices.missedCalls[p.id] = 1000 + i;
    }
    expect(state.notices.list, hasLength(5));
    state.notices.bellFrom = const Rect.fromLTWH(700, 10, 32, 32);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Stack(children: [BellLayer()])),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(find.text('Clear all'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.notices.list, isEmpty);
    expect(find.text('Nothing new'), findsOneWidget);
  });

  test('the badge counts every notice not seen, not only the three shown', () {
    final state = AppState('/nonexistent/fogata-notices-test')
      ..prefs.noticesSeenMs = 500;
    state.contacts.friends = [
      for (var i = 0; i < 5; i++)
        UiFriend(
          person: UiPerson(
            id: 'p$i',
            name: 'Friend $i',
            short: 'AB${i}C·D${i}EF',
            avatar: const UiAvatar(preset: 1),
          ),
          fingerprint: '7Q4M K2XR',
          presence: UiPresence.online,
          delivered: true,
        ),
    ];
    for (var i = 0; i < 5; i++) {
      state.notices.missedCalls['p$i'] = 1000 + i;
    }
    expect(state.notices.unseen, 5);
    state.prefs.noticesSeenMs = 1002;
    expect(state.notices.unseen, 2);
  });

  test(
    'dismissed notices that are gone are forgotten when settings are saved',
    () {
      const leo = UiPerson(
        id: 'leo',
        name: 'Leo',
        short: '3H8N·W5PA',
        avatar: UiAvatar(preset: 2),
      );
      final state = AppState('/nonexistent/fogata-notices-test')
        ..prefs.dismissedNotices.addAll([
          'friend:marta',
          'friend:gone',
          'group:old',
        ])
        ..contacts.requests = const UiRequests(
          incoming: [
            UiRequest(person: marta, fingerprint: '7Q4M K2XR', sinceMs: 1000),
            UiRequest(person: leo, fingerprint: '3H8N W5PA', sinceMs: 2000),
          ],
          outgoing: [],
        );
      // Before the core said what there is, nothing is forgotten.
      state.dismissNotice(state.notices.list.single);
      expect(state.prefs.dismissedNotices, {
        'friend:marta',
        'friend:gone',
        'group:old',
        'friend:leo',
      });
      expect(state.notices.list, isEmpty);
      state.notices.known = true;
      state.setClock12(false);
      expect(state.prefs.dismissedNotices, {'friend:marta', 'friend:leo'});
      expect(state.notices.list, isEmpty);
    },
  );
}
