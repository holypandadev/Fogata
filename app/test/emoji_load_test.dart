import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/emoji_names.dart';
import 'package:fogata/src/l10n.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('the words of a language are read away from the window', () async {
    useLanguage('it');
    addTearDown(() => useLanguage('en'));
    await loadEmojiWords();
    expect(emojiWordsLoaded, isTrue);
    expect(localEmojiName('❤️'), isNotEmpty);
    // Italian and English, each read on an isolate of its own.
    expect(emojiWordsParsedHere, 0);
  });
}
