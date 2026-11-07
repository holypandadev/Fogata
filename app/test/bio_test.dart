import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/names.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

UiFriend _friend(UiPerson person, {String? bio}) => UiFriend(
  person: person,
  fingerprint: '${person.short} 9PLD 3VHC',
  presence: UiPresence.online,
  delivered: true,
  bio: bio,
);

Future<void> _show(WidgetTester tester, AppState state) async {
  tester.view.physicalSize = const Size(1440, 900);
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

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  // The settings ask the core for what a test does not have.
  tester.takeException();
}

Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

String? _cardBio(WidgetTester tester) {
  final shown = find.byKey(const ValueKey('card-bio'));
  if (shown.evaluate().isEmpty) return null;
  return tester.widget<Text>(shown).data;
}

void main() {
  group('bio rules, as the core has them', () {
    test('each line is tidied like a name', () {
      expect(tidyBio(''), '');
      expect(tidyBio('  \n \u3164 \n'), '');
      expect(
        tidyBio('  Hey there!  I’m on Fogata. '),
        'Hey there! I’m on Fogata.',
      );
      expect(tidyBio('\n\nAna\n\n\n\nBea\n\n'), 'Ana\n\nBea');
      expect(tidyBio('Ana\u2800\nBea\u3000\u3164'), 'Ana\nBea');
      expect(tidyBio('Ana\u2028Bea\u2029Leo'), 'Ana Bea Leo');
      for (final text in ['김민지\n👩🏽\u200D💻', 'مريم', 'क्\u200Dषमा']) {
        expect(tidyBio(text), text);
      }
    });

    test('hidden characters, too many lines or characters are pointed out', () {
      for (final text in [
        'Ana\u202Eexe.png',
        'A\u200Bna',
        '\u2067Ana',
        'Ana\r\nBea',
        'Ana\tBea',
        'Ana\u0085Bea',
        'Ana\uFEFF',
      ]) {
        expect(bioProblem(text), contains('hidden'), reason: text);
      }
      expect(bioProblem('1\n2\n3\n4'), isNull);
      expect(bioProblem('1\n2\n3\n4\n5'), contains('4 lines'));
      expect(bioProblem('1\n\n\n\n2\n3'), isNull);
      expect(bioProblem('ñ' * maxBioChars), isNull);
      expect(bioProblem('ñ' * (maxBioChars + 1)), contains('190'));
      expect(bioProblem(''), isNull);
    });

    test('what is typed keeps to a bio’s lines', () {
      const lines = BioLines();
      TextEditingValue typed(String text) => TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      );
      final before = typed('1\n2\n3\n4');
      expect(lines.formatEditUpdate(before, typed('1\n2\n3\n4\n')), before);
      expect(
        lines.formatEditUpdate(typed(''), typed('Ana\r\nBea\tLeo')).text,
        'Ana\nBea Leo',
      );
      expect(
        lines.formatEditUpdate(typed(''), typed('1\r2\r3\r4\r5')).text,
        '',
      );
    });
  });

  group('cards', () {
    setUpAll(loadFonts);

    setUp(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('window_manager'),
            (call) async => false,
          );
    });

    testWidgets('a friend’s card shows the bio, the default one, or none', (
      tester,
    ) async {
      final leo = other(0);
      final state = fakeRoom(people: 2)
        ..contacts.friends = [_friend(leo)]
        ..nav.page = Screen.friends;
      await _show(tester, state);
      Future<void> open() async {
        state
          ..nav.friendCard = leo.id
          ..nav.friendCardFrom = const Rect.fromLTWH(600, 200, 400, 60);
        state.notifyListeners();
        await _settle(tester);
      }

      // Never wrote one (or runs a version without bios).
      await open();
      expect(_cardBio(tester), defaultBio);
      expect(find.text('BIO'), findsOneWidget);
      // A changed one shows as soon as it arrives.
      state.contacts.friends = [
        _friend(leo, bio: 'Leo here.\nI play on Fridays.'),
      ];
      state.notifyListeners();
      await _settle(tester);
      expect(_cardBio(tester), 'Leo here.\nI play on Fridays.');
      // Chose to show none.
      state.contacts.friends = [_friend(leo, bio: '')];
      state.notifyListeners();
      await _settle(tester);
      expect(_cardBio(tester), isNull);
      expect(find.text('BIO'), findsNothing);
      await _done(tester);
    });

    testWidgets('the own panel shows this device’s bio', (tester) async {
      final state = fakeRoom(people: 2);
      await _show(tester, state);
      state.openUserPanel(const Rect.fromLTWH(80, 800, 200, 50));
      await _settle(tester);
      expect(_cardBio(tester), defaultBio);
      state.contacts.bio = 'Marta, from Leo’s group.';
      state.notifyListeners();
      await _settle(tester);
      expect(_cardBio(tester), 'Marta, from Leo’s group.');
      await _done(tester);
    });

    testWidgets('My profile starts with the default bio, and Save waits for a '
        'change', (tester) async {
      final state = fakeRoom(people: 2);
      await _show(tester, state);
      state.openSettings(SettingsSection.profile);
      await _settle(tester);
      final field = find.byKey(const ValueKey('bio-field'));
      expect(tester.widget<TextField>(field).controller!.text, defaultBio);
      expect(
        find.text('${defaultBio.runes.length}/$maxBioChars'),
        findsOneWidget,
      );
      FgButton save() =>
          tester.widget<FgButton>(find.widgetWithText(FgButton, 'Save'));
      expect(save().onTap, isNull);
      await tester.enterText(field, 'Leo’s friend.\nHere on weekends.');
      await _settle(tester);
      expect(save().onTap, isNotNull);
      // Hidden characters keep it from being saved, and say why.
      await tester.enterText(field, 'Ana\u202Egnp.exe');
      await _settle(tester);
      expect(save().onTap, isNull);
      expect(find.textContaining('hidden characters'), findsOneWidget);
      // Emptied: the card shows none.
      await tester.enterText(field, '');
      await _settle(tester);
      expect(save().onTap, isNotNull);
      expect(find.text('Your card shows no bio.'), findsOneWidget);
      await _done(tester);
    });
  });
}
