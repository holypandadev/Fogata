// Two copies of the app while things change under a call: someone comes and goes many
// times in a row, renames themselves, writes a bio and empties it again, gives themselves a
// moving picture and banner while the group gets a moving banner, is removed as a
// friend in the middle of a direct call, is kept out of the room for a while and let back,
// and is removed from the group while in its room. Build once with
//
//   flutter build linux --debug -t integration_test/changes_test.dart
//
// and start two copies, FOGATA_ROLE=a and b, each with its own FOGATA_DATA_DIR and the
// same FOGATA_CALL folder, where they leave each other the friend link and their steps.
// Each copy writes <role>.log and <role>-result.txt ("passed" or why it failed) there, and
// pictures of the cards with the bio, and of the main screens in every theme.
// Nobody's microphone or camera is used. On Linux, copy a also checks that the network
// sockets the call opened are let go again.

import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/moving.dart';
import 'package:fogata/src/names.dart';
import 'package:fogata/src/photo.dart';
import 'package:fogata/src/rust/api/app.dart' as api;
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/room.dart' show openRoomChecked;
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/volume.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _dir = _env['FOGATA_CALL'] ?? Directory.systemTemp.path;
final _other = _role == 'a' ? 'b' : 'a';

void _log(String line) => File('$_dir/$_role.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _until(
  WidgetTester tester,
  String what,
  bool Function() done, {
  int seconds = 120,
}) async {
  final end = DateTime.now().add(Duration(seconds: seconds));
  while (!done()) {
    if (DateTime.now().isAfter(end)) throw StateError('not in time: $what');
    await _wait(tester, const Duration(milliseconds: 200));
  }
  _log(what);
}

/// Saves a picture of the window in the folder the two copies share.
Future<void> _shot(WidgetTester tester, String name) async {
  await _wait(tester, const Duration(milliseconds: 700));
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(Offset.zero & view.size);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_dir/$_role-$name.png').writeAsBytes(data!.buffer.asUint8List());
}

/// Says a step is done, for the other copy.
void _mark(String step) {
  File('$_dir/$_role-$step').writeAsStringSync('');
  _log('step: $step');
}

/// Waits for the other copy to say a step is done.
Future<void> _waitFor(WidgetTester tester, String step, {int seconds = 300}) =>
    _until(
      tester,
      '$_other did $step',
      () => File('$_dir/$_other-$step').existsSync(),
      seconds: seconds,
    );

AppState _state(WidgetTester tester) =>
    AppScope.read(tester.element(find.byType(Shell)));

/// UDP sockets this process holds open (Linux only), each as its local address (and the
/// remote one, when it has one): each call connection opens a few, one for each network of
/// the computer; the DHT's connections are TCP and QUIC on one port.
List<String>? _sockets() {
  if (!Platform.isLinux) return null;
  final udp = <String, String>{};
  for (final table in ['/proc/self/net/udp', '/proc/self/net/udp6']) {
    try {
      for (final line in File(table).readAsLinesSync().skip(1)) {
        final fields = line.trim().split(RegExp(r'\s+'));
        if (fields.length > 9) {
          final remote = _address(fields[2]);
          udp[fields[9]] = remote.endsWith(':0')
              ? _address(fields[1])
              : '${_address(fields[1])} > $remote';
        }
      }
    } catch (_) {}
  }
  // A descriptor closed while the folder is listed fails the listing: list again.
  var fds = const <FileSystemEntity>[];
  for (var i = 0; i < 5; i++) {
    try {
      fds = Directory('/proc/self/fd').listSync();
      break;
    } on FileSystemException {
      continue;
    }
  }
  final open = <String>[];
  for (final entry in fds) {
    try {
      final target = Link(entry.path).targetSync();
      if (!target.startsWith('socket:[')) continue;
      final address = udp[target.substring(8, target.length - 1)];
      if (address != null) open.add(address);
    } catch (_) {}
  }
  return open..sort();
}

/// Whether more sockets are open than before, past a couple that come and go.
bool _tooMany(List<String>? before, List<String>? after) =>
    before != null && after != null && after.length > before.length + 2;

