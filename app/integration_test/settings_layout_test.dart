// Pictures of the settings, every part, dark and light, at window sizes from the smallest
// Fogata allows up to a large screen, and of the settings narrower than both their column
// and a part fit in; and of the cards with the longest bio. Nothing connects; no
// microphone, camera or screen is used.
//
//   FOGATA_SHOTS=<folder> flutter test integration_test/settings_layout_test.dart -d linux

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:integration_test/integration_test.dart';

import '../test/fake_room.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _shot(WidgetTester tester, String name) async {
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(
    Offset.zero & (view.size * tester.view.devicePixelRatio),
  );
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

/// A bio as long as one may be: 190 characters on 4 lines.
const _longBio =
    'Leo, from the Friday games group. Mostly online in the evenings.\n'
    'I play strategy games and the odd shooter, badly.\n'
    'Ask me for a room any time.\n'
    'Usually in voice with the mic on, say hi to me.';

AppState _state() {
  final state = fakeRoom(
    people: 2,
    dataDir: '/nonexistent/fogata-settings-test',
  );
  // Over the home page, not a call.
  state.callState.call = null;
  state.nav.page = Screen.home;
  state.contacts.bio = 'Marta. Here most evenings.';
  state.contacts.friends = [
    UiFriend(
      person: other(0),
      fingerprint: 'AB0C D0EF 9PLD 3VHC',
      presence: UiPresence.online,
      delivered: true,
      bio: _longBio,
    ),
  ];
  return state;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('settings layouts', (tester) async {
    final ratio = tester.view.devicePixelRatio;
    final problems = <String>[];
    final themes = [('dark', Fg.darkTokens), ('light', Fg.lightTokens)];
    for (final (theme, tokens) in themes) {
      for (final size in const [
        Size(960, 640),
        Size(1280, 800),
        Size(1920, 1080),
      ]) {
        final state = _state();
        tester.view.physicalSize = size * ratio;
        await tester.pumpWidget(
          AppScope(
            state: state,
            child: MaterialApp(
              theme: tokens.themeData,
              home: const Material(child: Shell()),
            ),
          ),
        );
        state.openSettings();
        await _wait(tester, const Duration(milliseconds: 600));
        final at = '${size.width.round()}x${size.height.round()}';
        // Every part at the smallest size and one at the others.
        final sections = size.width == 960
            ? SettingsSection.values
            : const [SettingsSection.profile, SettingsSection.voice];
        for (final section in sections) {
          state.showSettingsSection(section);
          await _wait(tester, const Duration(milliseconds: 500));
          await _shot(tester, 'settings-$theme-$at-${section.name}');
        }
        // The cards: a friend's with the longest bio, and this device's own.
        state.closeSettings();
        state.go(Screen.friends);
        state
          ..nav.friendCard = 'p0'
          ..nav.friendCardFrom = const Rect.fromLTWH(340, 230, 400, 60);
        state.notifyListeners();
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'card-$theme-$at-friend');
        state.closeFriendCard();
        state.openUserPanel(Rect.fromLTWH(80, size.height - 56, 220, 48));
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'card-$theme-$at-own');
        state.closeUserPanel();
        final e = tester.takeException();
        if (e != null && !'$e'.contains('called during build')) {
          problems.add('$theme $at: $e');
        }
        await tester.pumpWidget(const SizedBox());
      }
      // Narrower than the column and a part: the list, then a part with the way back.
      final state = _state()..nav.settingsOpen = true;
      tester.view.physicalSize = const Size(960, 640) * ratio;
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: tokens.themeData,
            home: Builder(
              builder: (context) => Material(
                color: context.fg.bg2,
                child: Center(
                  child: Container(
                    width: 520,
                    height: 600,
                    clipBehavior: Clip.antiAlias,
                    decoration: BoxDecoration(
                      color: context.fg.bg,
                      borderRadius: BorderRadius.circular(22),
                      border: Border.all(color: context.fg.border),
                    ),
                    child: const Settings(),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await _wait(tester, const Duration(milliseconds: 600));
      await _shot(tester, 'settings-$theme-narrow-list');
      state.showSettingsSection(SettingsSection.profile);
      await _wait(tester, const Duration(milliseconds: 500));
      await _shot(tester, 'settings-$theme-narrow-profile');
      final e = tester.takeException();
      if (e != null && !'$e'.contains('called during build')) {
        problems.add('$theme narrow: $e');
      }
      await tester.pumpWidget(const SizedBox());
    }
    expect(problems, isEmpty);
  });
}
