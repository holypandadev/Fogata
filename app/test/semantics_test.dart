// What screen readers are told: buttons say what they do even when only a tooltip does,
// chosen options say they are chosen, and a text field is one field with its name.

import 'dart:ui' show Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/theme_picker.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

Widget _app(Widget child, {AppState? state}) {
  final app = MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Material(child: Center(child: child)),
  );
  return state == null ? app : AppScope(state: state, child: app);
}

/// The text field [field] is in: its own node holds the name, and no other text field
/// is wrapped around it.
void _oneField(WidgetTester tester, Finder field, String name) {
  final node = tester.getSemantics(
    find.descendant(of: field, matching: find.byType(EditableText)),
  );
  final data = node.getSemanticsData();
  expect(data.flagsCollection.isTextField, isTrue);
  expect(data.label, name);
  for (var up = node.parent; up != null; up = up.parent) {
    expect(
      up.getSemanticsData().flagsCollection.isTextField,
      isFalse,
      reason: 'a text field around $name',
    );
  }
}

void main() {
  testWidgets('a button with only a tooltip is named by it, once', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await tester.pumpWidget(
      _app(
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Pressable(
              key: const ValueKey('icon'),
              onTap: () {},
              tooltip: 'Add files',
              builder: (_, _) => const SizedBox.square(dimension: 30),
            ),
            Pressable(
              key: const ValueKey('both'),
              onTap: () {},
              label: 'Send',
              tooltip: 'Send the message',
              builder: (_, _) => const SizedBox.square(dimension: 30),
            ),
          ],
        ),
      ),
    );
    final icon = tester
        .getSemantics(find.bySemanticsLabel('Add files'))
        .getSemanticsData();
    expect(icon.label, 'Add files');
    expect(icon.tooltip, isEmpty);
    expect(icon.flagsCollection.isButton, isTrue);
    final both = tester
        .getSemantics(find.bySemanticsLabel('Send'))
        .getSemanticsData();
    expect(both.label, 'Send');
    expect(both.tooltip, 'Send the message');
    handle.dispose();
  });

  testWidgets('the theme chosen says so; the others do not', (tester) async {
    final handle = tester.ensureSemantics();
    final state = AppState('/nonexistent/fogata-semantics-test');
    await tester.pumpWidget(_app(const ThemePicker(), state: state));
    for (final look in Look.values) {
      final data = tester
          .getSemantics(
            find.bySemanticsLabel(
              RegExp(
                '^${look == Look.system ? 'System theme' : '${look.label} theme'}',
              ),
            ),
          )
          .getSemanticsData();
      expect(
        data.flagsCollection.isSelected,
        look == state.prefs.look ? Tristate.isTrue : Tristate.isFalse,
        reason: look.name,
      );
    }
    handle.dispose();
  });

  testWidgets('a field is one text field with its name', (tester) async {
    final handle = tester.ensureSemantics();
    await tester.pumpWidget(
      _app(
        SizedBox(
          width: 300,
          child: FgField(
            key: const ValueKey('field'),
            controller: TextEditingController(text: 'Hello'),
            label: 'Message Sofía',
          ),
        ),
      ),
    );
    _oneField(tester, find.byKey(const ValueKey('field')), 'Message Sofía');
    handle.dispose();
  });

  testWidgets('settings: the bio is one field, and chosen options say so', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await loadFonts();
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = fakeRoom(people: 2)
      ..callState.call = null
      ..roomState.room = null
      ..nav.page = Screen.home;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Shell()),
        ),
      ),
    );
    state.openSettings();
    state.showSettingsSection(SettingsSection.profile);
    for (var i = 0; i < 4; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    tester.takeException();
    _oneField(tester, find.byKey(const ValueKey('bio-field')), 'Bio');
    // The section shown is the chosen one in the list of sections.
    bool selected(Finder f) =>
        tester.getSemantics(f).getSemanticsData().flagsCollection.isSelected ==
        Tristate.isTrue;
    Finder nav(SettingsSection section) => find
        .descendant(
          of: find.byKey(ValueKey('settings-nav:${section.name}')),
          matching: find.byType(RichText),
        )
        .first;
    expect(selected(nav(SettingsSection.profile)), isTrue);
    expect(selected(nav(SettingsSection.appearance)), isFalse);
    // Of the two ways times show, the one in use.
    state.showSettingsSection(SettingsSection.languageTime);
    for (var i = 0; i < 4; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    final hours = find.bySemanticsLabel(RegExp('24 hours'));
    final twelve = find.bySemanticsLabel(RegExp('12 hours'));
    expect(selected(hours), isTrue);
    expect(selected(twelve), isFalse);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
    handle.dispose();
  });
}