/// An address of /proc/net/udp(6): the IP in hex, by 32-bit words in this computer's byte
/// order, and the port in hex.
String _address(String field) {
  final parts = field.split(':');
  final hex = parts[0];
  final port = int.parse(parts[1], radix: 16);
  final bytes = <int>[];
  for (var w = 0; w + 8 <= hex.length; w += 8) {
    final word = hex.substring(w, w + 8);
    for (var b = 6; b >= 0; b -= 2) {
      bytes.add(int.parse(word.substring(b, b + 2), radix: 16));
    }
  }
  if (bytes.length == 4) return '${bytes.join('.')}:$port';
  final groups = [
    for (var i = 0; i < 16; i += 2)
      ((bytes[i] << 8) | bytes[i + 1]).toRadixString(16),
  ];
  return '[${groups.join(':')}]:$port';
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('changes under a call', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      await _run(tester);
      File('$_dir/$_role-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$_role-result.txt').writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

Future<void> _run(WidgetTester tester) async {
  await app.main();
  await _until(
    tester,
    'welcome',
    () => find.byType(TextField).evaluate().isNotEmpty,
  );
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      _role == 'a' ? 'Ana' : 'Bea';
  await _wait(tester, const Duration(milliseconds: 300));
  await tester.tap(find.text('Create'));
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  // Nothing of the test is heard: no sounds, no microphone, no camera.
  state.prefs.sounds = false;
  state.prefs.joinWithMic = false;
  state.callState.joinWithCamera = false;

  if (_role == 'a') {
    final link = await api.friendLink();
    File('$_dir/link-b.txt').writeAsStringSync(link.text);
    await _until(
      tester,
      'a friend request',
      () => state.contacts.requests.incoming.isNotEmpty,
      seconds: 300,
    );
    await state.answer(state.contacts.requests.incoming.first.person.id, true);
    await _until(tester, 'friends', () => state.contacts.friends.isNotEmpty);
    await state.createGroup('Tests', 3, [
      state.contacts.friends.single.person.id,
    ]);
    await _until(tester, 'group', () => state.contacts.groups.isNotEmpty);
  } else {
    final file = File('$_dir/link-b.txt');
    await _until(
      tester,
      'link',
      () => file.existsSync() && file.readAsStringSync().isNotEmpty,
      seconds: 300,
    );
    await state.useInvitation(file.readAsStringSync());
    await _until(
      tester,
      'invited to the group',
      () => state.contacts.invitations.isNotEmpty,
      seconds: 300,
    );
    await state.answerInvitation(state.contacts.invitations.first.id, true);
    await _until(
      tester,
      'in the group',
      () => state.contacts.groups.isNotEmpty,
    );
  }
  final group = state.contacts.groups.single.id;
  final friend = state.contacts.friends.single.person;

  await _bio(tester, state, friend);
  await _looks(tester, state, group, friend);
  await _comings(tester, state, group, friend);
  await _rename(tester, state, friend);
  await _direct(tester, state, friend);
  await _keptOut(tester, state, group, friend);
  await _removal(tester, state, group, friend);
}

/// Pictures of the test's own, next to the clip (`FOGATA_CLIP`): an orbiting dot (96 by
/// 96 pixels) and waves (240 by 135), 8 frames each.
Uint8List _testGif(String name) {
  final clip = _env['FOGATA_CLIP'];
  final file = File(
    '${clip == null ? 'integration_test' : File(clip).parent.path}/pictures/$name',
  );
  return file.existsSync() ? file.readAsBytesSync() : Uint8List(0);
}

final _avatarGif = _testGif('orbit.gif');
final _bannerGif = _testGif('waves.gif');

/// Milliseconds of processor time each thread of this process used, by name (Linux only).
Map<String, int> _threadMs() {
  final out = <String, int>{};
  if (!Platform.isLinux) return out;
  for (final task in Directory('/proc/self/task').listSync()) {
    try {
      final stat = File('${task.path}/stat').readAsStringSync();
      final name = stat.substring(stat.indexOf('(') + 1, stat.lastIndexOf(')'));
      final fields = stat.substring(stat.lastIndexOf(') ') + 2).split(' ');
      // utime and stime, in clock ticks of 10 ms.
      final ms = (int.parse(fields[11]) + int.parse(fields[12])) * 10;
      out['$name ${task.path.split('/').last}'] = ms;
    } catch (_) {}
  }
  return out;
}

/// The share of one processor used over [seconds], in percent: by the whole process, and
/// by the threads that run the interface and draw it, and by those that decode pictures. Nothing
/// is pumped meanwhile: the window draws only what asks to be drawn.
Future<String> _cpuOver(int seconds) async {
  final before = _threadMs();
  final began = DateTime.now();
  await Future<void>.delayed(Duration(seconds: seconds));
  final after = _threadMs();
  if (after.isEmpty) return 'n/a';
  final wall = DateTime.now().difference(began).inMilliseconds;
  var all = 0, drawing = 0, decoding = 0;
  final main = 'fogata $pid';
  for (final MapEntry(:key, :value) in after.entries) {
    final used = value - (before[key] ?? 0);
    all += used;
    // The main thread runs Dart's interface code here; the raster thread and the
    // software renderer's threads draw.
    if (key == main ||
        key.startsWith('io.flutter.rast') ||
        key.startsWith('llvmpipe')) {
      drawing += used;
    }
    // Pictures are decoded on the io threads.
    if (key.startsWith('io.')) decoding += used;
  }
  String pc(int ms) => '${(100 * ms / wall).toStringAsFixed(1)} %';
  return '${pc(all)} (drawing ${pc(drawing)}, decoding ${pc(decoding)})';
}

/// Moving pictures: Bea gives herself an animated picture and card banner, Ana gives the
/// group an animated banner. Each sees the other's move; Ana measures what a card with
/// both costs while focused, while not, and closed; then pictures of the main screens in
/// every theme.
Future<void> _looks(
  WidgetTester tester,
  AppState state,
  String group,
  UiPerson friend,
) async {
  if (_avatarGif.isEmpty || _bannerGif.isEmpty) {
    _log('looks: no GIFs given, skipped');
    return;
  }
  if (_role == 'b') {
    final still = await squarePicture(_avatarGif);
    if (!await state.updateProfile('Bea', 0, still, moving: _avatarGif)) {
      throw StateError('the animated picture was refused');
    }
    final banner = await bannerPng(_bannerGif);
    await state.setBannerPicture(banner!, moving: _bannerGif);
    _mark('moving');
    await _until(
      tester,
      'the group banner',
      () => state.contacts.groupBanner(state.contacts.group(group)) != null,
      seconds: 60,
    );
    state.selectGroup(group);
    await _shots(tester, state, 'group-banner');
    _mark('saw-group-banner');
    await _waitFor(tester, 'looked', seconds: 600);
    return;
  }
  await state.setGroupBanner(group, _bannerGif);
  await _waitFor(tester, 'moving');
  // Opened as a click opens it, which asks for the friend's banner as it is now: Bea's
  // may have come while no card was open.
  const from = Rect.fromLTWH(300, 140, 200, 40);
  state.toggleFriendCard(friend.id, from);
  UiFriend bea() =>
      state.contacts.friends.firstWhere((f) => f.person.id == friend.id);
  await _until(
    tester,
    'Bea\'s picture moves',
    () => movingBook.photo(friend.id, bea().person.avatar.photo) != null,
    seconds: 60,
  );
  await _until(tester, 'Bea\'s banner moves', () {
    // While the card is open, a banner that comes is shown at the next refresh.
    final banner = state.contacts.friendBanners[friend.id];
    return movingBook.banner(friend.id, banner?.picture) != null;
  }, seconds: 60);
  await _shot(tester, 'card-moving');
  // What the card costs: closed, open and moving, open with the window not focused, in
  // turns, after the start has settled.
  Future<void> card(bool open) async {
    if (open && state.nav.friendCard != friend.id) {
      state.toggleFriendCard(friend.id, from);
    } else if (!open) {
      state.closeFriendCard();
    }
    await _wait(tester, const Duration(seconds: 1));
  }

  await card(false);
  await _cpuOver(10);
  for (var turn = 1; turn <= 3; turn++) {
    await card(false);
    _log('cpu $turn, card closed: ${await _cpuOver(15)}');
    await card(true);
    _log('cpu $turn, card moving, focused: ${await _cpuOver(15)}');
    windowFocused.value = false;
    _log('cpu $turn, card open, window not focused: ${await _cpuOver(15)}');
    windowFocused.value = true;
  }
  await card(false);
  await _waitFor(tester, 'saw-group-banner');

  // The main screens in every theme, at 1280x800.
  await state.openRoom(group);
  await _until(tester, 'in the room', () => state.callState.call != null);
  await state.sendChat('Who is up for a ride on Sunday?');
  // Past "Connecting to the room".
  await _wait(tester, const Duration(seconds: 4));
  final view = tester.view;
  view.physicalSize = const Size(1280, 800) * view.devicePixelRatio;
  for (final look in Look.values.where((l) => l != Look.system)) {
    await state.setLook(look);
    state.goHome();
    await _shot(tester, 'theme-${look.name}-1-home');
    await state.openRoom(group);
    await _shot(tester, 'theme-${look.name}-2-room');
    state.direct.show(friend.id);
    await _until(tester, 'the chat', () => state.direct.page != null);
    if (look == Look.dark) {
      await state.direct.send('See you there, I will bring the map.');
    }
    await _shot(tester, 'theme-${look.name}-3-dm');
    state.openSettings(SettingsSection.appearance);
    await _shot(tester, 'theme-${look.name}-4-settings');
    state.closeSettings();
  }
  await state.setLook(Look.dark);
  view.resetPhysicalSize();
  await state.leaveRoom();
  state.goHome();
  await _wait(tester, const Duration(seconds: 2));
  _mark('looked');
}

/// Pictures of the window at 1280x800, dark and light.
Future<void> _shots(WidgetTester tester, AppState state, String name) async {
  final view = tester.view;
  view.physicalSize = const Size(1280, 800) * view.devicePixelRatio;
  await _shot(tester, '$name-dark');
  state.setTheme(ThemeMode.light);
  await _shot(tester, '$name-light');
  state.setTheme(ThemeMode.dark);
  view.resetPhysicalSize();
  await _wait(tester, const Duration(milliseconds: 300));
}

/// Both in the room; Ana, from Bea's menu, keeps her out of it for an hour, with a second
/// room only Ana (first in the group) may enter. Bea leaves, is told why, cannot go back in, and the rest of
/// the group goes on; Ana lets her back from the group settings and she comes in again.
Future<void> _keptOut(
  WidgetTester tester,
  AppState state,
  String group,
  UiPerson friend,
) async {
  const general = AppState.firstRoom;
  await state.openRoom(group);
  await _until(
    tester,
    'both in the room again',
    () => (state.roomState.room?.participants.length ?? 0) == 2,
    seconds: 120,
  );
  _mark('both-again');
  await _waitFor(tester, 'both-again');
  if (_role == 'a') {
    final gaming = await api.setVoiceRoom(
      groupId: group,
      voice: null,
      name: 'Gaming',
    );
    await state.setRoomAccess(group, gaming, const []);
    await _until(
      tester,
      'Gaming for Ana only',
      () => state.contacts.voiceRoom(group, gaming)?.only?.isEmpty ?? false,
    );
    final tile = find.byWidgetPredicate(
      (w) => w is VolumeTarget && w.person.id == friend.id,
    );
    await _until(tester, "Bea's tile", () => tile.evaluate().isNotEmpty);
    // Taken out with no time: she leaves, and may come back at once.
    await tester.tap(tile.last, buttons: kSecondaryButton);
    await _until(
      tester,
      'the menu',
      () => find.text('Remove from General').evaluate().isNotEmpty,
      seconds: 10,
    );
    await tester.tap(find.text('Remove from General'));
    // It asks first.
    await _until(
      tester,
      'the question',
      () => find.text('Remove').evaluate().isNotEmpty,
      seconds: 10,
    );
    await tester.tap(find.text('Remove'));
    await _until(
      tester,
      'Bea taken out of the room',
      () => state.roomState.room?.participants.length == 1,
      seconds: 20,
    );
    _mark('removed');
    await _until(
      tester,
      'Bea back at once',
      () => state.roomState.room?.participants.length == 2,
      seconds: 60,
    );
    await _until(tester, "Bea's tile again", () => tile.evaluate().isNotEmpty);
    tester.view.physicalSize =
        const Size(1280, 800) * tester.view.devicePixelRatio;
    await _wait(tester, const Duration(milliseconds: 500));
    await tester.tap(tile.last, buttons: kSecondaryButton);
    await _until(
      tester,
      'the part of the menu for those above',
      () => find.text('Time out of General').evaluate().isNotEmpty,
      seconds: 10,
    );
    await tester.tap(find.text('Time out of General'));
    await _until(
      tester,
      'the durations',
      () => find.text('1 hour').evaluate().isNotEmpty,
      seconds: 10,
    );
    await _shots(tester, state, 'menu-time-out');
    await tester.tap(find.text('1 hour'));
    await _until(
      tester,
      'Bea out of the room',
      () => state.roomState.room?.participants.length == 1,
      seconds: 20,
    );
    state.openGroupSettings(group, tab: 1);
    await _until(
      tester,
      'Bea kept out in the settings',
      () => find.text('Let back').evaluate().isNotEmpty,
      seconds: 20,
    );
    await tester.tap(find.text('Only some'));
    await _wait(tester, const Duration(milliseconds: 600));
    await _shots(tester, state, 'settings-kept-out');
    _mark('kept-out');
    await _waitFor(tester, 'tried', seconds: 120);
    // Still out after Bea tried to come back.
    if (state.roomState.room?.participants.length != 1) {
      throw StateError('Bea came back into the room while kept out');
    }
    await tester.ensureVisible(find.text('Let back'));
    await _wait(tester, const Duration(milliseconds: 300));
    await tester.tap(find.text('Let back'));
    await _until(
      tester,
      'nobody kept out',
      () => state.contacts.group(group)?.keptOut?.isEmpty ?? false,
      seconds: 20,
    );
    state.closeGroupSettings();
    await state.setRoomAccess(group, gaming, null);
    _mark('let-back');
    await _until(
      tester,
      'Bea back in the room',
      () => state.roomState.room?.participants.length == 2,
      seconds: 60,
    );
    return;
  }
  await _waitFor(tester, 'removed', seconds: 120);
  await _until(
    tester,
    'taken out with no time, and told',
    () =>
        state.contacts.joinedVoice(group) == null &&
        state.toasts.list.any((t) => t.text == 'Ana took you out of General.'),
    seconds: 20,
  );
  await _shots(tester, state, 'removed');
  await state.openRoom(group, voice: general);
  await _until(
    tester,
    'back in at once',
    () => (state.roomState.room?.participants.length ?? 0) == 2,
    seconds: 60,
  );
  // Pictures while the toast still shows.
  await _until(
    tester,
    'taken out, and told why',
    () =>
        state.contacts.joinedVoice(group) == null &&
        state.toasts.list.any(
          (t) => t.text == 'Ana took you out of General for 1 hour.',
        ),
    seconds: 120,
  );
  if (state.callState.call != null) throw StateError('the call goes on');
  await _shots(tester, state, 'taken-out');
  await _waitFor(tester, 'kept-out', seconds: 120);
  await _until(
    tester,
    'General with a clock, Gaming with a lock',
    () =>
        state.contacts.voiceRoom(group, general)?.barrier?.kind ==
            UiBarrierKind.timedOut &&
        find.text('Not for you now').evaluate().isNotEmpty,
    seconds: 20,
  );
  await _shots(tester, state, 'kept-out');
  // Join stays off: the room in the list says until when, and the core says no too.
  await openRoomChecked(
    tester.element(find.byType(Shell)),
    group,
    voice: general,
  );
  await _until(
    tester,
    'told when it can come back',
    () => state.toasts.list.any(
      (t) => t.text.startsWith('You can come back to General in '),
    ),
    seconds: 10,
  );
  var refused = false;
  try {
    await api.joinRoom(groupId: group, voice: general);
  } catch (e) {
    refused = true;
    _log('the core says: $e');
  }
  if (!refused) throw StateError('the core let a member kept out in');
  await _wait(tester, const Duration(seconds: 5));
  if (state.contacts.joinedVoice(group) != null) {
    throw StateError('in the room again');
  }
  // The rest of the group is as before.
  if (state.contacts.groups.single.id != group) {
    throw StateError('the group is gone');
  }
  _mark('tried');
  await _waitFor(tester, 'let-back', seconds: 120);
  await _until(
    tester,
    'let back',
    () => state.contacts.voiceRoom(group, general)?.barrier == null,
    seconds: 30,
  );
  await state.openRoom(group, voice: general);
  await _until(
    tester,
    'in the room again',
    () => (state.roomState.room?.participants.length ?? 0) == 2,
    seconds: 60,
  );
}

/// Bea comes into the room and goes again many times in a row while Ana stays: every
/// connection made for her is let go again.
Future<void> _comings(
  WidgetTester tester,
  AppState state,
  String group,
  UiPerson friend,
) async {
  if (_role == 'a') {
    await state.openRoom(group);
    await _until(tester, 'in the room', () => state.callState.call != null);
    await _wait(tester, const Duration(seconds: 3));
    // The fewest over a few seconds, without the node's lookups of the moment (below).
    var before = _sockets();
    for (var i = 0; i < 4; i++) {
      await _wait(tester, const Duration(seconds: 1));
      final now = _sockets();
      if (now != null && before != null && now.length < before.length) {
        before = now;
      }
    }
    _log('sockets before: ${before?.length} ${before?.join(', ')}');
    _mark('alone');
    await _waitFor(tester, 'comings');
    await _until(
      tester,
      'Bea gone from the call',
      () => state.callState.call!.peers.isEmpty,
      seconds: 60,
    );
    // Connections closing take a moment to let their sockets go. The node also opens a
    // socket of its own for each name it looks up (each on a random port, gone within a
    // second or two), and while it looks up the DHT's peers there may be a dozen or more
    // at once: what is left open must stay over for a while to count.
    await _wait(tester, const Duration(seconds: 8));
    var after = _sockets();
    _log('sockets after: ${after?.length} ${after?.join(', ')}');
    for (var i = 0; i < 60 && _tooMany(before, after); i++) {
      await _wait(tester, const Duration(seconds: 1));
      after = _sockets();
    }
    _log('sockets at last: ${after?.length} ${after?.join(', ')}');
    if (_tooMany(before, after)) {
      throw StateError(
        'sockets left open: ${before!.length} before, ${after!.length} after',
      );
    }
    _mark('counted');
  } else {
    await _waitFor(tester, 'alone');
    for (var i = 0; i < 15; i++) {
      await state.openRoom(group);
      await _wait(tester, Duration(milliseconds: 150 + 70 * (i % 6)));
      await state.leaveRoom();
      await _wait(tester, Duration(milliseconds: 100 + 90 * (i % 5)));
    }
    _mark('comings');
    await _waitFor(tester, 'counted');
    // In again, to stay for the next steps.
    await state.openRoom(group);
  }
  await _until(
    tester,
    'connected to ${friend.name}',
    () => state.callState.call?.peers[friend.id]?.connected ?? false,
    seconds: 120,
  );
}

/// Bea renames herself in the room: Ana sees the new name on her tile and on what the
/// room noted of her.
Future<void> _rename(
  WidgetTester tester,
  AppState state,
  UiPerson friend,
) async {
  if (_role == 'b') {
    final me = state.contacts.profile!.person;
    await state.updateProfile('Beatriz', me.avatar.preset, me.avatar.photo);
    _mark('renamed');
    await _waitFor(tester, 'saw-rename');
    return;
  }
  await _waitFor(tester, 'renamed');
  await _until(
    tester,
    'the new name on Bea\'s tile',
    () =>
        state.roomState.room?.participants.any(
          (p) => p.person.id == friend.id && p.person.name == 'Beatriz',
        ) ??
        false,
    seconds: 30,
  );
  final notes =
      state.roomState.memories[state.roomState.roomGroup]?.notes ??
      const <RoomNote>[];
  final hers = notes.where((n) => n.device == friend.id).toList();
  if (hers.isEmpty) throw StateError('the room noted nothing of Bea');
  for (final note in hers) {
    final shown = state.nameOf(note.device, note.name);
    if (shown != 'Beatriz') throw StateError('a note still says $shown');
  }
  _log('${hers.length} notes show the new name');
  _mark('saw-rename');
}

/// Bea writes a bio in My profile, as a person does: Ana's card of her shows it at once,
/// where it showed the default before; Bea empties it, and the card shows none.
Future<void> _bio(WidgetTester tester, AppState state, UiPerson friend) async {
  const written = 'Bea from Tests.\nHere on weekends, ask me for a game.';
  final card = find.byKey(const ValueKey('card-bio'));
  String? shown() =>
      card.evaluate().isEmpty ? null : tester.widget<Text>(card).data;
  if (_role == 'b') {
    state.openSettings(SettingsSection.profile);
    await _until(
      tester,
      'My profile',
      () => find.byKey(const ValueKey('bio-field')).evaluate().isNotEmpty,
    );
    await tester.enterText(find.byKey(const ValueKey('bio-field')), written);
    await _wait(tester, const Duration(milliseconds: 300));
    await tester.ensureVisible(find.text('Save'));
    await _wait(tester, const Duration(milliseconds: 300));
    await tester.tap(find.text('Save'));
    await _until(tester, 'bio saved', () => state.contacts.bio == written);
    await _shot(tester, 'bio-profile');
    state.closeSettings();
    _mark('bio');
    await _waitFor(tester, 'saw-bio');
    await state.updateBio('');
    await _until(tester, 'bio emptied', () => state.contacts.bio == '');
    _mark('no-bio');
    await _waitFor(tester, 'saw-no-bio');
    return;
  }
  // Never written: the card shows the default.
  final before = state.contacts.friends.single;
  if (before.bio != null) throw StateError('a bio before any: ${before.bio}');
  state
    ..nav.friendCard = friend.id
    ..nav.friendCardFrom = const Rect.fromLTWH(300, 140, 200, 40);
  state.notifyListeners();
  await _until(tester, 'the card', () => shown() == defaultBio, seconds: 10);
  await _shot(tester, 'card-default-bio');
  await _waitFor(tester, 'bio');
  await _until(
    tester,
    'Bea\'s bio on her card',
    () => state.contacts.friends.single.bio == written && shown() == written,
    seconds: 30,
  );
  await _shot(tester, 'card-bio');
  state.setTheme(ThemeMode.light);
  await _shot(tester, 'card-bio-light');
  state.setTheme(ThemeMode.dark);
  _mark('saw-bio');
  await _waitFor(tester, 'no-bio');
  await _until(
    tester,
    'no bio on Bea\'s card',
    () => state.contacts.friends.single.bio == '' && card.evaluate().isEmpty,
    seconds: 30,
  );
  await _shot(tester, 'card-no-bio');
  state.closeFriendCard();
  _mark('saw-no-bio');
}

/// A direct call: Bea renames herself during it, then removes Ana from her friends. Ana's
/// call shows the new name all along, never a key.
Future<void> _direct(
  WidgetTester tester,
  AppState state,
  UiPerson friend,
) async {
  await state.leaveRoom();
  await _until(tester, 'out of the room', () => state.callState.call == null);
  _mark('out');
  await _waitFor(tester, 'out');
  if (_role == 'a') {
    await state.callFriend(friend.id);
  } else {
    await _until(tester, 'rung', () => state.callState.incomingCall != null);
    await state.acceptCall(state.callState.incomingCall!, micOff: true);
  }
  await _until(
    tester,
    'in the call',
    () => state.callState.activeCall?.phase == UiCallPhase.connected,
    seconds: 60,
  );
  _mark('in-call');
  await _waitFor(tester, 'in-call');
  if (_role == 'b') {
    final me = state.contacts.profile!.person;
    await state.updateProfile('Bea Torres', me.avatar.preset, me.avatar.photo);
    await _waitFor(tester, 'saw-name');
    // Removing a friend hangs up the direct call with them, on both sides.
    await state.removeFriend(friend.id);
    await _until(tester, 'call over', () => state.callState.activeCall == null);
    await _waitFor(tester, 'hung-up');
    return;
  }
  await _until(
    tester,
    'the new name in the call',
    () =>
        state.callState.activeCall?.with_.name == 'Bea Torres' &&
        find.textContaining('Call with Bea Torres').evaluate().isNotEmpty,
    seconds: 30,
  );
  _mark('saw-name');
  await _until(
    tester,
    'no longer friends',
    () => state.contacts.friends.isEmpty,
  );
  await _until(
    tester,
    'the call ended by the removal',
    () => state.callState.activeCall == null,
    seconds: 30,
  );
  _mark('hung-up');
}

/// Both in the group's room; Ana removes Bea from the group. Bea leaves Ana's room at once,
/// and Bea's window lets the group go.
Future<void> _removal(
  WidgetTester tester,
  AppState state,
  String group,
  UiPerson friend,
) async {
  await state.openRoom(group);
  await _until(
    tester,
    'both in the room',
    () => (state.roomState.room?.participants.length ?? 0) == 2,
    seconds: 120,
  );
  _mark('both');
  await _waitFor(tester, 'both');
  if (_role == 'a') {
    await api.removeFromGroup(groupId: group, memberId: friend.id);
    await _until(
      tester,
      'Bea out of the room',
      () => state.roomState.room?.participants.length == 1,
      seconds: 10,
    );
  } else {
    await _until(
      tester,
      'the group gone',
      () => state.contacts.groups.isEmpty && state.nav.page == Screen.home,
      seconds: 30,
    );
    if (state.callState.call != null) throw StateError('the call goes on');
  }
}
