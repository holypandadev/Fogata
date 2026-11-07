import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/names.dart';
import 'package:fogata/src/screens/new_group.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

void main() {
  test('fillers and spaces alone leave nothing', () {
    for (final blank in [
      '\u3164',
      '\u115F\u1160',
      '\uFFA0\uFFA0',
      '\u2800',
      '\u180E',
      '\u{1D159}',
      '\u00A0',
      '\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200A',
      '\u202F\u205F\u3000',
      '  \u3164 \u2800 ',
    ]) {
      expect(tidyName(blank), '', reason: blank.runes.toString());
    }
  });

  test('padding is trimmed and runs collapse', () {
    expect(tidyName('Ana\u3164\u3164\u3164'), 'Ana');
    expect(tidyName('\u2800\u2800Ana'), 'Ana');
    expect(tidyName('  Ana \u3000\u00A0 Bea  '), 'Ana Bea');
    expect(tidyName('Ana\u115FBea'), 'Ana Bea');
  });

  test('marks, joiners, selectors and tags with nothing to sit on go', () {
    expect(tidyName('\u0301\u0301'), '');
    expect(tidyName('\u20E3'), '');
    expect(tidyName('Ana \u0301Bea'), 'Ana Bea');
    expect(tidyName('\uFE0F\u{E0100}'), '');
    expect(tidyName('Ana\uFE0F'), 'Ana');
    expect(tidyName('\u{E0041}\u{E007F}'), '');
    expect(tidyName('Ana\u{E0067}'), 'Ana');
    expect(tidyName('\u200D\u200C'), '');
    expect(tidyName('Ana\u200D'), 'Ana');
    expect(tidyName('a\u200D\u200D\u200Db'), 'a\u200Db');
    expect(tidyName('\u00AD\u17B4\u206A\u{1D173}\u{E0001}'), '');
  });

  test('real names in any script are kept', () {
    for (final name in [
      'Marta',
      'Sofía',
      '김민지',
      '이 서연',
      'مريم',
      'می\u200Cخواهم',
      'नमस्ते',
      'क्\u200Dषमा',
      'สวัสดี',
      'e\u0301',
      '\u{1F468}\u200D\u{1F469}\u200D\u{1F467}',
      '\u{1F469}\u{1F3FD}\u200D\u{1F4BB}',
      '\u{1F3F3}\uFE0F\u200D\u{1F308}',
      '1\uFE0F\u20E3',
      '\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}',
    ]) {
      expect(tidyName(name), name);
      expect(nameProblem(name), isNull);
    }
  });

  test('a name says why it cannot be one', () {
    expect(nameProblem(''), isNull);
    expect(nameProblem('\u3164\u3164'), contains('Nothing in this name shows'));
    expect(nameProblem('\u0301'), contains('Nothing in this name shows'));
    expect(nameProblem('Ana\u202Eexe'), contains('hidden characters'));
    expect(nameProblem('A\u200Bna'), contains('hidden characters'));
    expect(nameProblem('Ana\uFEFF'), contains('hidden characters'));
    expect(nameProblem('Ana\nBea'), contains('hidden characters'));
  });

  testWidgets('a group name that shows nothing cannot be made, and says why', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-name-rules-test');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Builder(
            builder: (context) => Material(
              child: TextButton(
                onPressed: () => showNewGroup(context),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    final field = find.descendant(
      of: find.byWidgetPredicate(
        (w) => w is FgField && w.label == 'Group name',
      ),
      matching: find.byType(TextField),
    );
    FgButton create() =>
        tester.widget<FgButton>(find.widgetWithText(FgButton, 'Create group'));

    await tester.enterText(field, '\u3164\u2800\u3164');
    await tester.pump();
    expect(create().onTap, isNull);
    expect(find.textContaining('Nothing in this name shows'), findsOneWidget);

    await tester.enterText(field, '\u3164Club\u3164');
    await tester.pump();
    expect(create().onTap, isNotNull);
    expect(find.byType(NameNote), findsOneWidget);
    expect(find.textContaining('Nothing in this name shows'), findsNothing);
  });
}
