// The main screens in every language, and in a pseudo-language 40 % longer than English,
// at the window's smallest size, with the system's text at its size, at 1.5 times and at
// twice it: nothing overflows, and no text is cut by the control it is in.

import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' show FogataApp;
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/account_move.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/recovery.dart' show UiNewRecoveryKey;
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/api/update.dart';
import 'package:fogata/src/rust/api/voice.dart';
import 'package:fogata/src/screens/members.dart';
import 'package:fogata/src/screens/move_account.dart';
import 'package:fogata/src/screens/new_group.dart';
import 'package:fogata/src/screens/recovery.dart';
import 'package:fogata/src/screens/report.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/stream_quality.dart';
import 'package:fogata/src/screens/video_view.dart';
import 'package:fogata/src/screens/switcher.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/splash.dart';
import 'package:fogata/src/state/account_move.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';
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

/// A folder to save a picture of every page tried in, when set:
///
/// `FOGATA_SHOTS=<folder> flutter test test/languages_fit_test.dart --name ' in en,'`
final _shots = Platform.environment['FOGATA_SHOTS'];

/// Saves a picture of the test's window as `<name>.png` in [_shots].
Future<void> _shot(WidgetTester tester, String name) async {
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final file = name.replaceAll(RegExp(r'[^A-Za-z0-9.×-]+'), '-');
  await tester.runAsync(() async {
    final image = await layer.toImage(Offset.zero & view.size);
    final data = await image.toByteData(format: ui.ImageByteFormat.png);
    image.dispose();
    await File('$_shots/$file.png').writeAsBytes(data!.buffer.asUint8List());
  });
}

/// What overflowed or was cut while [child] showed [state] in a window of [size], after
/// [act] if given. Saved as a picture named [name] when [_shots] is set.
Future<List<String>> _overflows(
  WidgetTester tester,
  AppState state, {
  Widget child = const Shell(),
  Size size = const Size(960, 640),
  double scale = 1,
  Future<void> Function(WidgetTester tester)? act,
  String? name,
}) async {
  final messenger = tester.binding.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  // The device panels list what the system has.
  messenger.setMockMethodCallHandler(
    const MethodChannel('FlutterWebRTC.Method'),
    (call) async => call.method == 'getSources' ? {'sources': _sources} : null,
  );
  messenger.setMockMethodCallHandler(
    const MethodChannel('FlutterWebRTC.Event'),
    (call) async => null,
  );
  addTearDown(() {
    for (final name in [
      'window_manager',
      'FlutterWebRTC.Method',
      'FlutterWebRTC.Event',
    ]) {
      messenger.setMockMethodCallHandler(MethodChannel(name), null);
    }
  });
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
    if (act != null) {
      await act(tester);
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
    }
    if (_shots != null && name != null) await _shot(tester, name);
    // Text cut short in its control is as unreadable as text past the window's edge.
    found.addAll(cutText());
  } finally {
    FlutterError.onError = before;
  }
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
  return found;
}

/// A microphone, a speaker and a camera, as the system lists them.
const _sources = [
  {
    'deviceId': 'mic-1',
    'label': 'Blue Yeti Stereo Microphone',
    'kind': 'audioinput',
  },
  {'deviceId': 'mic-2', 'label': 'Built-in Microphone', 'kind': 'audioinput'},
  {'deviceId': 'out-1', 'label': 'Speakers', 'kind': 'audiooutput'},
  {'deviceId': 'cam-1', 'label': 'Webcam', 'kind': 'videoinput'},
];

/// Shows [child] and, once it is up, what [open] opens over it (a dialog).
class _Opens extends StatefulWidget {
  const _Opens(this.open);
  final void Function(BuildContext context) open;

  @override
  State<_Opens> createState() => _OpensState();
}

