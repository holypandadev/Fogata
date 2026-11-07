import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/quality.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);
const leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'K2XR·7Q4M',
  avatar: UiAvatar(preset: 2),
);
const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'AB12·CD34',
  avatar: UiAvatar(preset: 3),
);

final start = DateTime(2026, 10, 25, 12);
var _heard = 0;

/// A reading of [id]'s connection at second [s]: something arrived, with [ms] of round
/// trip.
void read(AppState state, String id, int s, {int ms = 32}) =>
    state.callState.call!.quality.take(
      id,
      LinkSample(delayMs: ms, heard: ++_heard),
      start.add(Duration(seconds: s)),
    );

/// Two readings, so the level shows past the gate.
void settle(AppState state, String id, int s, {int ms = 32}) {
  read(state, id, s, ms: ms);
  read(state, id, s + 2, ms: ms);
}

AppState callState() => AppState('/nonexistent/fogata-quality-test')
  ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
  ..roomState.roomGroup = 'g'
  ..roomState.room = const UiRoom(
    startedMs: 0,
    joined: true,
    participants: [
      UiParticipant(person: me, away: false, me: true),
      UiParticipant(person: leo, away: false, me: false),
      UiParticipant(person: sofia, away: false, me: false),
    ],
    chat: [],
    files: [],
  )
  ..contacts.paths['g'] = {'leo': UiPath.throughMember, 'sofia': UiPath.direct}
  ..callState.call = Call(group: 'g', me: 'me');

/// What a click on the bars lands on.
Finder bars() => find.ancestor(
  of: find.byType(SignalBars),
  matching: find.byType(Pressable),
);

Future<void> pump(WidgetTester tester, AppState state, Widget child) async {
  tester.view.physicalSize = const Size(1200, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(child: child),
      ),
    ),
  );
}

