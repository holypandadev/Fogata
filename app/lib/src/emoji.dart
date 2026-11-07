import 'dart:io';

import 'package:flutter/widgets.dart';

import 'emoji_names.dart';
import 'emoji_table.dart';
import 'host.dart';
import 'icons.dart';
import 'l10n.dart';
import 'theme.dart' show emojiFont;

export 'emoji_names.dart' show emojiWordsLoaded, loadEmojiWords;

/// The groups the picker shows, in its order, with their icons.
enum EmojiGroup {
  people(Ic.smiley),
  nature(Ic.leaf),
  food(Ic.forkKnife),
  activities(Ic.ball),
  travel(Ic.airplane),
  objects(Ic.lightbulb),
  symbols(Ic.heart),
  flags(Ic.flag);

  const EmojiGroup(this.icon);
  final Ic icon;

  /// The group's name, as the picker shows it.
  String get title => switch (this) {
    EmojiGroup.people => l10n.emojiGroupPeople,
    EmojiGroup.nature => l10n.emojiGroupNature,
    EmojiGroup.food => l10n.emojiGroupFood,
    EmojiGroup.activities => l10n.emojiGroupActivities,
    EmojiGroup.travel => l10n.emojiGroupTravel,
    EmojiGroup.objects => l10n.emojiGroupObjects,
    EmojiGroup.symbols => l10n.emojiGroupSymbols,
    EmojiGroup.flags => l10n.emojiGroupFlags,
  };
}

/// One emoji of the table: the default (yellow) form, its name and whether skin tones
/// apply to it.
class Emoji {
  const Emoji(this.text, this.name, this.group, {this.tones = false});
  final String text;
  final String name;
  final EmojiGroup group;
  final bool tones;

  /// The name in the language in use, as the picker shows it: English until that
  /// language's words are loaded (see [loadEmojiWords]).
  String get label => localEmojiName(text) ?? name;

  /// The emoji in skin tone [tone], 1 (light) to 5 (dark); 0 or an emoji without tones
  /// gives the default form. The modifier goes after the first code point, in place of
  /// its presentation selector.
  String toned(int tone) {
    if (!tones || tone < 1 || tone > 5) return text;
    final runes = text.runes.toList();
    var rest = runes.sublist(1);
    if (rest.isNotEmpty && rest.first == 0xFE0F) rest = rest.sublist(1);
    return String.fromCharCodes([runes.first, 0x1F3FA + tone, ...rest]);
  }

  @override
  String toString() => text;
}

/// The five skin tone modifiers, light to dark.
const skinTones = ['🏻', '🏼', '🏽', '🏾', '🏿'];

/// Common short names that are not words of the emoji's own name, as in other chat apps.
const _aliases = {
  'heart': '❤️',
  '+1': '👍',
  'thumbsup': '👍',
  'like': '👍',
  '-1': '👎',
  'thumbsdown': '👎',
  'smile': '😄',
  'laughing': '😆',
  'joy': '😂',
  'lol': '😂',
  'rofl': '🤣',
  'cry': '😢',
  'sob': '😭',
  'tada': '🎉',
  'party': '🥳',
  'ok': '👌',
  'wave': '👋',
  'pray': '🙏',
  'thanks': '🙏',
  'clap': '👏',
  'thinking': '🤔',
  'shrug': '🤷',
  'wink': '😉',
  'blush': '😊',
  'grin': '😁',
  'sweat_smile': '😅',
  'love': '😍',
  'kiss': '😘',
  'angry': '😠',
  'sad': '😞',
  'yes': '✅',
  'no': '❌',
  'check': '✅',
  'poop': '💩',
  'muscle': '💪',
  'eyes': '👀',
  '100': '💯',
};

List<Emoji>? _all;
Map<String, Emoji>? _byText;

/// Every emoji in the picker's order. Country and region flags are left out on Windows,
/// whose emoji font draws them as two letters.
List<Emoji> get allEmoji => _all ??= _read();

/// The emoji whose default form is [text], if the table has it.
Emoji? emojiOf(String text) {
  _byText ??= {for (final e in allEmoji) e.text: e};
  return _byText![text];
}

