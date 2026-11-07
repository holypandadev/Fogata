import 'package:fogata/src/emoji.dart';
import 'package:fogata/src/emoji_names.dart';
import 'package:fogata/src/l10n.dart';
import 'package:flutter_test/flutter_test.dart';

/// The first emoji [query] finds in [language].
Future<String?> _first(String language, String query) async {
  useLanguage(language);
  await loadEmojiWords();
  return searchEmoji(query).firstOrNull?.text;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  tearDown(() => useLanguage('en'));

  test(
    'search finds by the names and keywords of the language in use',
    () async {
      expect(await _first('es', 'corazón'), '❤️');
      expect(await _first('es', 'corazon'), '❤️');
      expect(await _first('es', 'CORAZÓN ROJO'), '❤️');
      expect(await _first('de', 'Herz'), '❤️');
      expect(await _first('ru', 'сердце'), '❤️');
      expect(await _first('fr', 'cœur'), '❤️');
      expect(await _first('fr', 'coeur'), '❤️');
      expect(await _first('ja', 'ハート'), '❤️');
      expect(await _first('ja', 'はーと'), '❤️');
      expect(await _first('zh-Hans', '红心'), '❤️');
      expect(await _first('pt-BR', 'coração'), '❤️');
      expect(await _first('pt-BR', 'coracao'), '❤️');
      expect(await _first('it', 'cuore rosso'), '❤️');
      expect(await _first('zh-Hans', '爱'), isNotNull);
      expect(await _first('es', 'pulgar'), '👍');
      expect(await _first('de', 'daumen'), '👍');
    },
  );

  test(
    'a prefix of a name or keyword is enough, and the name comes first',
    () async {
      useLanguage('es');
      await loadEmojiWords();
      final found = searchEmoji('coraz').map((e) => e.text).toList();
      expect(found, contains('❤️'));
      // The emoji named exactly as typed leads those that only have it as a keyword.
      final name = emojiOf('🔥')!.label;
      expect(name, 'fuego');
      expect(searchEmoji('fuego').first.text, '🔥');
      // The emoji of the language's own name come before ones found by the English fallback.
      final mixed = searchEmoji('pie').map((e) => e.text).toList();
      expect(mixed.first, '🦶');
    },
  );

  test(
    'English is the fallback, and switching language switches the data',
    () async {
      useLanguage('es');
      await loadEmojiWords();
      expect(searchEmoji('thumbs').first.text, '👍');
      expect(searchEmoji('+1').first.text, '👍');
      expect(emojiOf('❤️')!.label, 'corazón rojo');
      useLanguage('de');
      await loadEmojiWords();
      expect(emojiOf('❤️')!.label, 'rotes Herz');
      expect(searchEmoji('corazón'), isEmpty);
      expect(searchEmoji('Herz').first.text, '❤️');
      useLanguage('en');
      expect(emojiOf('❤️')!.label, 'red heart');
      expect(searchEmoji('corazón'), isEmpty);
      expect(searchEmoji('heart').first.text, '❤️');
    },
  );

  test('folding drops accents, spells ligatures out and joins the kana', () {
    expect(foldForSearch('Corazón'), 'corazon');
    expect(foldForSearch('Cœur Ça'), 'coeur ca');
    expect(foldForSearch('Straße'), 'strasse');
    expect(foldForSearch('ёлка'), 'елка');
    expect(foldForSearch('ハート'), 'はーと');
    expect(foldForSearch('红心'), '红心');
    // A letter and its accent written apart.
    expect(foldForSearch('corázón'), 'corazon');
  });

  test('every language has every emoji of the table, with a name', () async {
    for (final language in languages.keys) {
      useLanguage(language);
      await loadEmojiWords();
      final words = currentEmojiWords!;
      for (final e in allEmoji) {
        final w = words[e.text];
        expect(w, isNotNull, reason: '$language ${e.text} ${e.name}');
        expect(w!.name, isNotEmpty, reason: '$language ${e.text}');
      }
      expect(words.length, greaterThanOrEqualTo(allEmoji.length));
    }
  });
}