void main() {
  testWidgets('a tile shows bars, the word when not good, and Reconnecting', (
    tester,
  ) async {
    final state = callState();
    await pump(
      tester,
      state,
      const Align(
        alignment: Alignment.topRight,
        child: TileQuality(id: 'leo'),
      ),
    );
    expect(find.byType(SignalBars), findsNothing);
    // Good from the start: nothing shows.
    read(state, 'leo', 0);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(SignalBars), findsNothing);
    expect(find.text('Good'), findsNothing);
    settle(state, 'leo', 2, ms: 320);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('Poor'), findsOneWidget);
    // Nothing for 3 s.
    state.callState.call!.quality.take(
      'leo',
      LinkSample(delayMs: 320, heard: _heard),
      start.add(const Duration(seconds: 7)),
    );
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('Reconnecting…'), findsOneWidget);
    expect(find.byType(Spinner), findsOneWidget);
    expect(find.byType(SignalBars), findsNothing);
    // Good again: the bars stay a few seconds, then go.
    settle(state, 'leo', 8);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(SignalBars), findsOneWidget);
    await tester.pump(goodFor - const Duration(seconds: 1));
    expect(find.byType(SignalBars), findsOneWidget);
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(SignalBars), findsNothing);
    // Worse again: back at once.
    settle(state, 'leo', 12, ms: 320);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(SignalBars), findsOneWidget);
  });

  testWidgets('a picture dims while nothing comes', (tester) async {
    final state = callState();
    await pump(
      tester,
      state,
      const QualityDim(id: 'leo', child: SizedBox(width: 20, height: 20)),
    );
    double opacity() =>
        tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity;
    read(state, 'leo', 0);
    await tester.pump();
    expect(opacity(), 1);
    state.callState.call!.quality.cut('leo');
    await tester.pump();
    expect(opacity(), .35);
  });

  testWidgets(
    'a click on the bars opens the details; outside or Escape closes',
    (tester) async {
      final state = callState();
      await pump(
        tester,
        state,
        const Align(
          alignment: Alignment.topRight,
          child: TileQuality(id: 'leo'),
        ),
      );
      settle(state, 'leo', 0, ms: 160);
      settle(state, 'sofia', 0);
      state.callState.call!.quality.theirs('leo', NetLevel.good);
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.text('Okay'), findsOneWidget);
      await tester.tap(bars());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text("Leo’s connection"), findsOneWidget);
      expect(find.text('Between you and Leo'), findsOneWidget);
      expect(find.text("Leo’s own network"), findsOneWidget);
      expect(find.text('Delay'), findsOneWidget);
      expect(find.text('160 ms'), findsOneWidget);
      expect(find.text('Lost packets'), findsOneWidget);
      expect(find.text('Through a member, encrypted'), findsOneWidget);
      expect(
        find.textContaining('goes through another member'),
        findsOneWidget,
      );
      // The details sit under the pill, inside the window.
      final details = tester.getRect(find.byType(ConnectionDetails));
      expect(details.width, 320);
      expect(details.right, lessThanOrEqualTo(1200 - 8));
      expect(
        details.top,
        greaterThan(tester.getRect(find.byType(TileQuality)).bottom),
      );
      // A new reading updates the figures in place.
      read(state, 'leo', 4, ms: 170);
      await tester.pump();
      expect(find.text('170 ms'), findsOneWidget);
      await tester.tapAt(const Offset(20, 700));
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.text("Leo’s connection"), findsNothing);

      await tester.tap(bars());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(ConnectionDetails), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byType(ConnectionDetails), findsNothing);
    },
  );

  testWidgets(
    'the member who carries someone is named, the old words if unknown',
    (tester) async {
      const ana = UiPerson(
        id: 'ana',
        name: 'Ana',
        short: 'EF56·GH78',
        avatar: UiAvatar(preset: 4),
      );
      final state = callState();
      // Leo comes through Ana, who is not in the call but is a member of the group.
      state.contacts.carriers['g'] = {'leo': ana.id};
      expect(state.contacts.carrierName('g', 'leo'), isNull);
      expect(pathSummary(state), 'Via a member · encrypted');
      state.contacts.members['g'] = [
        const UiMember(
          away: false,
          person: ana,
          rank: 1,
          manager: false,
          me: false,
          path: UiPath.direct,
          waitingForKey: false,
          fingerprint: '7Q4M K2XR 9PLD 3VHC',
        ),
      ];
      expect(state.contacts.carrierName('g', 'leo'), 'Ana');
      expect(state.contacts.carrierName('g', 'sofia'), isNull);
      expect(pathSummary(state), 'Via Ana · encrypted');
      await pump(
        tester,
        state,
        const Align(
          alignment: Alignment.topRight,
          child: TileQuality(id: 'leo'),
        ),
      );
      settle(state, 'leo', 0, ms: 160);
      settle(state, 'sofia', 0);
      state.callState.call!.quality.theirs('leo', NetLevel.good);
      await tester.pump(const Duration(milliseconds: 300));
      await tester.tap(bars());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Through Ana, encrypted'), findsOneWidget);
      expect(find.textContaining('goes through Ana.'), findsOneWidget);
      // Ana renamed herself: the new name shows.
      state.contacts.members['g'] = [
        const UiMember(
          away: false,
          person: UiPerson(
            id: 'ana',
            name: 'Ana M.',
            short: 'EF56·GH78',
            avatar: UiAvatar(preset: 4),
          ),
          rank: 1,
          manager: false,
          me: false,
          path: UiPath.direct,
          waitingForKey: false,
          fingerprint: '7Q4M K2XR 9PLD 3VHC',
        ),
      ];
      expect(pathSummary(state), 'Via Ana M. · encrypted');
    },
  );

  testWidgets('your own details: your network and your link to each', (
    tester,
  ) async {
    final state = callState();
    await pump(
      tester,
      state,
      const Align(
        alignment: Alignment.topRight,
        child: TileQuality(id: 'me'),
      ),
    );
    // Okay first, so the bars show; they stay a few seconds once it is good.
    settle(state, 'leo', 0, ms: 200);
    settle(state, 'sofia', 0, ms: 380);
    await tester.pump(const Duration(milliseconds: 300));
    settle(state, 'leo', 2, ms: 40);
    state.callState.call!.quality.theirs('sofia', NetLevel.weak);
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(bars());
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Your connection'), findsOneWidget);
    expect(find.text('Your network'), findsOneWidget);
    expect(find.text('To Leo'), findsOneWidget);
    expect(find.text('To Sofía'), findsOneWidget);
    expect(find.text('380 ms'), findsOneWidget);
    expect(
      find.text(
        "All is well on your side. Sofía’s network is poor, so they sound choppy "
        'to everyone.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('the room list marks only those not good', (tester) async {
    final state = callState();
    await pump(
      tester,
      state,
      const Column(
        children: [
          QualityMark(id: 'me'),
          QualityMark(id: 'leo'),
          QualityMark(id: 'sofia'),
        ],
      ),
    );
    settle(state, 'leo', 0);
    settle(state, 'sofia', 0, ms: 400);
    await tester.pump();
    expect(find.byType(SignalBars), findsOneWidget);
    expect(find.bySemanticsLabel('Poor connection'), findsOneWidget);
    state.callState.call!.quality.cut('sofia');
    await tester.pump();
    expect(find.byType(SignalBars), findsNothing);
    expect(find.byType(Spinner), findsOneWidget);
  });

  testWidgets(
    'a line says when your own network is poor, 10 s after it is not',
    (tester) async {
      final state = callState();
      await pump(tester, state, const Column(children: [OwnNetworkLine()]));
      const line = 'Your network is poor: you may sound choppy to others';
      // One other only: a poor connection may be on either side, so no line.
      settle(state, 'leo', 0, ms: 400);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text(line), findsNothing);
      settle(state, 'sofia', 0, ms: 400);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text(line), findsOneWidget);
      settle(state, 'leo', 4);
      await tester.pump(const Duration(seconds: 9));
      expect(find.text(line), findsOneWidget);
      await tester.pump(const Duration(seconds: 2));
      expect(find.text(line), findsNothing);
    },
  );

  testWidgets('the own card shows your bars, and its words when poor', (
    tester,
  ) async {
    final state = callState();
    await pump(
      tester,
      state,
      Align(
        alignment: Alignment.bottomLeft,
        child: Builder(
          builder: (card) => SizedBox(
            width: 200,
            height: 48,
            child: OwnQuality(
              words: 'In the room',
              color: Fg.darkTokens.green,
              card: card,
              fallback: const Text('Measuring nothing yet'),
            ),
          ),
        ),
      ),
    );
    expect(find.text('Measuring nothing yet'), findsOneWidget);
    settle(state, 'leo', 0);
    await tester.pump();
    expect(find.text('In the room'), findsOneWidget);
    // Good: the words alone.
    expect(find.byType(SignalBars), findsNothing);
    settle(state, 'leo', 4, ms: 400);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('Poor connection'), findsOneWidget);
    await tester.tap(bars());
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Your connection'), findsOneWidget);
    // Above the card, their left edges lined up.
    final details = tester.getRect(find.byType(ConnectionDetails));
    expect(details.bottom, lessThanOrEqualTo(800 - 48 - 8 + .5));
    expect(details.left, 8);
  });

  testWidgets(
    'the room shows the pills on the tiles and the marks in the list',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      final state = callState()
        ..contacts.groups = [
          const UiGroup(
            id: 'g',
            name: 'The usual crew',
            localName: '',
            icon: 0,
            members: 3,
            online: 3,
            manager: false,
            rank: 1,
            outranks: [],
            maxMembers: 20,
            maxInRoom: 8,
            voiceRooms: [
              UiVoiceRoom(
                id: AppState.firstRoom,
                name: '',
                room: UiRoomSummary(
                  startedMs: 0,
                  participants: [me, leo, sofia],
                  joined: true,
                ),
              ),
            ],
            openInvites: 0,
            room: UiRoomSummary(
              startedMs: 0,
              participants: [me, leo, sofia],
              joined: true,
            ),
          ),
        ]
        ..roomState.roomVoice = AppState.firstRoom
        ..nav.selectedGroup = 'g'
        ..nav.page = Screen.room;
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
      settle(state, 'leo', 0, ms: 200);
      settle(state, 'sofia', 0);
      await tester.pump(const Duration(milliseconds: 400));
      // The test font draws every letter as a wide square: texts may overflow here.
      tester.takeException();
      // Each tile has its pill: Leo okay, Sofía and this device good.
      expect(find.byType(TileQuality), findsNWidgets(3));
      expect(find.text('Okay'), findsOneWidget);
      // The list marks Leo only.
      expect(find.byType(QualityMark), findsNWidgets(3));
      expect(
        find.descendant(
          of: find.byType(QualityMark),
          matching: find.byType(SignalBars),
        ),
        findsOneWidget,
      );
      // The own card has the bars instead of the path.
      expect(find.byType(OwnQuality), findsOneWidget);
      expect(find.byType(OwnNetworkLine), findsOneWidget);
    },
  );
}
