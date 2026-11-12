import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/api/voice.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/voice_ui.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/voice.dart';

const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

final now = DateTime.now().millisecondsSinceEpoch;

UiDirectFile voiceFile(
  String id, {
  UiFileState state = UiFileState.here,
  bool played = false,
}) => UiDirectFile(
  id: id,
  name: 'Voice message.ogg',
  size: 60000,
  kind: UiFileKind.file,
  voice: UiVoice(
    durationMs: 18400,
    bars: Uint8List.fromList(List.generate(40, (i) => i * 6)),
    played: played,
  ),
  state: state,
  received: 0,
);

UiDirectItem item(String id, UiDirectFile file, {bool mine = false}) =>
    UiDirectItem(
      id: id,
      seq: 0,
      mine: mine,
      atMs: now,
      text: '',
      files: [file],
      delivered: true,
      expiresMs: 0,
      pinned: false,
    );

Future<AppState> show(
  WidgetTester tester,
  List<UiDirectItem> items, {
  bool online = true,
  void Function(AppState)? before,
}) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState('/nonexistent/fogata-voice-test')
    ..nav.page = Screen.direct;
  state.direct
    ..open = sofia.id
    ..page = UiDirectPage(
      friend: sofia,
      version: 0,
      online: online,
      away: false,
      lastSeenMs: online ? null : now - 60000,
      connected: online,
      items: items,
      more: false,
      pinned: const [],
      timerMs: 0,
      usage: const UiDirectUsage(
        messages: 1,
        pictures: 0,
        videos: 0,
        files: 60000,
      ),
      altered: false,
    );
  before?.call(state);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: DirectPage()),
      ),
    ),
  );
  await tester.pump();
  // The test font draws every letter as a wide square: long lines may overflow here.
  tester.takeException();
  return state;
}

