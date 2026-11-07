// The main screens in every language, and in a pseudo-language 40 % longer than English,
// at the window's smallest size, with the system's text at its size, at 1.5 times and at
// twice it: nothing overflows, and no text is cut by the control it is in.

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/api/update.dart';
import 'package:fogata/src/rust/api/voice.dart';
import 'package:fogata/src/screens/report.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/splash.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/window.dart' show splashSize;

import 'cut_text.dart';
import 'fake_room.dart';
import 'icu.dart';

const _sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

/// What overflowed or was cut while [child] showed [state] in a window of [size].
Future<List<String>> _overflows(
  WidgetTester tester,
  AppState state, {
  Widget child = const Shell(),
  Size size = const Size(960, 640),
  double scale = 1,
}) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  addTearDown(tester.view.reset);
  final found = <String>[];
  final before = FlutterError.onError;
  // Only what overflows counts here: the pages also ask the core, which tests lack.
  FlutterError.onError = (details) {
    final text = details.toString();
    if (text.contains('overflowed')) {
      found.add(
        text
            .split('\n')
            .where((l) => l.contains('overflowed') || l.contains('.dart:'))
            .take(2)
            .join(' | '),
      );
    }
  };
  try {
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Material(child: child),
          builder: appFrame,
        ),
      ),
    );
    for (var i = 0; i < 4; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    // Text cut short in its control is as unreadable as text past the window's edge.
    found.addAll(cutText());
  } finally {
    FlutterError.onError = before;
  }
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
  return found;
}

AppState _home() {
  final state = fakeRoom(people: 4)
    ..nav.page = Screen.home
    ..roomState.roomGroup = null
    ..nav.selectedGroup = null;
  state.contacts.invitations = const [
    UiGroupInvite(id: 'h', name: 'The usual crew', by: _sofia, members: 6),
  ];
  state.direct.take([
    UiDirectChat(
      away: true,
      friend: _sofia,
      online: true,
      unread: 3,
      last: 'Are we still on for Friday?',
      lastMine: false,
      lastMs: 0,
      timerMs: 0,
    ),
    UiDirectChat(
      friend: _sofia,
      online: false,
      away: false,
      unread: 0,
      last: 'Voice message · 0:18',
      lastMine: true,
      lastVoiceMs: 18000,
      lastMs: 0,
      timerMs: 0,
    ),
  ]);
  return state;
}

/// A voice message of the chat page: here and not played, coming, or not here.
UiDirectFile _voice(String id, UiFileState state) => UiDirectFile(
  id: id,
  name: 'Voice message.ogg',
  size: 60000,
  kind: UiFileKind.file,
  voice: UiVoice(
    durationMs: 838000,
    bars: Uint8List.fromList([for (var i = 0; i < 40; i++) 40 + i * 5]),
    played: false,
  ),
  state: state,
  received: 0,
);

AppState _direct() {
  final state = fakeRoom(people: 2)
    ..nav.page = Screen.direct
    ..roomState.roomGroup = null;
  final now = DateTime.now().millisecondsSinceEpoch;
  state.direct
    ..open = _sofia.id
    ..page = UiDirectPage(
      away: true,
      friend: _sofia,
      online: true,
      connected: true,
      items: [
        for (var i = 0; i < 6; i++)
          UiDirectItem(
            id: '$i',
            mine: i.isEven,
            atMs: now - (6 - i) * 26 * 3600 * 1000,
            text: 'Message number $i, a little longer than one word',
            files: const [],
            delivered: i < 4,
            expiresMs: 0,
            pinned: i == 2,
          ),
      ],
      more: false,
      pinned: const [],
      timerMs: 3600 * 1000,
      usage: const UiDirectUsage(messages: 1, pictures: 2, videos: 0, files: 3),
      altered: false,
    );
  return state;
}