List<Emoji> _read() {
  final list = <Emoji>[];
  for (final group in EmojiGroup.values) {
    for (final line in (emojiTable[group.name] ?? '').split('\n')) {
      final tab = line.indexOf('\t');
      if (tab <= 0) continue;
      var text = line.substring(0, tab);
      final tones = text.endsWith('~');
      if (tones) text = text.substring(0, text.length - 1);
      if (onWindows && group == EmojiGroup.flags && _regional(text)) {
        continue;
      }
      list.add(Emoji(text, line.substring(tab + 1), group, tones: tones));
    }
  }
  return list;
}

/// A flag made of two regional letters, or of the black flag and tag letters.
bool _regional(String text) {
  final runes = text.runes.toList();
  return runes.first >= 0x1F1E6 && runes.first <= 0x1F1FF ||
      runes.any((r) => r >= 0xE0020 && r <= 0xE007F);
}

String _plain(String text) =>
    text.toLowerCase().replaceAll(RegExp(r'[\s_:’]'), '');

final _wordBreak = RegExp(r'[\s:,\-·・]+');

/// Emoji that lead the others a word is a keyword of, in any language: the red heart
/// among the hearts, the thumb up among the thumbs.
const _lead = {'❤️', '👍'};
List<String>? _folded;

/// The English names of [allEmoji], folded for search.
List<String> get _foldedNames =>
    _folded ??= [for (final e in allEmoji) foldForSearch(e.name)];

/// How well [q] (folded, with spaces) matches an emoji called [name] (folded) with
/// [keywords] (folded); [flat] is [q] without its spaces. Higher is better, 0 is no match:
/// the name itself 9, the start of the name 7, a keyword 6, the start of a word of the
/// name 5, the start of a keyword (or of a word of it) 4, the name with its spaces left
/// out ("thumbsup") containing it 3, a keyword containing it 2.
int _score(String q, String flat, String name, List<String> keywords) {
  if (name == q) return 9;
  if (name.startsWith(q)) return 7;
  if (keywords.contains(q)) return 6;
  if (name.split(_wordBreak).any((w) => w.startsWith(q))) return 5;
  bool starts(String w) =>
      w.startsWith(q) || w.split(_wordBreak).any((p) => p.startsWith(q));
  if (keywords.any(starts)) return 4;
  // Written without spaces, and a single character only for languages that have no
  // spaces between words.
  if (flat.length > 1 || flat.runes.first > 0x2E80) {
    if (_plain(name).contains(flat)) return 3;
    if (keywords.any((w) => _plain(w).contains(flat))) return 2;
  }
  return 0;
}

/// The emoji whose name, common short name or keywords match [query], best first. The
/// names and keywords in the language in use come before English ones, the words need
/// no accents ("corazon" finds corazón) and a prefix or a word of a name is enough. In
/// each language the name itself comes first (and the [_lead] emoji that have it in a
/// word of their name or as a keyword), then names that start with the query, keywords,
/// names with a word that starts with it, keywords that do, and names that contain it
/// with the spaces left out. A common short name ("+1", "joy") comes first among the
/// English ones. Among equals [recent] come first, then the table's order. The words of
/// the language are those [loadEmojiWords] has loaded; before that only the English
/// names are searched.
List<Emoji> searchEmoji(String query, {List<String> recent = const []}) {
  final raw = query.trim().replaceAll(':', '').replaceAll('_', ' ');
  final q = foldForSearch(raw);
  if (q.isEmpty) return const [];
  final flat = _plain(q);
  final key = q.replaceAll(' ', '_');
  final aliased = _aliases[key] ?? _aliases[flat];
  final started = {
    for (final MapEntry(key: name, value: text) in _aliases.entries)
      if (name.startsWith(key)) text,
  };
  final local = currentLanguage == 'en' ? null : currentEmojiWords;
  final english = englishEmojiWords;
  final scored = <(Emoji, int, int, int, int)>[];
  final all = allEmoji;
  for (var i = 0; i < all.length; i++) {
    final e = all[i];
    var here = 0;
    if (local?[e.text] case final words?) {
      here = _score(q, flat, words.folded, words.keywords);
      if (here >= 6 && _lead.contains(e.text)) here = 9;
    }
    var there = _score(
      q,
      flat,
      _foldedNames[i],
      english?[e.text]?.keywords ?? const [],
    );
    // The short names rank among the English matches: exactly one above all, those
    // that start with the query level with the name itself.
    if (e.text == aliased) {
      there = 10;
    } else if (started.contains(e.text) && there < 8) {
      there = 8;
    }
    if (here == 0 && there == 0) continue;
    final seen = recent.indexOf(e.text);
    scored.add((e, here, there, seen < 0 ? recent.length : seen, i));
  }
  scored.sort((a, b) {
    for (final (x, y) in [
      (b.$2, a.$2),
      (b.$3, a.$3),
      (a.$4, b.$4),
      (a.$5, b.$5),
    ]) {
      if (x != y) return x.compareTo(y);
    }
    return 0;
  });
  return [for (final (e, _, _, _, _) in scored) e];
}

