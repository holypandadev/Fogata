import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/api/voice.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/shown.dart';
import 'package:fogata/src/theme.dart';

const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

/// The core's side of voice messages, as far as these tests need it: a microphone that
/// opens when the test says so, and what was asked of it.
class FakeCore implements RustLibApi {
  /// The microphone opens when this completes; each start makes a new one.
  Completer<bool>? opening;

  /// The microphone is open, recording.
  bool micOpen = false;

  /// A recording is kept, under way or waiting.
  bool kept = false;

  /// The microphone stopped part way.
  bool failed = false;

  int cancels = 0;

  /// Sending waits for this, when set.
  Completer<bool>? sending;

  /// What was recorded is under half a second: sending sends nothing.
  bool tooShort = false;

  /// How long the recording is.
  int elapsedMs = 3000;

  UiVoiceRecording? get _recording => kept
      ? UiVoiceRecording(
          elapsedMs: elapsedMs,
          bars: Uint8List.fromList(List.filled(10, 180)),
          capturing: micOpen,
          full: false,
          failed: failed,
        )
      : null;

  @override
  Future<bool> crateApiVoiceVoiceRecordStart({
    String? input,
    required bool noiseSuppression,
  }) async {
    final done = opening = Completer<bool>();
    final fellBack = await done.future;
    micOpen = true;
    kept = true;
    return fellBack;
  }

  @override
  UiVoiceRecording? crateApiVoiceVoiceRecording() => _recording;

  @override
  void crateApiVoiceVoiceRecordCancel() {
    cancels++;
    micOpen = false;
    kept = false;
  }

  @override
  void crateApiVoiceVoiceRecordStop() => micOpen = false;

  @override
  Future<bool> crateApiVoiceVoiceRecordSend({required String deviceId}) async {
    micOpen = false;
    final sent = await (sending?.future ?? Future.value(true));
    if (sent) kept = false;
    return sent && !tooShort;
  }

  /// The message playing, if any.
  UiVoicePlayback? playing;

  /// What reading the chat gives.
  UiDirectPage? chat;

  @override
  Future<void> crateApiVoiceVoicePlay({
    required String deviceId,
    required String file,
    required double speed,
  }) async => playing = UiVoicePlayback(
    file: file,
    deviceId: deviceId,
    positionMs: 1200,
    durationMs: 18400,
    playing: true,
    speed: speed,
  );

  @override
  UiVoicePlayback? crateApiVoiceVoicePlayback() => playing;

  @override
  void crateApiVoiceVoiceStop() => playing = null;

  @override
  Future<UiDirectPage> crateApiDirectDirectChat({
    required String deviceId,
    required int limit,
  }) async => chat!;

  /// The attachments kept here, for one not in the page read; and how often asked.
  Set<String> keptFiles = {};
  int keptAsked = 0;

