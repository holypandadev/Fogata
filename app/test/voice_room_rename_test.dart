// Renaming a voice room in the group's settings: one rename per change, judged against the
// room as it is now, and a name changed elsewhere shows in the field.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// Keeps the renames asked of the core.
class _Renames extends AppState {
  _Renames() : super('/nonexistent/fogata-voice-rename-test');

  final asked = <String>[];

  @override
  Future<bool> setVoiceRoom(String group, String? voice, String name) async {
    asked.add('$voice $name');
    return true;
  }
}

UiGroup _group(String name) => UiGroup(
  id: 'g',
  name: 'Friday games',
  localName: '',
  icon: 0,
  members: 3,
  online: 1,
  manager: true,
  rank: 0,
  outranks: const [],
  maxMembers: 20,
  maxInRoom: 8,
  voiceRooms: [
    const UiVoiceRoom(id: AppState.firstRoom, name: ''),
    UiVoiceRoom(id: 'v2', name: name),
  ],
  openInvites: 0,
  needsUpdate: false,
  raiseAnyway: false,
);

void main() {
  testWidgets('a voice room is renamed once, and a name from elsewhere shows', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = _Renames()
      ..contacts.profile = const UiProfile(
        person: me,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      )
      ..contacts.groups = [_group('Room 2')]
      ..nav.page = Screen.home
      ..nav.selectedGroup = 'g';
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Shell()),
        ),
      ),
    );
    Future<void> settle() async {
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
    }

    state.openGroupSettings('g', tab: 1);
    await settle();
    Finder field() => find.byWidgetPredicate(
      (w) =>
          w is EditableText &&
          w.controller.text.isNotEmpty &&
          w.controller.text != 'General',
    );
    expect(field(), findsOneWidget);
    // Typed and left: one rename.
    await tester.enterText(field(), 'Music');
    FocusManager.instance.primaryFocus?.unfocus();
    await settle();
    expect(state.asked, ['v2 Music']);
    // The core renamed it: going into the field and out again asks for nothing more.
    state.contacts.groups = [_group('Music')];
    state.go(state.nav.page);
    await settle();
    await tester.tap(field());
    await settle();
    FocusManager.instance.primaryFocus?.unfocus();
    await settle();
    expect(state.asked, ['v2 Music']);
    // Another manager renames it: the field shows the new name.
    state.contacts.groups = [_group('Chill')];
    state.go(state.nav.page);
    await settle();
    expect(tester.widget<EditableText>(field()).controller.text, 'Chill');
    FocusManager.instance.primaryFocus?.unfocus();
    await settle();
    expect(state.asked, ['v2 Music']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