/// How an emoji is written between colons: its name in lower case, underscores between
/// the words ("red_heart", "flag_spain").
String shortName(Emoji emoji) => emoji.name
    .toLowerCase()
    .replaceAll(RegExp(r'[\s:,.’!“”()]+'), '_')
    .replaceAll(RegExp(r'^_+|_+$'), '');

/// The emoji written as [name] between colons: a common short name ("joy", "+1") or
/// its own name with underscores ("red_heart").
Emoji? emojiNamed(String name) {
  final key = name.toLowerCase();
  if (_aliases[key] case final text?) return emojiOf(text);
  for (final e in allEmoji) {
    if (shortName(e) == key) return e;
  }
  return null;
}

// A symbol asking for its emoji form (a presentation selector after it), a keycap, and
// what a zero width joiner adds to them (❤️‍🔥).
final _selected = RegExp(
  r'(?:[#*0-9]️⃣|[©® -㏿]️)'
  r'(?:‍(?:[ -㏿]|[\uD83C-\uD83E][\uDC00-\uDFFF])️?)*',
);

/// [text] in [style], with each symbol written in its emoji form drawn in the colour
/// emoji font: the interface font has plain forms of some (❤ ☀ ✈) and would take them.
TextSpan emojiText(String text, TextStyle style) {
  if (!text.contains('️')) return TextSpan(text: text, style: style);
  final emoji = style.copyWith(fontFamily: emojiFont);
  final spans = <TextSpan>[];
  var at = 0;
  for (final m in _selected.allMatches(text)) {
    if (m.start > at) spans.add(TextSpan(text: text.substring(at, m.start)));
    spans.add(TextSpan(text: m.group(0), style: emoji));
    at = m.end;
  }
  if (at < text.length) spans.add(TextSpan(text: text.substring(at)));
  return TextSpan(style: style, children: spans);
}

/// A style that draws emoji in the system's colour font.
TextStyle emojiStyle(double size) => TextStyle(
  fontFamily: emojiFont,
  fontSize: size,
  height: 1.15,
  leadingDistribution: TextLeadingDistribution.even,
);

bool? _hasColorFont;

/// Whether this Linux system has a font with colour emoji; asked of fontconfig once.
/// Elsewhere the system's own emoji font is always there.
Future<bool> hasColorEmojiFont() async {
  if (!Platform.isLinux) return true;
  if (_hasColorFont case final known?) return known;
  try {
    final found = await Process.run('fc-list', [
      ':charset=1f600:color=true',
      'family',
    ]);
    _hasColorFont =
        found.exitCode != 0 || (found.stdout as String).trim().isNotEmpty;
  } catch (_) {
    // Without fontconfig's tool there is no telling: no note then.
    _hasColorFont = true;
  }
  return _hasColorFont!;
}

/// Puts [text] in place of the selection of [controller], or at its cursor (at the end
/// when it has none), leaving the cursor after it. Nothing changes when the message
/// would go over [max] characters.
bool insertAtCursor(TextEditingController controller, String text, {int? max}) {
  final value = controller.value;
  final all = value.text;
  var sel = value.selection;
  if (!sel.isValid) sel = TextSelection.collapsed(offset: all.length);
  final next = all.replaceRange(sel.start, sel.end, text);
  if (max != null && next.runes.length > max) return false;
  controller.value = TextEditingValue(
    text: next,
    selection: TextSelection.collapsed(offset: sel.start + text.length),
  );
  return true;
}