  @override
  Future<bool> crateApiDirectDirectFileKept({
    required String deviceId,
    required String file,
  }) async {
    keptAsked++;
    return keptFiles.contains(file);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

final now = DateTime.now().millisecondsSinceEpoch;

UiDirectPage chatPage(List<UiDirectItem> items, {bool more = false}) =>
    UiDirectPage(
      friend: sofia,
      online: true,
      away: false,
      connected: true,
      items: items,
      more: more,
      pinned: const [],
      timerMs: 0,
      usage: const UiDirectUsage(messages: 0, pictures: 0, videos: 0, files: 0),
      altered: false,
    );

UiDirectItem voiceItem(String id, String file, {int ago = 0}) => UiDirectItem(
  id: id,
  mine: false,
  atMs: now - ago,
  text: '',
  files: [
    UiDirectFile(
      id: file,
      name: 'Voice message.ogg',
      size: 60000,
      kind: UiFileKind.file,
      voice: UiVoice(
        durationMs: 18400,
        bars: Uint8List.fromList(List.filled(40, 120)),
        played: true,
      ),
      state: UiFileState.here,
      received: 0,
    ),
  ],
  delivered: true,
  expiresMs: 0,
  pinned: false,
);

Future<AppState> show(
  WidgetTester tester, {
  List<UiDirectItem> items = const [],
}) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState('/nonexistent/fogata-voice-recording-test')
    ..nav.page = Screen.direct;
  state.direct
    ..open = sofia.id
    ..page = chatPage(items);
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
  setUpAll(() => RustLib.initMock(api: core));
  setUp(() {
    core
      ..opening = null
      ..micOpen = false
      ..kept = false
      ..failed = false
      ..cancels = 0
      ..sending = null
      ..tooShort = false
      ..elapsedMs = 3000
      ..playing = null
      ..chat = null
      ..keptFiles = {}
      ..keptAsked = 0;
  });

  testWidgets(
    'Esc while the microphone opens: once it opens it closes, and nothing records',
    (tester) async {
      final state = await show(tester);
      final voice = state.direct.voice;
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      expect(voice.starting, isTrue);
      expect(find.byKey(const ValueKey('voice-delete')), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(voice.recordingFor, isNull);
      expect(find.byKey(const ValueKey('voice-record')), findsOneWidget);
      // The microphone opens only now.
      core.opening!.complete(false);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(core.micOpen, isFalse, reason: 'the microphone was left open');
      expect(voice.recordingFor, isNull);
      expect(voice.starting, isFalse);
      expect(voice.micProblem, isFalse);
      // A new recording starts as usual.
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      core.opening!.complete(false);
      await tester.pump();
      expect(core.micOpen, isTrue);
      expect(voice.recordingFor, sofia.id);
      expect(voice.recording?.capturing, isTrue);
      voice.cancel();
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets(
    'the chat left while the microphone opens: it stops once open, the recording waits',
    (tester) async {
      final state = await show(tester);
      final voice = state.direct.voice;
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      // Away from the chat.
      await tester.pumpWidget(const SizedBox());
      core.opening!.complete(false);
      await tester.pump();
      expect(core.micOpen, isFalse, reason: 'recording in a chat not shown');
      expect(voice.recordingFor, sofia.id);
      expect(voice.recording?.capturing, isFalse);
      voice.cancel();
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets(
    'hidden in the tray or minimized, the recording waits with the microphone off',
    (tester) async {
      addTearDown(() => windowShown.value = true);
      final state = await show(tester);
      final voice = state.direct.voice;
      // Hidden while recording, then minimized while the microphone opens.
      for (final whileOpening in [false, true]) {
        await tester.tap(find.byKey(const ValueKey('voice-record')));
        await tester.pump();
        if (!whileOpening) core.opening!.complete(false);
        await tester.pump();
        windowShown.value = false;
        if (whileOpening) core.opening!.complete(false);
        await tester.pump();
        expect(core.micOpen, isFalse, reason: 'recording unseen');
        expect(voice.recordingFor, sofia.id);
        expect(voice.recording?.capturing, isFalse);
        // Back: it waits for Send or Delete.
        windowShown.value = true;
        await tester.pump();
        expect(find.byKey(const ValueKey('voice-send')), findsOneWidget);
        expect(core.micOpen, isFalse);
        await tester.tap(find.byKey(const ValueKey('voice-delete')));
        await tester.pump();
        expect(voice.recordingFor, isNull);
      }
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('while it is sent, Delete and Esc leave it be: it goes', (
    tester,
  ) async {
    final state = await show(tester);
    final voice = state.direct.voice;
    await tester.tap(find.byKey(const ValueKey('voice-record')));
    await tester.pump();
    core.opening!.complete(false);
    await tester.pump();
    final going = core.sending = Completer<bool>();
    await tester.tap(find.byKey(const ValueKey('voice-send')));
    await tester.pump();
    expect(voice.sending, isTrue);
    await tester.tap(
      find.byKey(const ValueKey('voice-delete')),
      warnIfMissed: false,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(core.cancels, 0, reason: 'thrown away while it went');
    expect(voice.recordingFor, sofia.id);
    going.complete(true);
    await tester.pump();
    await tester.pump();
    expect(voice.sending, isFalse);
    expect(voice.recordingFor, isNull);
    expect(core.cancels, 0);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets(
    'the microphone stops part way: a note says so, the recording can be sent',
    (tester) async {
      final state = await show(tester);
      final voice = state.direct.voice;
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      core.opening!.complete(false);
      await tester.pump();
      expect(find.byKey(const ValueKey('voice-mic-stopped')), findsNothing);
      core
        ..micOpen = false
        ..failed = true;
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byKey(const ValueKey('voice-mic-stopped')), findsOneWidget);
      expect(
        find.textContaining('You can still send what was recorded.'),
        findsOneWidget,
      );
      expect(voice.recordingFor, sofia.id);
      expect(voice.micProblem, isFalse);
      await tester.tap(find.byKey(const ValueKey('voice-send')));
      await tester.pump();
      await tester.pump();
      expect(voice.recordingFor, isNull);
      expect(find.byKey(const ValueKey('voice-mic-stopped')), findsNothing);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('one too short to send goes, and a toast says why', (
    tester,
  ) async {
    final state = await show(tester);
    final voice = state.direct.voice;
    await tester.tap(find.byKey(const ValueKey('voice-record')));
    await tester.pump();
    core
      ..opening!.complete(false)
      ..tooShort = true;
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('voice-send')));
    await tester.pump();
    await tester.pump();
    expect(voice.recordingFor, isNull);
    expect(
      state.toasts.list.map((t) => t.text),
      contains(
        'Too short to send: a voice message takes at least half a second.',
      ),
    );
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets(
    'Esc deletes a recording of a few seconds; a longer one takes Delete',
    (tester) async {
      final state = await show(tester);
      final voice = state.direct.voice;
      String under() => tester
          .widget<Text>(find.byKey(const ValueKey('voice-under')))
          .textSpan!
          .toPlainText();
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      core.opening!.complete(false);
      await tester.pump();
      expect(under(), endsWith('Esc deletes it, Enter sends it'));
      core.elapsedMs = 6000;
      await tester.pump(const Duration(milliseconds: 100));
      expect(under(), endsWith(' · Enter sends it'));
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(voice.recordingFor, sofia.id, reason: 'Esc threw away 6 s');
      expect(core.cancels, 0);
      await tester.tap(find.byKey(const ValueKey('voice-delete')));
      await tester.pump();
      expect(voice.recordingFor, isNull);
      // A short one, Esc deletes.
      core.elapsedMs = 3000;
      await tester.tap(find.byKey(const ValueKey('voice-record')));
      await tester.pump();
      core.opening!.complete(false);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(voice.recordingFor, isNull);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('a message deleted while it plays stops; another one plays on', (
    tester,
  ) async {
    final state = await show(
      tester,
      items: [voiceItem('a', 'fa', ago: 60000), voiceItem('b', 'fb')],
    );
    final voice = state.direct.voice;
    await tester.tap(find.byKey(const ValueKey('voice-play:fa')));
    await tester.pump();
    expect(voice.playing('fa'), isTrue);
    // Read again with a newer message: it plays on.
    core.chat = chatPage([
      voiceItem('a', 'fa', ago: 60000),
      voiceItem('b', 'fb'),
      voiceItem('c', 'fc'),
    ]);
    await state.direct.refresh();
    await tester.pump();
    expect(voice.playing('fa'), isTrue);
    // Deleted (or its timer ran out): it stops.
    core.chat = chatPage([voiceItem('b', 'fb'), voiceItem('c', 'fc')]);
    await state.direct.refresh();
    await tester.pump();
    expect(voice.playback, isNull);
    expect(core.playing, isNull, reason: 'still playing a deleted message');
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets(
    'a message pushed out of the page by newer ones plays on, and stops once deleted',
    (tester) async {
      final state = await show(
        tester,
        items: [voiceItem('a', 'fa', ago: 60000), voiceItem('b', 'fb')],
      );
      final voice = state.direct.voice;
      await tester.tap(find.byKey(const ValueKey('voice-play:fa')));
      await tester.pump();
      expect(voice.playing('fa'), isTrue);
      // More newer messages than a page holds: the one playing is no longer read.
      final newer = [
        for (var i = 0; i < 200; i++) voiceItem('n$i', 'fn$i', ago: 200 - i),
      ];
      core
        ..keptFiles = {'fa'}
        ..chat = chatPage(newer, more: true);
      for (var read = 0; read < 2; read++) {
        await state.direct.refresh();
        await tester.pump();
        expect(voice.playing('fa'), isTrue, reason: 'read $read');
      }
      expect(core.keptAsked, 2);
      // Deleted (or its timer ran out) while out of the page: it stops all the same.
      core.keptFiles = {};
      await state.direct.refresh();
      await tester.pump();
      expect(voice.playback, isNull);
      expect(core.playing, isNull, reason: 'still playing a deleted message');
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('a screen reader is told that recording started', (tester) async {
    final handle = tester.ensureSemantics();
    final state = await show(tester);
    tester.takeAnnouncements();
    await tester.tap(find.byKey(const ValueKey('voice-record')));
    await tester.pump();
    core.opening!.complete(false);
    await tester.pump();
    expect(
      tester.takeAnnouncements().map((a) => a.message),
      contains(startsWith('Recording from ')),
    );
    state.direct.voice.cancel();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
    handle.dispose();
  });
}