class _OpensState extends State<_Opens> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.open(context);
    });
  }

  @override
  Widget build(BuildContext context) => const Shell();
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
      version: 0,
      online: true,
      connected: true,
      items: [
        for (var i = 0; i < 6; i++)
          UiDirectItem(
            id: '$i',
            seq: 0,
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
    version: 0,
    online: true,
    away: false,
    connected: true,
    items: [
      for (final (i, s) in UiFileState.values.indexed)
        UiDirectItem(
          id: 'v$i',
          seq: 0,
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
  // Waiting for three members to update, as a manager sees it in the settings.
  final g = state.contacts.groups.single;
  state.contacts.groups = [
    UiGroup(
      id: g.id,
      name: g.name,
      localName: g.localName,
      icon: g.icon,
      members: g.members,
      online: g.online,
      manager: g.manager,
      rank: g.rank,
      outranks: g.outranks,
      maxMembers: g.maxMembers,
      maxInRoom: g.maxInRoom,
      voiceRooms: g.voiceRooms,
      room: g.room,
      openInvites: g.openInvites,
      // And gone past this version: the note every member sees comes first.
      needsUpdate: true,
      // Waited long enough: the button that turns it on anyway shows too.
      raiseAnyway: true,
      levelBehind: [for (var i = 1; i < 4; i++) other(i)],
    ),
  ];
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

/// A page of the settings as large as the settings give it in the test's window: 688 by
/// 600 at 960 by 640, and as much smaller as the window is.
class _InSettings extends StatelessWidget {
  const _InSettings(this.page);
  final Widget page;

  @override
  Widget build(BuildContext context) {
    final window = MediaQuery.sizeOf(context);
    return Center(
      child: SizedBox(
        width: math.min(688, window.width - 272),
        height: math.min(600, window.height - 40),
        child: page,
      ),
    );
  }
}

/// A video over the window that cannot play here, as [video] describes it: an hour long,
/// so the time is as long as it gets.
class _Viewer extends StatelessWidget {
  const _Viewer({required this.width, required this.height});
  final int width, height;

  @override
  Widget build(BuildContext context) => VideoViewer(
    state: AppScope.read(context),
    open: () async => throw VideoProblem(l10n.roomVideoCannotPlay),
    cannotPlay: l10n.roomVideoCannotPlay,
    video: () => UiVideo(
      thumb: Uint8List(0),
      width: width,
      height: height,
      durationMs: 3723000,
    ),
    top: (context, path) => const SizedBox(),
    fullScreen: true,
  );
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

/// [state] with its one group gone past this version (see [UiGroup.needsUpdate]).
AppState _needingUpdate(AppState state) {
  final g = state.contacts.groups.single;
  state.contacts.groups = [
    UiGroup(
      id: g.id,
      name: g.name,
      localName: g.localName,
      icon: g.icon,
      members: g.members,
      online: g.online,
      manager: g.manager,
      rank: g.rank,
      managers: g.managers,
      picture: g.picture,
      banner: g.banner,
      maxMembers: g.maxMembers,
      maxInRoom: g.maxInRoom,
      voiceRooms: g.voiceRooms,
      room: g.room,
      lastRoomMs: g.lastRoomMs,
      openInvites: g.openInvites,
      outranks: g.outranks,
      keptOut: g.keptOut,
      levelBehind: g.levelBehind,
      needsUpdate: true,
      raiseAnyway: g.raiseAnyway,
    ),
  ];
  return state;
}

const _summary = UiMoveSummary(
  profile: UiProfile(person: me, fingerprint: '7Q4M K2XR 9PLD 3VHC'),
  friends: 12,
  groups: 3,
  chats: 8,
  files: 40,
  bytes: 1400000000,
);

/// An account move at [step], as the screens see it.
AccountMove _move(MoveStep step, {bool sending = false}) =>
    AccountMove(sending: sending)
      ..step = step
      ..code = 'K7QM2-XR9PD'
      ..expiresMs = DateTime.now().millisecondsSinceEpoch + 9 * 60 * 1000
      ..computer = 'Leo’s desktop'
      ..lan = true
      ..summary = step == MoveStep.looking ? null : _summary
      ..done = 880000000
      ..total = 1400000000;

AppState _movedAway(String kind) => fakeRoom(people: 1)
  ..moveState = UiMoveState(
    kind: kind,
    name: 'Marta',
    computer: 'Leo’s desktop',
    atMs: 0,
  );

/// A room with a friend (`p0`) and a member who is not one (`p1`) in it.
AppState _withFriend() {
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
    for (final (i, p) in [me, other(0), other(1)].indexed)
      UiMember(
        away: false,
        person: p,
        rank: i,
        manager: i == 0,
        me: i == 0,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
  ];
  return state;
}

/// What is opened by a click once a page is up, by the page's name.
final _acts = <String, Future<void> Function(WidgetTester tester)>{
  for (final kind in DeviceKind.values)
    'the ${kind.name} panel': (tester) async {
      await tester.tap(find.byTooltip(DeviceArrow.tooltip(kind)).last);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
    },
  'room, its chat shown': (tester) async {
    final show = find.byTooltip(l10n.roomShowChat);
    if (show.evaluate().isNotEmpty) await tester.tap(show);
  },
  'room, an emoji picked in its chat': (tester) async {
    final show = find.byTooltip(l10n.roomShowChat);
    if (show.evaluate().isNotEmpty) {
      await tester.tap(show);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
    }
    await tester.tap(find.byTooltip(l10n.emojiPickerButton).last);
  },
  'the quick switcher': (tester) async {
    await tester.tap(find.byType(FindButton));
  },
};

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
  'room, its group needing a newer Fogata': (
    () => _needingUpdate(fakeRoom(people: 4, cameras: 1)),
    const Shell(),
  ),
  'the bell, a group needing a newer Fogata': (
    () =>
        _needingUpdate(fakeRoom(people: 2))
          ..toggleBell(const Rect.fromLTWH(900, 4, 28, 28)),
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
  'the video viewer': (
    () => fakeRoom(people: 2),
    const _Viewer(width: 1280, height: 720),
  ),
  // As thin as a picture is drawn ([minAspect]), whatever the video says.
  'the video viewer, a tall video': (
    () => fakeRoom(people: 2),
    const _Viewer(width: 1, height: 4294967295),
  ),
  'the screen quality card': (
    () => fakeRoom(people: 2, own: true),
    const Center(child: StreamQualityCard()),
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
  'room, alone in it': (() => fakeRoom(people: 1), const Shell()),
  // Hidden at first in a small window, shown by hand.
  'room, its chat shown': (() => fakeRoom(people: 4), const Shell()),
  'room, an emoji picked in its chat': (
    () => fakeRoom(people: 4),
    const Shell(),
  ),
  'welcome': (() => fakeRoom(people: 1), const Welcome()),
  'welcome, bringing an account here': (
    () => fakeRoom(people: 1)
      ..bringing = true
      ..accountMove = _move(MoveStep.looking),
    const Welcome(),
  ),
  'welcome, restoring a copy': (
    () => fakeRoom(people: 1)..restoring = true,
    const Welcome(),
  ),
  'the account moved away': (() => _movedAway('moved'), const MovedScreen()),
  'the account moving away': (() => _movedAway('moving'), const MovedScreen()),
  'asked to move the account': (
    () =>
        fakeRoom(people: 2)
          ..accountMove = _move(MoveStep.matched, sending: true),
    const WithMoveQuestion(child: Shell()),
  ),
  'Fogata could not start': (
    () => fakeRoom(people: 1),
    FogataApp(
      state: AppState('/nonexistent/fogata-languages')
        ..splash.showing = false
        ..startError =
            'Fogata could not open its data folder, '
            '/home/someone/.local/share/fogata/a-rather-long-folder-name: '
            'permission denied',
    ),
  ),
  "a friend's card": (
    () =>
        _withFriend()
          ..toggleFriendCard('p0', const Rect.fromLTWH(80, 260, 232, 32)),
    const Shell(),
  ),
  "a member's card": (
    () => _withFriend()
      ..toggleFriendCard(
        'p1',
        const Rect.fromLTWH(80, 300, 232, 32),
        member: true,
        group: 'g',
      ),
    const Shell(),
  ),
  'a call ringing': (
    () => fakeRoom(people: 1)
      ..contacts.friends = [
        UiFriend(
          person: other(0),
          fingerprint: other(0).short,
          presence: UiPresence.online,
          delivered: true,
        ),
      ]
      ..callState.calls = [
        UiCall(
          groupId: 'c',
          with_: other(0),
          outgoing: false,
          phase: UiCallPhase.incoming,
          sinceMs: DateTime.now().millisecondsSinceEpoch,
          members: [other(0)],
          adding: const [],
          away: const [],
        ),
      ],
    const Shell(),
  ),
  for (final kind in DeviceKind.values)
    'the ${kind.name} panel': (
      () => fakeRoom(people: 3)..prefs.audioInput = 'mic-1',
      const Shell(),
    ),
  'the quick switcher': (() => fakeRoom(people: 3), const Shell()),
  'a new group': (() => fakeRoom(people: 2), _Opens(showNewGroup)),
  'a time-out': (
    () => _members(),
    _Opens((context) {
      final state = AppScope.read(context);
      showTimeOut(context, state.contacts.groups.single, other(1));
    }),
  ),
  'removing a member': (
    () => _members(),
    _Opens(
      (context) => confirmDialog(
        context,
        title: l10n.membersRemoveTitle(other(1).name),
        text: l10n.membersRemoveText(other(1).name),
        action: l10n.commonRemove,
      ),
    ),
  ),
  'a new recovery key': (
    () => fakeRoom(people: 2),
    _Opens(
      (context) => showFgDialog<bool>(
        context,
        builder: (dialog) => const NewKeyDialog(
          newKey: UiNewRecoveryKey(
            text:
                'ABCD EFGH JKMN PQRS TVWX YZ01 2345 6789 ABCD EFGH JKMN PQRS '
                'TVWX Y',
            id: '7Q4M K2XR',
          ),
          dataDir: '/nonexistent/fogata-languages',
        ),
      ),
    ),
  ),
  'a recovery copy': (
    () => fakeRoom(people: 2),
    _Opens(
      (context) => showFgDialog<bool>(
        context,
        builder: (dialog) =>
            const MakeCopyDialog(dataDir: '/nonexistent/fogata-languages'),
      ),
    ),
  ),
};

/// The window's sizes tried, each with the text sizes tried in it: the smallest the window
/// takes on a screen large enough, the smallest a screen gives it in practice (a Mac's
/// "Larger Text", 1024 by 571 once its menu bar is off: `mainWindowSizes`), and a
/// smaller one still, for a margin.
const _windows = [
  (Size(960, 640), [1.0, 1.5, 2.0]),
  (Size(1024, 571), [1.0, 1.5]),
  (Size(900, 600), [1.0]),
  (Size(800, 560), [1.0]),
];

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
    testWidgets(
      'the screen quality card keeps its words whole and its pills at '
      'their size in $code, text ×2',
      (tester) async {
        final state = fakeRoom(people: 2, own: true)
          ..prefs.language = code == 'pseudo' ? 'en' : code;
        if (code == 'pseudo') l10n = PseudoL10n();
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
              home: const Material(child: Center(child: StreamQualityCard())),
            ),
          ),
        );
        await tester.pump();
        final card = find.byType(StreamQualityCard);
        // Nothing in it is drawn smaller than the text size chosen.
        expect(
          find.descendant(of: card, matching: find.byType(FittedBox)),
          findsNothing,
        );
        for (final label in [
          l10n.streamQualityResolution,
          l10n.streamQualityFrameRate,
        ]) {
          final paragraph = tester.renderObject<RenderParagraph>(
            find.text(label),
          );
          // No word split across lines: the widest fits the width it was given.
          expect(
            paragraph.getMinIntrinsicWidth(double.infinity),
            lessThanOrEqualTo(paragraph.size.width + .5),
            reason: label,
          );
        }
        // The pills keep the card's width, on more lines if they need them.
        for (final key in ['stream-resolution', 'stream-fps']) {
          expect(
            tester.getSize(find.byKey(ValueKey(key))).width,
            lessThanOrEqualTo(tester.getSize(card).width - 32 + .5),
          );
        }
        await tester.pumpWidget(const SizedBox());
        await tester.pump(const Duration(seconds: 1));
      },
    );

    for (final (window, scales) in _windows) {
      final at = '${window.width.round()}×${window.height.round()}';
      for (final scale in scales) {
        for (final MapEntry(key: name, value: (make, child))
            in _pages.entries) {
          // The picker's cells keep their size, which emoji twice as large overflow.
          if (scale == 2 && name == 'room, an emoji picked in its chat') {
            continue;
          }
          testWidgets('$name fits at $at in $code, text ×$scale', (
            tester,
          ) async {
            // Set on the state's settings, which otherwise start in the system's
            // language when first read and would undo a language picked before.
            final state = make()
              ..prefs.language = code == 'pseudo' ? 'en' : code;
            if (code == 'pseudo') l10n = PseudoL10n();
            final found = await _overflows(
              tester,
              state,
              child: child,
              size: window,
              scale: scale,
              act: _acts[name],
              name: '$name $at $code x$scale',
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
}
