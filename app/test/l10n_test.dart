// Every language has every message of English, with the same placeholders, the plural
// categories the language has, and ICU syntax that reads.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/l10n/app_localizations.dart' show lookupL10n;
import 'package:fogata/src/format.dart';
import 'package:fogata/src/l10n.dart';

import 'icu.dart';

/// CLDR's cardinal plural categories of each language.
const _categories = {
  'en': {'one', 'other'},
  'es': {'one', 'many', 'other'},
  'pt': {'one', 'many', 'other'},
  'fr': {'one', 'many', 'other'},
  'de': {'one', 'other'},
  'it': {'one', 'many', 'other'},
  'ru': {'one', 'few', 'many', 'other'},
  'ja': {'other'},
  'zh': {'other'},
};

void main() {
  final arbs = readArbs();
  final en = arbs['en']!;
  final keys = [
    for (final key in en.keys)
      if (!key.startsWith('@')) key,
  ];

  test('there is a file for every language offered, and nothing else', () {
    expect(arbs.keys.toSet(), _categories.keys.toSet());
    expect(languages.length, arbs.length);
    for (final code in languages.keys) {
      expect(L10n.delegate.isSupported(localeOf(code)), isTrue, reason: code);
    }
  });

  test('every English message says where it shows and reads', () {
    for (final key in keys) {
      final meta = en['@$key'] as Map<String, dynamic>?;
      expect(
        (meta?['description'] as String?)?.trim(),
        isNotEmpty,
        reason: key,
      );
      expect(() => parseMessage(en[key] as String), returnsNormally);
    }
  });

  for (final code in _categories.keys.where((c) => c != 'en')) {
    test('$code has every message, with the same placeholders', () {
      final arb = arbs[code]!;
      final problems = <String>[];
      for (final key in keys) {
        final text = arb[key];
        if (text is! String || text.trim().isEmpty) {
          problems.add('$key: missing');
          continue;
        }
        try {
          final theirs = shape(parseMessage(text));
          final ours = shape(parseMessage(en[key] as String));
          if (!_same(theirs.args, ours.args)) {
            problems.add('$key: placeholders ${theirs.args} for ${ours.args}');
          }
          if (!_same(theirs.selects.keys.toSet(), ours.selects.keys.toSet()) ||
              !_same(theirs.plurals.keys.toSet(), ours.plurals.keys.toSet())) {
            problems.add('$key: other plurals or selects than English');
            continue;
          }
          for (final MapEntry(:key, :value) in ours.selects.entries) {
            if (!_same(theirs.selects[key]!, value)) {
              problems.add('$key: select cases ${theirs.selects[key]}');
            }
          }
          for (final MapEntry(key: name, value: cases)
              in theirs.plurals.entries) {
            for (final c in cases.where((c) => !c.startsWith('='))) {
              if (!_categories[code]!.contains(c)) {
                problems.add('$key: $name has "$c", not a category of $code');
              }
            }
            if (!cases.contains('other')) {
              problems.add('$key: $name has no "other"');
            }
            // 21, 31… take "one" in Russian, 2-4 "few", 5-20 "many".
            if (code == 'ru' &&
                !cases.containsAll(const {'one', 'few', 'many'})) {
              problems.add('$key: $name lacks one, few or many');
            }
          }
          problems.addAll(_pluralPitfalls(code, key, parseMessage(text)));
        } on FormatException catch (e) {
          problems.add('$key: ${e.message}');
        }
      }
      for (final key in arb.keys) {
        if (!key.startsWith('@') && !en.containsKey(key)) {
          problems.add('$key: not in English');
        }
      }
      expect(problems, isEmpty);
    });
  }

  test('each language words a few things its own way', () {
    addTearDown(() => useLanguage('en'));
    final now = DateTime(2026, 10, 30, 12);
    final lastWeek = DateTime(2026, 10, 21, 20, 13).millisecondsSinceEpoch;
    useLanguage('de');
    expect(l10n.commonCancel, 'Abbrechen');
    expect(bytes(48200000), '48,2\u00a0MB');
    useLanguage('fr');
    expect(bytes(48200000), contains('48,2'));
    expect(bytes(48200000), contains('Mo'));
    useLanguage('ru');
    // Russian counts: 1 and 21 one way, 2 and 24 another, 5 and 11 a third.
    final one = l10n.agoDays(21),
        few = l10n.agoDays(24),
        many = l10n.agoDays(5);
    expect({one, few, many}.length, 3);
    expect(l10n.agoDays(11).replaceAll('11', '5'), many);
    useLanguage('ja');
    expect(dayClock(lastWeek, now: now), contains('10月21日'));
    clock12 = true;
    addTearDown(() => clock12 = false);
    expect(clock(lastWeek), '午後8:13');
    useLanguage('en');
    expect(clock(lastWeek), '8:13 PM');
    expect(dayClock(lastWeek, now: now), 'Oct 21, 8:13 PM');
    useLanguage('zh-Hans');
    expect(l10n.localeName, 'zh');
    expect(intlLocale, 'zh');
  });

  test('a tile whose connection dropped never reads as away', () {
    addTearDown(() => useLanguage('en'));
    for (final code in languages.keys) {
      useLanguage(code);
      // Away is nobody using the computer: a dropped connection is said otherwise.
      expect(
        l10n.roomTileAway('Bea').toLowerCase(),
        isNot(contains(l10n.friendsAway.toLowerCase())),
        reason: code,
      );
    }
  });

  testWidgets(
    'the pseudo-language makes English longer, placeholders as given',
    (tester) async {
      final pseudo = PseudoL10n();
      final english = lookupL10n(const Locale('en'));
      expect(
        pseudoGrowth(english.commonCancel, pseudo.commonCancel),
        greaterThanOrEqualTo(1.35),
      );
      expect(pseudo.timeHoursMinutes(2, 5), contains('2'));
      expect(pseudo.timeHoursMinutes(2, 5), contains('5'));
      expect(pseudo.listTwo('Marta', 'Leo'), startsWith('Marta'));
      expect(pseudo.listTwo('Marta', 'Leo'), endsWith('Leo'));
    },
  );
}