/// The direct chat with voice messages in every state, and recording one, or the note
/// that no microphone can be used.
AppState _voiceChat({bool problem = false}) {
  final state = _direct();
  final page = state.direct.page!;
  final now = DateTime.now().millisecondsSinceEpoch;
  state.direct.page = UiDirectPage(
    friend: page.friend,
    online: true,
    away: false,
    connected: true,
    items: [
      for (final (i, s) in UiFileState.values.indexed)
        UiDirectItem(
          id: 'v$i',
          mine: i.isOdd,
          atMs: now - 1000 + i,
          text: '',
          files: [_voice('f$i', s)],
          delivered: true,
          expiresMs: 0,
          pinned: false,
        ),
    ],
    more: false,
    pinned: const [],
    timerMs: 0,
    usage: page.usage,
    altered: false,
  );
  final voice = state.direct.voice;
  if (problem) {
    voice.micProblem = true;
  } else {
    voice
      ..recordingFor = _sofia.id
      ..micName = 'Blue Yeti Stereo Microphone Analog Stereo'
      ..recording = UiVoiceRecording(
        elapsedMs: 852000,
        bars: Uint8List.fromList(List.filled(160, 200)),
        capturing: true,
        full: false,
        failed: false,
      );
  }
  return state;
}

AppState _members() {
  final state = fakeRoom(people: 4)
    ..nav.page = Screen.members
    ..nav.membersGroup = 'g'
    ..roomState.roomGroup = null;
  state.contacts.members['g'] = [
    for (var i = 0; i < 4; i++)
      UiMember(
        // An away member's line is the longest.
        away: i == 2,
        path: i == 2 ? UiPath.throughMember : null,
        person: i == 0 ? me : other(i),
        rank: i,
        manager: i < 2,
        me: i == 0,
        waitingForKey: i == 3,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
  ];
  return state;
}

/// A page of the settings as wide as the settings give it at 960 by 640.
class _InSettings extends StatelessWidget {
  const _InSettings(this.page);
  final Widget page;

  @override
  Widget build(BuildContext context) =>
      Center(child: SizedBox(width: 688, height: 600, child: page));
}

/// The splash at its window's size, in the middle of the test's view.
class _Splash extends StatelessWidget {
  const _Splash(this.step);
  final SplashStep step;

  @override
  Widget build(BuildContext context) => Center(
    child: SizedBox.fromSize(
      size: splashSize,
      child: SplashCard(
        step: step,
        done: 34e6.round(),
        total: 58e6.round(),
        from: 'Sofía Fernández',
      ),
    ),
  );
}

AppState _update({required bool required}) {
  final state = _home();
  state.updates.status = UiUpdate(
    thisBuild: 3,
    thisVersion: '0.1.0',
    state: UiUpdateState.ready,
    done: 0,
    total: 0,
    releaseBuild: 4,
    releaseVersion: '0.1.1',
    required_: required,
  );
  return state;
}

final _pages = <String, (AppState Function(), Widget)>{
  'room': (() => fakeRoom(people: 4, cameras: 1), const Shell()),
  'room, many': (() => fakeRoom(people: 8, cameras: 2), const Shell()),
  // The red "live" labels on a watched screen and beside those sharing.
  'room, watching a screen': (
    () => fakeRoom(people: 4, sharing: 2, watching: true),
    const Shell(),
  ),
  'home': (_home, const Shell()),
  'direct chat': (_direct, const Shell()),
  'direct chat, recording a voice message': (_voiceChat, const Shell()),
  'direct chat, no microphone': (
    () => _voiceChat(problem: true),
    const Shell(),
  ),
  'friends': (
    () => fakeRoom(people: 3)..nav.page = Screen.friends,
    const Shell(),
  ),
  'members': (_members, const Shell()),
  "the person's panel": (
    () => _home()..openUserPanel(const Rect.fromLTWH(8, 590, 290, 44)),
    const Shell(),
  ),
  'the bell': (
    () => _home()..toggleBell(const Rect.fromLTWH(900, 4, 28, 28)),
    const Shell(),
  ),
  for (final section in SettingsSection.values)
    'settings ${section.name}': (
      () => fakeRoom(people: 2)..openSettings(section),
      const Shell(),
    ),
  'settings report, what goes': (
    () => fakeRoom(people: 2),
    const _InSettings(ReportPage(listShown: true)),
  ),
  'settings report, the whole report shown': (
    () => fakeRoom(people: 2),
    const _InSettings(
      ReportPage(
        preview: {
          'report.txt': 'Fogata problem report\nVersion: 0.1.0\n\nWhat happened:\nThe call dropped.\n',
          'log/fogata-2026-10-30.log': '2026-10-30T09:12:02.400Z  INFO fogata_net::node: friend ‹fingerprint 1› online peer=‹peer 1› path=direct\n',
        },
      ),
    ),
  ),
  'settings report, e-mail ready': (
    () => fakeRoom(people: 2),
    const _InSettings(ReportPage(sent: '/home/x/fogata-report-2026-10-30.zip')),
  ),
  'an update ready': (() => _update(required: false), const Shell()),
  'an update required': (() => _update(required: true), const Shell()),
  for (final step in SplashStep.values)
    'the splash, ${step.name}': (() => fakeRoom(people: 2), _Splash(step)),
  for (var tab = 0; tab < 6; tab++)
    'group settings tab $tab': (
      () {
        final state = _members()..nav.groupSettings = 'g';
        state.nav.groupSettingsTab = tab;
        return state;
      },
      const Shell(),
    ),
};

void main() {
  setUpAll(loadFonts);
  tearDown(() => useLanguage('en'));

  testWidgets('an overflow is seen', (tester) async {
    final found = await _overflows(
      tester,
      AppState('/nonexistent/fogata-languages'),
      child: const Center(
        child: SizedBox(
          width: 40,
          child: Row(children: [Text('far too long for forty points')]),
        ),
      ),
    );
    expect(found, isNotEmpty);
  });

  for (final code in languages.keys) {
    testWidgets('the status note shows whole in $code, text ×2', (
      tester,
    ) async {
      final state = _home()
        ..openUserPanel(const Rect.fromLTWH(8, 590, 290, 44))
        ..prefs.language = code;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
            builder: appFrame,
          ),
        ),
      );
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      tester.takeException();
      // The note shown (after the hidden ones that keep its place) has the height its
      // lines need, and ends above what follows it.
      final note = find.text(l10n.userPanelOnlineNote).last;
      final paragraph = tester.renderObject<RenderParagraph>(note);
      expect(
        paragraph.size.height,
        greaterThanOrEqualTo(
          paragraph.getMinIntrinsicHeight(paragraph.size.width) - .5,
        ),
      );
      expect(
        tester.getRect(note).bottom,
        lessThanOrEqualTo(
          tester.getRect(find.text(l10n.userPanelCopyLink)).top,
        ),
      );
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    });
  }

  for (final code in [...languages.keys, 'pseudo']) {
    for (final scale in const [1.0, 1.5, 2.0]) {
      for (final MapEntry(key: name, value: (make, child)) in _pages.entries) {
        testWidgets('$name fits at 960×640 in $code, text ×$scale', (
          tester,
        ) async {
          // Set on the state's settings, which otherwise start in the system's
          // language when first read and would undo a language picked before.
          final state = make()..prefs.language = code == 'pseudo' ? 'en' : code;
          if (code == 'pseudo') l10n = PseudoL10n();
          final found = await _overflows(
            tester,
            state,
            child: child,
            scale: scale,
          );
          // The language really in use is the one asked for, not the system's.
          final arbs = readArbs();
          final word =
              arbs[code == 'pseudo'
                  ? 'en'
                  : localeOf(code).languageCode]!['settingsLanguage'];
          if (code != 'pseudo') {
            expect(l10n.settingsLanguage, word);
            expect(currentLanguage, code);
            if (code != 'en') {
              expect(word, isNot(arbs['en']!['settingsLanguage']));
            }
          } else {
            expect(l10n.settingsLanguage, isNot(word));
          }
          expect(found, isEmpty);
        });
      }
    }
  }
}
