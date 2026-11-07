import 'dart:isolate';

import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:flutter/services.dart' show rootBundle;

import 'l10n.dart' show currentLanguage;

/// What one language calls one emoji: its name as shown, and the name and the keywords
/// folded for search (see [foldForSearch]).
class EmojiWords {
  const EmojiWords(this.name, this.folded, this.keywords);
  final String name;
  final String folded;
  final List<String> keywords;
}

/// The words of each language, by the asset's name, loaded the first time they are asked
/// for (see [loadEmojiWords]).
final _words = <String, Map<String, EmojiWords>>{};
final _loading = <String, Future<void>>{};

/// The code of the asset with the emoji words of [language] (one of the app's languages).
String emojiAssetOf(String language) => switch (language) {
  'pt-BR' => 'pt',
  'zh-Hans' => 'zh',
  final code => code,
};

/// Loads the emoji words of the language in use and of English, the fallback; done when
/// they are in memory. Searching before that finds by English names only.
Future<void> loadEmojiWords() {
  final codes = {'en', emojiAssetOf(currentLanguage)};
  return Future.wait([for (final code in codes) _load(code)]);
}

/// Whether the words of the language in use are in memory.
bool get emojiWordsLoaded =>
    _words.containsKey(emojiAssetOf(currentLanguage)) &&
    _words.containsKey('en');

/// The words of the language in use, once loaded.
Map<String, EmojiWords>? get currentEmojiWords =>
    _words[emojiAssetOf(currentLanguage)];

/// The English words, once loaded.
Map<String, EmojiWords>? get englishEmojiWords => _words['en'];

/// The emoji's name in the language in use, when its words are loaded.
String? localEmojiName(String emoji) => currentEmojiWords?[emoji]?.name;

Future<void> _load(String code) => _loading[code] ??= () async {
  try {
    final text = await rootBundle.loadString('assets/emoji/$code.tsv');
    // Thousands of lines folded for search: not on the window's thread, where they held
    // up a frame or two as the picker first opened.
    _words[code] = await Isolate.run(() => parseEmojiWords(text));
  } catch (_) {
    // Without the file the language searches by English names only.
    _words[code] = {};
  }
}();

/// How many emoji words files were read on this isolate, the window's when it is not
/// a test's.
@visibleForTesting
int emojiWordsParsedHere = 0;

/// The lines of an emoji words file, by emoji.
Map<String, EmojiWords> parseEmojiWords(String text) {
  emojiWordsParsedHere++;
  final words = <String, EmojiWords>{};
  for (final line in text.split('\n')) {
    final parts = line.split('\t');
    if (parts.length < 3) continue;
    final name = parts[1];
    words[parts[0]] = EmojiWords(name, foldForSearch(name), [
      for (final word in parts[2].split('|'))
        if (word.isNotEmpty) foldForSearch(word),
    ]);
  }
  return words;
}

// Letters folded to their base: accents go, ligatures spell out, ё is е.
const _bases = {
  'a': 'àáâãäåāăą',
  'c': 'çćĉċč',
  'd': 'ďđ',
  'e': 'èéêëēĕėęě',
  'g': 'ĝğġģ',
  'h': 'ĥħ',
  'i': 'ìíîïĩīĭįı',
  'j': 'ĵ',
  'k': 'ķ',
  'l': 'ĺļľł',
  'n': 'ñńņň',
  'o': 'òóôõöøōŏő',
  'r': 'ŕŗř',
  's': 'śŝşš',
  't': 'ţťŧ',
  'u': 'ùúûüũūŭůűų',
  'w': 'ŵ',
  'y': 'ýÿŷ',
  'z': 'źżž',
  'е': 'ё',
  'oe': 'œ',
  'ae': 'æ',
  'ss': 'ß',
};
final _fold = {
  for (final MapEntry(key: base, value: letters) in _bases.entries)
    for (final letter in letters.split('')) letter: base,
};

/// [text] as it is compared when searching: lower case, without accents (corazón is
/// corazon, cœur is coeur), ё as е, and katakana as hiragana, so that either finds the
/// other. Anything else, Chinese characters included, stays as it is.
String foldForSearch(String text) {
  final out = StringBuffer();
  for (final c in text.toLowerCase().runes) {
    final s = String.fromCharCode(c);
    if (c >= 0x30A1 && c <= 0x30F6) {
      out.writeCharCode(c - 0x60);
    } else if (c >= 0x300 && c <= 0x36F) {
      // A combining accent, as when the text is written with the letter and the accent
      // apart.
    } else {
      out.write(_fold[s] ?? s);
    }
  }
  return out.toString();
}