/// Languages whose "one" counts more than 1: 0 in French and Portuguese, 21, 31… in
/// Russian.
const _wideOne = {'fr', 'pt', 'ru'};

/// What a plural of [nodes] would say wrong: gen-l10n gives `=1` and `one` to the same
/// case (so one of the two is lost), and that case is said for every number of the "one"
/// category, so where that holds more than 1 it has to show the number (or, for 0, an
/// `=0` of its own).
List<String> _pluralPitfalls(String code, String key, List<Node> nodes) {
  final found = <String>[];
  // The number shown, by itself or drawn apart through another placeholder.
  bool shows(List<Node> nodes, String name) => nodes.any(
    (n) => switch (n) {
      TextNode() => false,
      ArgNode() => true,
      ChoiceNode(name: final arg, :final cases) =>
        arg == name || cases.values.any((c) => shows(c, name)),
    },
  );
  void walk(List<Node> nodes) {
    for (final n in nodes.whereType<ChoiceNode>()) {
      n.cases.values.forEach(walk);
      if (n.kind != 'plural') continue;
      if (n.cases.containsKey('=1') && n.cases.containsKey('one')) {
        found.add('$key: ${n.name} has both =1 and one');
      }
      final one = n.cases['one'] ?? n.cases['=1'];
      if (_wideOne.contains(code) &&
          one != null &&
          !shows(one, n.name) &&
          (code == 'ru' || !n.cases.containsKey('=0'))) {
        found.add('$key: ${n.name}\'s "one" is said for other numbers too');
      }
    }
  }

  walk(nodes);
  return found;
}

bool _same(Set<String> a, Set<String> b) =>
    a.length == b.length && a.containsAll(b);
