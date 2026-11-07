import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/widgets.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  // The settings ask the core for what a test does not have (devices, storage).
  tester.takeException();
}

Future<void> _show(WidgetTester tester, AppState state, Size size) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await _settle(tester);
}

/// The settings alone in a box of `size`, smaller than the window allows.
Future<void> _showAlone(WidgetTester tester, AppState state, Size size) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Center(
            child: SizedBox.fromSize(size: size, child: const Settings()),
          ),
        ),
      ),
    ),
  );
  await _settle(tester);
}

Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

Finder _nav(SettingsSection section) =>
    find.byKey(ValueKey('settings-nav:${section.name}'));

Finder _page(SettingsSection section) =>
    find.byKey(ValueKey('settings-page:${section.name}'));

void main() {
  setUpAll(loadFonts);

  setUp(() {
    // The window's buttons ask the window manager, which tests do not have.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  for (final size in const [
    Size(960, 640),
    Size(1280, 800),
    Size(1920, 1080),
  ]) {
    testWidgets('the column and one part side by side at $size', (
      tester,
    ) async {
      final state = fakeRoom(people: 2);
      await _show(tester, state, size);
      state.openSettings();
      await _settle(tester);
      expect(find.text('USER'), findsOneWidget);
      expect(find.text('APP'), findsOneWidget);
      for (final section in SettingsSection.values) {
        // The tray's part only where there is a tray (none in tests).
        if (section == SettingsSection.behaviour) continue;
        expect(_nav(section), findsOneWidget, reason: section.name);
      }
      // My profile first.
      expect(_page(SettingsSection.profile), findsOneWidget);
      expect(find.text('Your identity'), findsOneWidget);
      expect(find.byTooltip('All settings'), findsNothing);

      final parts = {
        SettingsSection.appearance: 'Theme',
        SettingsSection.voice: 'Microphone and speaker',
        SettingsSection.sounds: 'Sound effects',
        SettingsSection.network: 'Check the network',
        SettingsSection.storage: 'Downloads',
      };
      for (final MapEntry(key: section, value: shows) in parts.entries) {
        await tester.tap(_nav(section));
        await _settle(tester);
        expect(_page(section), findsOneWidget, reason: section.name);
        expect(find.text(shows), findsWidgets, reason: section.name);
        expect(find.text('Your identity'), findsNothing);
        // The column stays.
        expect(_nav(SettingsSection.profile), findsOneWidget);
      }
      // The panel fits the window, whatever its size.
      final panel = tester.getRect(find.byType(Settings));
      expect(panel.width, lessThanOrEqualTo(1000));
      expect(panel.height, lessThanOrEqualTo(760));
      expect(panel.bottom, lessThanOrEqualTo(size.height));

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await _settle(tester);
      expect(state.nav.settingsOpen, isFalse);
      expect(find.byType(Settings), findsNothing);
      await _done(tester);
    });
  }

  testWidgets('too narrow for both, the list and a part take turns', (
    tester,
  ) async {
    final state = fakeRoom(people: 2)..nav.settingsOpen = true;
    await _showAlone(tester, state, const Size(520, 640));
    // The list first, with the way out.
    expect(_nav(SettingsSection.profile), findsOneWidget);
    expect(_page(SettingsSection.profile), findsNothing);
    expect(find.byTooltip('Close settings'), findsOneWidget);
    await tester.tap(_nav(SettingsSection.sounds));
    await _settle(tester);
    expect(_page(SettingsSection.sounds), findsOneWidget);
    expect(_nav(SettingsSection.profile), findsNothing);
    expect(find.text('Sound effects'), findsOneWidget);
    // Back to the list.
    await tester.tap(find.byTooltip('All settings'));
    await _settle(tester);
    expect(state.nav.settingsSection, isNull);
    expect(_nav(SettingsSection.sounds), findsOneWidget);
    expect(_page(SettingsSection.sounds), findsNothing);
    await _done(tester);
  });

  testWidgets('Edit profile opens My profile', (tester) async {
    final state = fakeRoom(people: 2);
    await _show(tester, state, const Size(1280, 800));
    // A part chosen before is not where Edit profile goes.
    state.openSettings(SettingsSection.network);
    await _settle(tester);
    state.closeSettings();
    await _settle(tester);
    state.openUserPanel(const Rect.fromLTWH(80, 700, 200, 50));
    await _settle(tester);
    await tester.tap(find.text('Edit profile'));
    await _settle(tester);
    expect(state.nav.settingsOpen, isTrue);
    expect(state.nav.settingsSection, SettingsSection.profile);
    expect(_page(SettingsSection.profile), findsOneWidget);
    expect(find.text('Your identity'), findsOneWidget);
    await _done(tester);
  });

  test('the storage and network notices open their parts', () {
    final state = fakeRoom(people: 2);
    state.openSettings();
    expect(state.nav.settingsSection, isNull);
    state.closeSettings();
    state.nav.networkCheckOnOpen = true;
    state.openSettings();
    expect(state.nav.settingsSection, SettingsSection.network);
    state.closeSettings();
    state.nav.networkCheckOnOpen = false;
    state.direct.storageOnOpen = true;
    state.openSettings();
    expect(state.nav.settingsSection, SettingsSection.storage);
  });

  testWidgets('clicking into the name changes nothing to save', (tester) async {
    final state = fakeRoom(people: 2);
    await _show(tester, state, const Size(1280, 800));
    state.openSettings(SettingsSection.profile);
    await _settle(tester);
    FgButton save() =>
        tester.widget<FgButton>(find.widgetWithText(FgButton, 'Save'));
    final name = find.descendant(
      of: find.byWidgetPredicate((w) => w is FgField && w.label == 'Name'),
      matching: find.byType(EditableText),
    );
    await tester.tap(name);
    await _settle(tester);
    final field = tester.widget<EditableText>(name).controller;
    field.selection = const TextSelection.collapsed(offset: 1);
    await _settle(tester);
    expect(save().onTap, isNull);
    expect(state.nav.settingsMayClose, isNull);
    // The same name written again is no change either.
    final written = field.text;
    await tester.enterText(name, '$written ');
    await _settle(tester);
    expect(save().onTap, isNull);
    await tester.enterText(name, 'Ana Bea');
    await _settle(tester);
    expect(save().onTap, isNotNull);
    await _done(tester);
  });

  testWidgets('closing with changes not saved asks first', (tester) async {
    final state = fakeRoom(people: 2);
    await _show(tester, state, const Size(1280, 800));
    state.openSettings(SettingsSection.profile);
    await _settle(tester);
    final bio = find.byKey(const ValueKey('bio-field'));
    await tester.enterText(bio, 'Here on weekends.');
    await _settle(tester);

    // Esc: the question; Keep editing leaves everything as it was.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('Save your changes?'), findsOneWidget);
    await tester.tap(find.text('Keep editing'));
    await _settle(tester);
    expect(state.nav.settingsOpen, isTrue);
    expect(tester.widget<TextField>(bio).controller!.text, 'Here on weekends.');

    // A click beside the settings asks too; Discard closes them.
    await tester.tapAt(const Offset(8, 8));
    await _settle(tester);
    expect(find.text('Save your changes?'), findsOneWidget);
    await tester.tap(find.text('Discard'));
    await _settle(tester);
    expect(state.nav.settingsOpen, isFalse);

    // Opened again, nothing is left of the change, and closing does not ask.
    state.openSettings(SettingsSection.profile);
    await _settle(tester);
    expect(
      tester.widget<TextField>(bio).controller!.text,
      isNot(contains('weekends')),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('Save your changes?'), findsNothing);
    expect(state.nav.settingsOpen, isFalse);

    // Changes that cannot be saved as they are only offer to discard them.
    state.openSettings(SettingsSection.profile);
    await _settle(tester);
    await tester.enterText(bio, 'Ana\u202Egnp.exe');
    await _settle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('Discard your changes?'), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-unsaved')),
        matching: find.text('Save'),
      ),
      findsNothing,
    );
    await tester.tap(find.text('Discard'));
    await _settle(tester);
    expect(state.nav.settingsOpen, isFalse);
    await _done(tester);
  });
}
