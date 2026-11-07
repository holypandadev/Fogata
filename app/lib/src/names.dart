/// What of a name shows, by the same rules the core signs and reads names with
/// (crates/fogata-crypto/src/names.rs): the many kinds of space and the characters that draw
/// as blank (Hangul fillers, the braille blank) become one space between words and none at
/// either end; marks, joiners, variation selectors and tag characters with nothing to sit on,
/// and format characters that never draw, are left out. Control characters and marks that
/// hide in text or turn it around are left for [nameProblem] to point out.
library;

import 'l10n.dart';

final _mark = RegExp(r'^\p{M}$', unicode: true);

bool _blank(int c) =>
    c == 0x20 ||
    c == 0xA0 ||
    c == 0x1680 ||
    (c >= 0x2000 && c <= 0x200A) ||
    c == 0x2028 ||
    c == 0x2029 ||
    c == 0x202F ||
    c == 0x205F ||
    c == 0x3000 ||
    c == 0x115F ||
    c == 0x1160 ||
    c == 0x180E ||
    c == 0x2800 ||
    c == 0x3164 ||
    c == 0xFFA0 ||
    c == 0x1D159;

bool _unseen(int c) =>
    c == 0xAD ||
    c == 0x17B4 ||
    c == 0x17B5 ||
    c == 0x2065 ||
    (c >= 0x206A && c <= 0x206F) ||
    (c >= 0xFFF0 && c <= 0xFFFB) ||
    (c >= 0x1BCA0 && c <= 0x1BCA3) ||
    (c >= 0x1D173 && c <= 0x1D17A) ||
    (c >= 0xE0000 && c <= 0xE001F) ||
    (c >= 0xE0080 && c <= 0xE00FF) ||
    (c >= 0xE01F0 && c <= 0xE0FFF);

bool _joiner(int c) => c == 0x200C || c == 0x200D;

bool _selector(int c) =>
    (c >= 0xFE00 && c <= 0xFE0F) || (c >= 0xE0100 && c <= 0xE01EF);

bool _tag(int c) => c >= 0xE0020 && c <= 0xE007F;

bool _combining(int c) => _mark.hasMatch(String.fromCharCode(c));

const _blackFlag = 0x1F3F4;

/// Control characters, and marks that hide in text or turn it around (direction overrides
/// and isolates, zero-width spaces): Fogata refuses names with them.
bool _hides(int c) =>
    c < 0x20 ||
    (c >= 0x7F && c <= 0x9F) ||
    c == 0x061C ||
    c == 0x200B ||
    c == 0x200E ||
    c == 0x200F ||
    (c >= 0x202A && c <= 0x202E) ||
    (c >= 0x2060 && c <= 0x2064) ||
    (c >= 0x2066 && c <= 0x2069) ||
    c == 0xFEFF;

/// [text] without control characters and marks that hide in text or turn it around.
String withoutHiding(String text) =>
    String.fromCharCodes(text.runes.where((c) => !_hides(c)));

/// What of [name] shows; empty when nothing does.
String tidyName(String name) {
  final out = <int>[];
  var space = false;
  var selectable = false;
  var tags = false;
  void dropJoiners() {
    while (out.isNotEmpty && _joiner(out.last)) {
      out.removeLast();
    }
  }

  for (final c in name.runes) {
    final sits = !space && out.isNotEmpty;
    if (_blank(c)) {
      space = out.isNotEmpty;
      selectable = false;
      tags = false;
      dropJoiners();
      continue;
    }
    if (_unseen(c)) continue;
    if (_tag(c)) {
      if (tags && sits) {
        out.add(c);
        tags = c != 0xE007F;
      }
      selectable = false;
      continue;
    }
    tags = false;
    if (_selector(c)) {
      if (selectable && sits) out.add(c);
      selectable = false;
      continue;
    }
    if (_joiner(c)) {
      if (sits && !_joiner(out.last)) out.add(c);
      selectable = false;
      continue;
    }
    if (_combining(c)) {
      if (sits) out.add(c);
      selectable = false;
      continue;
    }
    if (space) {
      out.add(0x20);
      space = false;
    }
    out.add(c);
    // Plain ASCII text has no variations; keycaps start with a digit, # or *.
    selectable = c > 0x7F || (c >= 0x30 && c <= 0x39) || c == 0x23 || c == 0x2A;
    tags = c == _blackFlag;
  }
  dropJoiners();
  return String.fromCharCodes(out);
}

/// Why [text], typed as a name, cannot be one, in words for people; null when it can (an
/// empty field says nothing: the button stays off).
String? nameProblem(String text) {
  if (text.isEmpty) return null;
  if (text.runes.any(_hides)) {
    return l10n.namesHidden;
  }
  if (tidyName(text).isEmpty) {
    return l10n.namesBlank;
  }
  return null;
}

/// Longest bio, in characters (line breaks included), and most lines in one.
const maxBioChars = 190;
const maxBioLines = 4;

/// What a card shows of someone who never wrote a bio.
String get defaultBio => l10n.namesDefaultBio;

/// What of [text] shows as a bio, by the same rules the core signs and reads bios with
/// (crates/fogata-friends/src/bio.rs): each line as [tidyName] leaves it, blank lines at
/// either end left out and those in between kept as one.
String tidyBio(String text) {
  final lines = <String>[];
  var gap = false;
  for (final raw in text.split('\n')) {
    final line = tidyName(raw);
    if (line.isEmpty) {
      gap = lines.isNotEmpty;
      continue;
    }
    if (gap) {
      lines.add('');
      gap = false;
    }
    lines.add(line);
  }
  return lines.join('\n');
}

/// Why [text], typed as a bio, cannot be one, in words for people; null when it can. An
/// empty bio can: the card then shows none.
String? bioProblem(String text) {
  if (text.runes.any((c) => c != 0x0A && _hides(c))) {
    return l10n.namesBioHidden;
  }
  final bio = tidyBio(text);
  if ('\n'.allMatches(bio).length >= maxBioLines) {
    return l10n.namesBioLines(maxBioLines);
  }
  if (bio.runes.length > maxBioChars) {
    return l10n.namesBioChars(maxBioChars);
  }
  return null;
}