void main() {
  test('times and speeds in words', () {
    expect(voiceTime(0), '0:00');
    expect(voiceTime(18400), '0:18');
    expect(voiceTime(852000), '14:12');
    expect(voiceTime(voiceMaxMs), '15:00');
    expect([for (final s in voiceSpeeds) speedText(s)], ['1×', '1.5×', '2×']);
  });

  testWidgets(
    'a friend\'s message not played yet has a dot; the speed cycles',
    (tester) async {
      final state = await show(tester, [
        item('a', voiceFile('fa')),
        item('b', voiceFile('fb', played: true)),
        item('c', voiceFile('fc', played: true), mine: true),
      ]);
      expect(find.byKey(const ValueKey('voice-new:fa')), findsOneWidget);
      expect(find.byKey(const ValueKey('voice-new:fb')), findsNothing);
      expect(find.byKey(const ValueKey('voice-new:fc')), findsNothing);
      expect(find.text('0:18'), findsNWidgets(3));
      // The pill goes 1×, 1.5×, 2× and back, without playing.
      for (final speed in ['1.5×', '2×', '1×']) {
        await tester.tap(find.byKey(const ValueKey('voice-speed:fa')));
        await tester.pump();
        expect(
          find.descendant(
            of: find.byKey(const ValueKey('voice-speed:fa')),
            matching: find.text(speed),
          ),
          findsOneWidget,
        );
      }
      expect(state.direct.voice.speedOf('fb'), 1.0);
      // The microphone button is beside the message box.
      expect(find.byKey(const ValueKey('voice-record')), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'one that is not here waits; one that did not come has Download',
    (tester) async {
      await show(tester, [
        item('a', voiceFile('fa', state: UiFileState.missing)),
      ]);
      // Both online: it can come, so it says nothing and offers Download.
      expect(find.byKey(const ValueKey('direct-get:fa')), findsOneWidget);
      expect(find.byKey(const ValueKey('voice-speed:fa')), findsNothing);
      await tester.pumpWidget(const SizedBox());
      await show(tester, [
        item('a', voiceFile('fa', state: UiFileState.missing)),
      ], online: false);
      expect(find.text('Available when Sofía is online'), findsOneWidget);
      expect(find.byKey(const ValueKey('direct-get:fa')), findsNothing);
      // The box is locked, microphone included.
      expect(find.byKey(const ValueKey('direct-locked')), findsOneWidget);
      expect(find.byKey(const ValueKey('voice-record')), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'the recording bar: time, the last minute, what Esc and Enter do',
    (tester) async {
      UiVoiceRecording rec(int ms) => UiVoiceRecording(
        elapsedMs: ms,
        bars: Uint8List.fromList(List.filled(40, 180)),
        capturing: true,
        full: false,
        failed: false,
      );
      final state = await show(
        tester,
        const [],
        before: (s) => s.direct.voice
          ..recordingFor = sofia.id
          ..micName = 'Blue Yeti'
          ..recording = rec(4000),
      );
      expect(find.byKey(const ValueKey('voice-elapsed')), findsOneWidget);
      expect(find.text('0:04'), findsOneWidget);
      expect(find.textContaining('left'), findsNothing);
      final under = tester.widget<Text>(
        find.byKey(const ValueKey('voice-under')),
      );
      expect(
        under.textSpan!.toPlainText(),
        'Recording from Blue Yeti · Esc deletes it, Enter sends it',
      );
      // The chosen microphone could not be used: the line says so.
      state.direct.voice
        ..fellBack = true
        ..changed();
      await tester.pump();
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('voice-under')))
            .textSpan!
            .toPlainText(),
        'The chosen microphone isn’t available: recording from the default one · '
        'Esc deletes it, Enter sends it',
      );
      state.direct.voice.fellBack = false;
      // No message box while recording.
      expect(find.byKey(const ValueKey('direct-message')), findsNothing);
      state.direct.voice
        ..recording = rec(852000)
        ..changed();
      await tester.pump();
      expect(find.text('14:12'), findsOneWidget);
      expect(find.text('0:48 left'), findsOneWidget);
      state.direct.voice
        ..recording = rec(voiceMaxMs)
        ..changed();
      await tester.pump();
      expect(find.text('0:00 left'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('in a call the microphone button says why it is dimmed', (
    tester,
  ) async {
    await show(
      tester,
      const [],
      before: (s) => s.callState.calls = [
        const UiCall(
          groupId: 'g',
          with_: sofia,
          outgoing: true,
          phase: UiCallPhase.connected,
          sinceMs: 0,
          members: [sofia],
          adding: [],
          away: [],
        ),
      ],
    );
    final button = find.byKey(const ValueKey('voice-record'));
    expect(button, findsOneWidget);
    expect(
      find.descendant(
        of: find.ancestor(of: button, matching: find.byType(MicButton)),
        matching: find.byTooltip('You can’t record during a call'),
      ),
      findsOneWidget,
    );
    // A click does nothing.
    await tester.tap(button, warnIfMissed: false);
    await tester.pump();
    expect(find.byKey(const ValueKey('voice-delete')), findsNothing);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'a recording waiting in another chat is kept: no second one starts',
    (tester) async {
      final waiting = UiVoiceRecording(
        elapsedMs: 42000,
        bars: Uint8List.fromList(List.filled(40, 180)),
        capturing: false,
        full: false,
        failed: false,
      );
      final state = await show(
        tester,
        const [],
        before: (s) {
          s.direct.take([
            UiDirectChat(
              friend: const UiPerson(
                id: 'ana',
                name: 'Ana',
                short: '',
                avatar: UiAvatar(preset: 3),
              ),
              online: true,
              away: false,
              last: 'hello',
              lastMine: false,
              lastMs: now,
              unread: 0,
              timerMs: 0,
            ),
          ]);
          s.direct.voice
            ..recordingFor = 'ana'
            ..recording = waiting;
        },
      );
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      final voice = state.direct.voice;
      expect(voice.recordingFor, 'ana');
      expect(identical(voice.recording, waiting), isTrue);
      expect(
        state.toasts.list.map((t) => t.text),
        contains(
          'Send or delete the voice message in your chat with Ana first.',
        ),
      );
      // Sofía's box is still the message box.
      expect(find.byKey(const ValueKey('voice-delete')), findsNothing);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('no microphone: a note with a way to its settings', (
    tester,
  ) async {
    final state = await show(
      tester,
      const [],
      before: (s) => s.direct.voice.micProblem = true,
    );
    expect(find.byKey(const ValueKey('voice-no-mic')), findsOneWidget);
    expect(find.text('Voice & video settings'), findsOneWidget);
    await tester.tap(find.byTooltip('Close'));
    await tester.pump();
    expect(state.direct.voice.micProblem, isFalse);
    expect(find.byKey(const ValueKey('voice-no-mic')), findsNothing);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('the list of chats says a voice message and its length', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-voice-test');
    UiDirectChat chat(String id, bool mine) => UiDirectChat(
      friend: UiPerson(
        id: id,
        name: id,
        short: '',
        avatar: const UiAvatar(preset: 1),
      ),
      online: true,
      away: false,
      last: 'Voice message · 0:18',
      lastMine: mine,
      lastVoiceMs: 18400,
      lastMs: now,
      unread: 0,
      timerMs: 0,
    );
    state.direct.take([chat('Ana', true), chat('Bea', false)]);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: DirectList()),
        ),
      ),
    );
    expect(find.text('You: Voice message · 0:18'), findsOneWidget);
    expect(find.text('Voice message · 0:18'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
}
