import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/shown_text.dart';

void main() {
  test('ordinary text is drawn as written', () {
    for (final text in [
      'hello',
      'Café with José and Zoë?',
      'line one\nline two',
      'Tiếng Việt',
      '\u{1F44D}\u{1F3FD} 1️⃣ \u{1F468}\u200D\u{1F469}\u200D\u{1F467}',
      '',
    ]) {
      expect(shownText(text), text);
    }
  });

  test('marks that turn text around are not drawn', () {
    expect(shownText('photo\u202Egnp.exe'), 'photognp.exe');
    expect(shownText('a\u202Bb\u202Cc'), 'abc');
    expect(shownText('yes\u2067 no\u2069'), 'yes no');
    expect(shownText('\u2069\u202C\u202D'), '');
  });

  test('a line written right to left is set apart in its own direction', () {
    expect(shownText('שלום'), '\u2068שלום\u2069');
    expect(shownText('hi\nمرحبا!'), 'hi\n\u2068مرحبا!\u2069');
    expect(shownText('\u202Eשל'), '\u2068של\u2069');
  });

  test('a character keeps at most a few marks', () {
    final zalgo = 'a${'́' * 300}b';
    expect(shownText(zalgo), 'a${'́' * maxMarks}b');
    // Each character has its own.
    expect(shownText('ệö'), 'ệö');
  });

  test('lower case keeps the length of every character', () {
    // The chat's search marks what it finds at the places found in the lower-case text.
    for (var c = 0; c < 0x110000; c++) {
      if (c >= 0xD800 && c < 0xE000) continue;
      final s = String.fromCharCode(c);
      expect(
        s.toLowerCase().length,
        s.length,
        reason: 'U+${c.toRadixString(16)}',
      );
    }
    expect('İstanbul ΣΑΣ'.toLowerCase().length, 'İstanbul ΣΑΣ'.length);
  });
}
