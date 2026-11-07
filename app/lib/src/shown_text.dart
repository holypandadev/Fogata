// What other people wrote, as this device draws it. Only the drawing changes: what is
// kept and sent stays as written.

/// Marks that set the direction of what follows until their end: embeddings, overrides
/// and isolates, and what ends them. Nobody types them, and one can turn text around:
/// `photo\u202Egnp.exe` reads as `photoexe.png`.
bool _directionMark(int r) =>
    (r >= 0x202A && r <= 0x202E) || (r >= 0x2066 && r <= 0x2069);

/// Letters written right to left (Hebrew, Arabic, Syriac, Thaana, N'Ko and the like).
bool _rightToLeft(int r) =>
    (r >= 0x0590 && r <= 0x08FF) ||
    (r >= 0xFB1D && r <= 0xFDFF) ||
    (r >= 0xFE70 && r <= 0xFEFF) ||
    (r >= 0x10800 && r <= 0x10FFF) ||
    (r >= 0x1E800 && r <= 0x1EFFF);

/// Marks drawn on the character before them (accents and the like): a character with a
/// few is ordinary writing, one with hundreds smears over the lines around it.
final _mark = RegExp(r'[\p{Mn}\p{Me}]', unicode: true);

/// Most marks kept on one character.
const maxMarks = 4;

/// [text] as it is drawn: without marks that set the direction of what follows, with at
/// most [maxMarks] marks on one character, and each line with letters written right to
/// left set apart in the direction its first letter says, so it reads as typed.
String shownText(String text) {
  // Marks and letters written right to left all come after U+0300.
  if (text.codeUnits.every((u) => u < 0x300)) return text;
  return text.split('\n').map(_shownLine).join('\n');
}

String _shownLine(String line) {
  final out = StringBuffer();
  var marks = 0;
  var rightToLeft = false;
  for (final r in line.runes) {
    if (_directionMark(r)) continue;
    final ch = String.fromCharCode(r);
    if (_mark.hasMatch(ch)) {
      if (++marks > maxMarks) continue;
    } else {
      marks = 0;
    }
    rightToLeft |= _rightToLeft(r);
    out.write(ch);
  }
  // First strong isolate ... pop directional isolate.
  return rightToLeft ? '\u2068$out\u2069' : out.toString();
}
