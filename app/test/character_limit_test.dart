import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/widgets.dart';

TextEditingValue _typed(String text) => TextEditingValue(
  text: text,
  selection: TextSelection.collapsed(offset: text.length),
);

void main() {
  const limit = CharacterLimit(5);

  test('a text within the limit stays as typed', () {
    final value = _typed('hello');
    expect(limit.formatEditUpdate(TextEditingValue.empty, value), value);
  });

  test('a longer paste is cut at the limit', () {
    final out = limit.formatEditUpdate(
      TextEditingValue.empty,
      _typed('tomorrow!'),
    );
    expect(out.text, 'tomor');
    expect(out.selection.baseOffset, 5);
  });

  test(
    'characters are counted as the core counts them, symbols kept whole',
    () {
      // A family emoji is one symbol made of five characters.
      const family = '\u{1F468}\u200D\u{1F469}\u200D\u{1F467}';
      expect(
        limit.formatEditUpdate(TextEditingValue.empty, _typed('a$family')).text,
        'a',
      );
      expect(
        limit.formatEditUpdate(TextEditingValue.empty, _typed(family)).text,
        family,
      );
    },
  );
}
