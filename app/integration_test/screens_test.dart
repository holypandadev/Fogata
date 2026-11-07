// Walks through the screens with a fresh data folder and saves a picture of each one, in
// the language FOGATA_TEST_LANGUAGE names (one of lib/src/l10n.dart's `languages`;
// English without it). FOGATA_TEST_GROUP names the group it makes.
//
//   FOGATA_DATA_DIR=<empty folder> FOGATA_SHOTS=<folder> [FOGATA_TEST_LANGUAGE=de] \
//     [FOGATA_TEST_WINDOW=960x640] [FOGATA_TEST_GROUP="The usual crew"] \
//     flutter test integration_test/screens_test.dart -d linux

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/photo.dart';
import 'package:fogata/src/screens/new_group.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/user_panel.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/widgets.dart';
import 'package:integration_test/integration_test.dart';
import 'package:window_manager/window_manager.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];

/// The longest part of a message said with a mark where a part drawn its own way goes
/// (a countdown, a size): what a finder looks for in any language.
String _part(String Function(String at) message) {
  final parts = message('\u0000').split('\u0000')
    ..sort((a, b) => b.trim().length.compareTo(a.trim().length));
  return parts.first.trim();
}

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _until(
  WidgetTester tester,
  Finder finder, {
  int seconds = 20,
}) async {
  final end = DateTime.now().add(Duration(seconds: seconds));
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(end)) {
      await _shot(tester, 'failed');
      fail('not shown in time: $finder');
    }
    await _wait(tester, const Duration(milliseconds: 100));
  }
}

