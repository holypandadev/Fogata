// The app with another device: tool/peer.sh runs a command-line Fogata called Marta. The
// waits are long enough for two devices on different networks finding each other through
// the internet.
//
//   tool/peer.sh <fogata> <folder B> <peer output> &
//   FOGATA_DATA_DIR=<empty folder> FOGATA_SHOTS=<folder> FOGATA_PEER=<peer output> \
//     flutter test integration_test/with_friend_test.dart -d linux

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/names.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/volume.dart';
import 'package:fogata/src/widgets.dart';
import 'package:integration_test/integration_test.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];
final _peer = Platform.environment['FOGATA_PEER']!;

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _shot(WidgetTester tester, String name) async {
  await _wait(tester, const Duration(milliseconds: 400));
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(Offset.zero & view.size);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

Future<void> _until(
  WidgetTester tester,
  Finder finder, {
  int seconds = 30,
}) async {
  final end = DateTime.now().add(Duration(seconds: seconds));
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(end)) {
      await _shot(tester, 'failed');
      fail('not shown in time: $finder');
    }
    await _wait(tester, const Duration(milliseconds: 200));
  }
}

Future<void> _type(WidgetTester tester, Finder field, String text) async {
  await _until(tester, field);
  tester.widget<TextField>(field.first).controller!.text = text;
  await _wait(tester, const Duration(milliseconds: 200));
}

Future<void> _tap(
  WidgetTester tester,
  Finder finder, {
  int seconds = 30,
}) async {
  await _until(tester, finder, seconds: seconds);
  await tester.ensureVisible(finder.first);
  await _wait(tester, const Duration(milliseconds: 200));
  await tester.tap(finder.first);
  await _wait(tester, const Duration(milliseconds: 300));
}

Future<String> _link() async {
  final file = File('$_peer/invite.out');
  final end = DateTime.now().add(const Duration(minutes: 10));
  while (DateTime.now().isBefore(end)) {
    if (file.existsSync()) {
      final match = RegExp(r'fogata:friend:\S+')
          .firstMatch(file.readAsStringSync());
      if (match != null) return match.group(0)!;
    }
    await Future<void>.delayed(const Duration(milliseconds: 300));
  }
  throw StateError('the other device showed no link');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('friend, group, room, chat and a file', (tester) async {
    await app.main();
    await continueUnencryptedIfAsked(tester);
    await _type(tester, find.byType(TextField), 'Leo');
    await _tap(tester, find.text('Create'));
    await _until(tester, find.text('No groups yet'));
    // Rooms start their call with the microphone off: the test uses no real device.
    AppScope.read(tester.element(find.byType(Shell))).prefs.joinWithMic = false;
    // A bio goes to Marta with the greeting; a version without bios turns it down, and
    // everything else carries on.
    final me = AppScope.read(tester.element(find.byType(Shell)));
    expect(await me.updateBio('Leo, trying Fogata out.'), isTrue);

    // Friends: use Marta's link.
    await _tap(tester, find.widgetWithText(Pressable, 'Friends').first);
    await _type(tester, find.byType(TextField).last, await _link());
    await _tap(tester, find.text('Add'));
    await _until(tester, find.text('Marta'), seconds: 300);
    await _shot(tester, '21-friends-with-marta');
    // Marta never wrote a bio: her card shows the default one.
    await _tap(
      tester,
      find.byKey(
        ValueKey('friend-row:${me.contacts.friends.single.person.id}'),
      ),
    );
    await _until(tester, find.text(defaultBio));
    await _shot(tester, '21b-marta-card');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 400));
    // Her menu hangs from her "···".
    await _tap(tester, find.byTooltip('Options for Marta'));
    await _until(tester, find.text('Remove from friends…'));
    await _shot(tester, '21c-marta-menu');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 400));

    // A group with Marta.
    await _tap(tester, find.byTooltip('Home'));
    await _tap(tester, find.text('New group'));
    await _type(tester, find.byType(TextField).first, 'The usual crew');
    await _tap(tester, find.text('Marta'));
    await _shot(tester, '22-new-group-with-marta');
    await _tap(tester, find.text('Create group'));
    await _until(tester, find.text('Open room'));

    // The room: Marta comes in, says hello and shares a file.
    await _tap(tester, find.text('Open room'));
    await _until(tester, find.text('Marta'), seconds: 420);
    await _until(tester, find.text('hi from Marta'), seconds: 180);
    await _until(tester, find.text('test-photo.bin'), seconds: 120);
    await _shot(tester, '23-room-live');

    // Writing for a while without sending: Marta's device is told while she is in the
    // room, if it takes typing signals (an older one is sent nothing).
    final box = find.byWidgetPredicate(
      (w) =>
          w is TextField && w.decoration?.hintText == 'Message The usual crew',
    );
    final writing = tester.widget<TextField>(box.first).controller!;
    for (var i = 0; i < 5; i++) {
      writing.text = 'typing $i';
      await _wait(tester, const Duration(seconds: 1));
    }
    writing.clear();
    await _wait(tester, const Duration(milliseconds: 500));

    // How loud Marta plays here: a right click on her opens her voice menu.
    await tester.tap(find.byType(VolumeTarget).last, buttons: kSecondaryButton);
    await _until(tester, find.text('Only you hear these changes'));
    await tester.drag(find.byType(Slider), const Offset(60, 0));
    await _shot(tester, '23b-room-volume');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 300));
    final volume = AppScope.read(tester.element(find.byType(Shell)));
    final marta = volume.roomState.room!.participants
        .firstWhere((p) => !p.me)
        .person;
    expect(volume.prefs.volumeOf(marta.id), isNot(100));

    await _tap(tester, find.byTooltip('Download test-photo.bin'));
    await _until(tester, find.text('Sharing from this computer'), seconds: 300);
    await _tap(tester, find.text('Files'));
    await _shot(tester, '24-room-files');
    await _tap(tester, find.text('Chat'));
    await _until(tester, find.text('heading off now'), seconds: 180);
    await _until(tester, find.text('Only you'), seconds: 180);
    await _shot(tester, '25-room-marta-left');
    await _tap(tester, find.byTooltip('Home'));
    await _shot(tester, '26-home-after');

    // Sounds: entering, Marta coming in, her two messages, her leaving; nothing for the
    // room's past or for the file.
    final state = AppScope.read(tester.element(find.byType(Shell)));
    expect(state.played, [
      UiSound.join,
      UiSound.join,
      UiSound.message,
      UiSound.message,
      UiSound.leave,
    ]);
  });
}