/// Saves a picture of the window; `settle` lets animations finish first.
Future<void> _shot(
  WidgetTester tester,
  String name, {
  Duration settle = const Duration(milliseconds: 400),
}) async {
  await _wait(tester, settle);
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final size = view.size;
  final image = await layer.toImage(Offset.zero & size);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

Future<void> _type(WidgetTester tester, Finder field, String text) async {
  await _until(tester, field);
  tester.widget<TextField>(field.first).controller!.text = text;
  await _wait(tester, const Duration(milliseconds: 200));
}

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _until(tester, finder);
  await tester.ensureVisible(finder.first);
  await _wait(tester, const Duration(milliseconds: 200));
  await tester.tap(finder.first);
  await _wait(tester, const Duration(milliseconds: 300));
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('every screen', (tester) async {
    // The language, as if chosen in Settings before.
    final language = Platform.environment['FOGATA_TEST_LANGUAGE'];
    final data = Platform.environment['FOGATA_DATA_DIR'];
    if (language != null && data != null) {
      Directory(data).createSync(recursive: true);
      File('$data/app.json').writeAsStringSync('{"uiLanguage": "$language"}');
    }
    await app.main();
    if (language != null) expect(currentLanguage, language);
    // FOGATA_TEST_WINDOW=960x640: the whole walk in a window of that size.
    final window = Platform.environment['FOGATA_TEST_WINDOW']?.split('x');
    if (window != null) {
      await windowManager.setSize(
        Size(double.parse(window[0]), double.parse(window[1])),
      );
    }
    await _until(tester, find.text(l10n.welcomeCardTitle));
    await continueUnencryptedIfAsked(tester);
    await _shot(tester, '01-welcome');
    // A new key rolls in character by character: a picture in the middle of it.
    await tester.tap(find.byTooltip(l10n.welcomeNewKey));
    await _shot(
      tester,
      '01d-welcome-new-key',
      settle: const Duration(milliseconds: 150),
    );
    await _tap(tester, find.byTooltip('Duckly'));
    await _type(tester, find.byType(TextField), 'Leo');
    await _shot(tester, '01b-welcome-named');
    final welcome = AppScope.read(tester.element(find.byType(Welcome)));
    await welcome.setTheme(ThemeMode.light);
    await _shot(tester, '01c-welcome-light');
    await welcome.setTheme(ThemeMode.dark);
    // The colours fade between themes; let them finish.
    await _wait(tester, const Duration(milliseconds: 600));

    // The button turns green with "Ready" for a moment before the app opens.
    await tester.tap(find.text(l10n.welcomeCreate));
    await _shot(
      tester,
      '01e-welcome-ready',
      settle: const Duration(milliseconds: 250),
    );
    await _until(tester, find.text(l10n.homeSubtitle));
    // Rooms start their call with the microphone off: the test uses no real device.
    AppScope.read(tester.element(find.byType(Shell))).prefs.joinWithMic = false;
    await _shot(tester, '02-home-empty');

    await _tap(tester, find.text(l10n.homeNewGroup));
    await _until(tester, find.text(l10n.newGroupSubtitle));
    // A name of only blank characters (Hangul fillers here) says why it is not taken.
    await _type(tester, find.byType(TextField).first, '\u3164\u3164\u2800');
    expect(find.text(l10n.namesBlank), findsOneWidget);
    await _shot(tester, '03a-new-group-blank-name');
    await _type(
      tester,
      find.byType(TextField).first,
      Platform.environment['FOGATA_TEST_GROUP'] ?? 'The usual crew',
    );
    await _tap(tester, find.byTooltip(l10n.iconsGroupStar));
    await _shot(tester, '03-new-group');
    // A picture from the computer instead: the file dialog is the system's, so the
    // picture is made here as one picked would be.
    final chosen = await rootBundle.load('assets/images/welcome.png');
    final groupPicture = await squarePicture(
      chosen.buffer.asUint8List(),
      maxBytes: maxGroupPictureBytes,
    );
    pickGroupPicture = () async => groupPicture;
    await _tap(tester, find.byTooltip(l10n.newGroupChooseFromComputer));
    await _shot(tester, '03b-new-group-picture');
    await _tap(tester, find.text(l10n.newGroupCreate));
    await _until(tester, find.text(l10n.homeNoRoom));
    // Set right after the group is made.
    final made = AppScope.read(tester.element(find.byType(Shell)));
    final end = DateTime.now().add(const Duration(seconds: 20));
    while (made.contacts.groups.isEmpty ||
        made.contacts.groups.first.picture == null) {
      if (DateTime.now().isAfter(end)) fail('the new group has no picture');
      await _wait(tester, const Duration(milliseconds: 100));
    }
    await _shot(tester, '04-home-group');
    // Changed twice: the newest shows, from the record that replaced the one before.
    final logo = await rootBundle.load('assets/images/logo.png');
    for (final picture in [
      await squarePicture(
        logo.buffer.asUint8List(),
        maxBytes: maxGroupPictureBytes,
      ),
      await _circles(),
    ]) {
      expect(
        await made.setGroupPicture(made.contacts.groups.first.id, picture),
        true,
      );
      expect(made.contacts.groups.first.picture, picture);
    }
    await _shot(tester, '04b-home-group-picture-changed');
    // The bell's panel hangs from the bell.
    await _tap(tester, find.byTooltip(l10n.bellNotifications));
    await _until(tester, find.byKey(const ValueKey('bell-panel')));
    await _shot(tester, '04c-bell');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 400));

    await _tap(
      tester,
      find.widgetWithText(Pressable, l10n.commonFriends).first,
    );
    await _until(
      tester,
      find.textContaining(_part(l10n.friendsHintCode)),
      seconds: 40,
    );
    await _shot(tester, '05-friends-code');
    // The code shown beside the list, then with one typed in, in the smallest window
    // Fogata allows.
    await _tap(tester, find.byTooltip(l10n.widgetsShowWhat('code')));
    await _shot(tester, '05b-friends-code-shown');
    tester.view.physicalSize =
        const Size(960, 640) * tester.view.devicePixelRatio;
    // The page lays itself out again in one column, with the panel made anew.
    await _wait(tester, const Duration(milliseconds: 500));
    await _tap(tester, find.byTooltip(l10n.widgetsShowWhat('code')));
    final input = find.descendant(
      of: find.byWidgetPredicate(
        (w) => w is FgField && w.label == l10n.friendsLinkOrCode,
      ),
      matching: find.byType(TextField),
    );
    await _type(tester, input, 'FGT-7Q4MK-2XR9D');
    await Scrollable.ensureVisible(tester.element(input), alignment: 1);
    await _shot(tester, '05a-friends-code-960x640');
    await _type(tester, input, '');
    tester.view.resetPhysicalSize();
    // It hides itself again after a while.
    await _until(tester, find.byTooltip(l10n.widgetsShowWhat('code')));
    await _tap(tester, find.text(l10n.friendsTabLink));
    await _until(tester, find.textContaining(_part(l10n.friendsHintLink)));
    await _shot(tester, '06-friends-link');
    await _tap(tester, find.text(l10n.friendsTabQr));
    await _until(tester, find.textContaining(_part(l10n.friendsHintQr)));
    await _shot(tester, '07-friends-qr');

    // The person's panel, from their card: appearing offline, and a friend link copied.
    await _tap(
      tester,
      find.byWidgetPredicate(
        (w) => w is Pressable && w.label == l10n.shellYourProfile,
      ),
    );
    await _until(tester, find.text(l10n.userPanelCopyLink));
    await _shot(tester, '07b-user-panel');
    Finder inPanel(String text) => find.descendant(
      of: find.byType(UserPanelLayer),
      matching: find.text(text),
    );
    await _tap(tester, inPanel(l10n.userPanelOffline));
    await _until(tester, find.text(l10n.userPanelOfflineNote));
    expect(
      AppScope.read(tester.element(find.byType(Shell))).prefs.appearOffline,
      true,
    );
    await _tap(tester, inPanel(l10n.userPanelCopyLink));
    await _until(tester, inPanel(l10n.commonCopied));
    await _shot(tester, '07c-user-panel-offline-copied');
    await _tap(tester, inPanel(l10n.userPanelOnline));
    expect(
      AppScope.read(tester.element(find.byType(Shell))).prefs.appearOffline,
      false,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 400));
    expect(find.text(l10n.userPanelCopyLink), findsNothing);

    await _tap(tester, find.byTooltip(l10n.commonSettings).first);
    await _until(tester, find.text(l10n.settingsBannerTitle));
    await _shot(tester, '08a-settings-identity');
    await _tap(tester, find.byKey(const ValueKey('settings-nav:network')));
    await _tap(tester, find.text(l10n.settingsCheckNetwork));
    await _until(tester, find.text(l10n.settingsCheckAgain), seconds: 40);
    await _shot(tester, '08-settings');
    await _tap(tester, find.byKey(const ValueKey('settings-nav:donate')));
    await _shot(tester, '08b-settings-donate');
    await _tap(tester, find.byKey(const ValueKey('settings-nav:report')));
    await _type(
      tester,
      find.byKey(const ValueKey('report-what')),
      'The microphone stopped when Leo shared his screen.',
    );
    await _tap(tester, find.byKey(const ValueKey('report-see-sent')));
    await _shot(tester, '08c-settings-report');
    // "Email it" only where what would open is written down instead.
    final opened = Platform.environment['FOGATA_TEST_OPENED'];
    if (opened != null) {
      await _tap(tester, find.byKey(const ValueKey('report-email')));
      await _until(tester, find.byKey(const ValueKey('report-ready')));
      await _shot(tester, '08d-settings-report-ready');
      final mail = File(opened).readAsStringSync();
      expect(mail, contains('mailto:'));
      // What was written goes in the report's file only, never on a command line.
      expect(mail, isNot(contains('microphone')));
    }
    await _tap(tester, find.byTooltip(l10n.settingsCloseTooltip));

    await _tap(tester, find.byTooltip(l10n.commonHome));
    await _tap(tester, find.text(l10n.homeOpenRoom));
    await _until(tester, find.text(l10n.roomPillOnlyYou));
    // A small window starts the call with the chat hidden: its button shows it.
    final showChat = find.byTooltip(l10n.roomShowChat);
    await _until(
      tester,
      find.byWidgetPredicate(
        (w) =>
            w is TextField || (w is Tooltip && w.message == l10n.roomShowChat),
      ),
    );
    if (showChat.evaluate().isNotEmpty) await _tap(tester, showChat);
    await _type(tester, find.byType(TextField).last, 'Hi, anyone around?');
    await _tap(tester, find.byTooltip(l10n.commonSend));
    await _until(tester, find.text('Hi, anyone around?'));
    await _shot(tester, '09-room-alone');
    // Each device's own panel, from the room's controls and from the person's pill (whose
    // arrows come first).
    for (final (tip, at, name) in [
      (l10n.devicesMicOptions, 1, '09g-room-mic-panel'),
      (l10n.devicesSpeakerOptions, 1, '09h-room-speaker-panel'),
      (l10n.devicesCameraOptions, 0, '09i-room-camera-panel'),
      (l10n.devicesMicOptions, 0, '09j-pill-mic-panel'),
      (l10n.devicesSpeakerOptions, 0, '09k-pill-speaker-panel'),
    ]) {
      await _tap(tester, find.byTooltip(tip).at(at));
      await _until(tester, find.text(l10n.devicesVoiceSettings));
      await _shot(tester, name);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      // Gone once it has closed, which takes longer on a busy computer.
      final end = DateTime.now().add(const Duration(seconds: 5));
      do {
        await _wait(tester, const Duration(milliseconds: 100));
      } while (find.text(l10n.devicesVoiceSettings).evaluate().isNotEmpty &&
          DateTime.now().isBefore(end));
      expect(find.text(l10n.devicesVoiceSettings), findsNothing);
    }

    // Files dropped on the room wait above the message box until Send.
    final state = AppScope.read(tester.element(find.byType(Shell)));
    final dropped = Directory('${state.dataDir}/dropped')
      ..createSync(recursive: true);
    File('${dropped.path}/sunday-plan.pdf').writeAsStringSync('plan\n' * 64);
    File('${dropped.path}/notes.txt').writeAsStringSync('notes\n');
    await state.addOutgoing([
      '${dropped.path}/sunday-plan.pdf',
      '${dropped.path}/notes.txt',
      dropped.path,
    ]);
    await _until(
      tester,
      find.textContaining(
        _part((at) => l10n.outgoingToShare(2, '2', at)),
        findRichText: true,
      ),
    );
    expect(find.text(l10n.roomAddMessageOptional), findsOneWidget);
    await _shot(tester, '09b-room-files-waiting');
    // The note about the folder covers the Send button for a moment.
    while (find.text(l10n.appFoldersNotShared(1)).evaluate().isNotEmpty) {
      await _wait(tester, const Duration(milliseconds: 200));
    }
    await _tap(tester, find.byTooltip(l10n.outgoingRemoveFile('notes.txt')));
    await _type(tester, find.byType(TextField).last, 'Here is the plan');
    await _tap(tester, find.byTooltip(l10n.commonSend));
    await _until(tester, find.text(l10n.roomSharingFromHere));
    expect(find.text('Here is the plan'), findsOneWidget);
    expect(state.roomState.outgoingHere, isEmpty);
    expect(state.roomState.room!.files.map((f) => f.name), ['sunday-plan.pdf']);
    await _shot(tester, '09c-room-files-sent');
    // A picture shows in the chat, and large with a click.
    final picture = await rootBundle.load('assets/images/welcome.png');
    File('${dropped.path}/fogata.png')
        .writeAsBytesSync(picture.buffer.asUint8List());
    await state.addOutgoing(['${dropped.path}/fogata.png']);
    await _until(tester, find.byTooltip(l10n.commonSend));
    await _tap(tester, find.byTooltip(l10n.commonSend));
    await _until(
      tester,
      find.byWidgetPredicate(
        (w) => w is Pressable && w.label == l10n.roomOpenFile('fogata.png'),
      ),
    );
    await _shot(tester, '09e-room-picture', settle: const Duration(seconds: 1));
    await _tap(
      tester,
      find.byWidgetPredicate(
        (w) => w is Pressable && w.label == l10n.roomOpenFile('fogata.png'),
      ),
    );
    await _until(tester, find.text(l10n.roomSaveFile));
    await _shot(tester, '09f-picture-view');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 500));
    // The smallest window the app allows: the room's controls get smaller to fit.
    final size = await windowManager.getSize();
    await windowManager.setSize(const Size(960, 640));
    await _shot(
      tester,
      '09d-room-small',
      settle: const Duration(milliseconds: 1200),
    );
    await windowManager.setSize(size);
    await _wait(tester, const Duration(milliseconds: 800));

    await _tap(tester, find.text(l10n.roomTabFiles));
    await _shot(tester, '10-room-files');

    // A small window's controls are icons, named in their tooltips.
    await _tap(
      tester,
      find.text(l10n.roomLeave).evaluate().isEmpty
          ? find.byTooltip(l10n.roomLeave)
          : find.text(l10n.roomLeave),
    );
    await _until(tester, find.text(l10n.roomLeaveAndClose));
    await _shot(tester, '11-room-leave');
    await _tap(tester, find.text(l10n.roomLeaveAndClose));
    await _until(tester, find.text(l10n.homeNoRoom));

    await _tap(tester, find.byTooltip(l10n.commonSettings).first);
    await _tap(tester, find.byKey(const ValueKey('settings-nav:appearance')));
    await _tap(tester, find.text(l10n.themeLight));
    await _tap(tester, find.byTooltip(l10n.settingsCloseTooltip));
    await _tap(tester, find.byTooltip(l10n.commonHome));
    await _shot(tester, '12-home-light');
    await _tap(
      tester,
      find.widgetWithText(Pressable, l10n.commonFriends).first,
    );
    await _until(
      tester,
      find.textContaining(_part(l10n.friendsHintCode)),
      seconds: 40,
    );
    await _shot(tester, '13-friends-light');
    await _tap(tester, find.byTooltip(l10n.commonSettings).first);
    await _tap(tester, find.byKey(const ValueKey('settings-nav:appearance')));
    await _tap(tester, find.text(l10n.themeDark));
  });
}

/// A small picture: two circles on a purple square.
Future<Uint8List> _circles() async {
  final recorder = ui.PictureRecorder();
  final canvas = Canvas(recorder);
  canvas.drawRect(
    const Rect.fromLTWH(0, 0, 64, 64),
    Paint()..color = const Color(0xFF5A1E9E),
  );
  canvas.drawCircle(
    const Offset(20, 20),
    18,
    Paint()..color = const Color(0xFFE0245E),
  );
  canvas.drawCircle(
    const Offset(48, 46),
    16,
    Paint()..color = const Color(0xFF2563EB),
  );
  final image = await recorder.endRecording().toImage(64, 64);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  return data!.buffer.asUint8List();
}
