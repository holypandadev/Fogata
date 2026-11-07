import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

/// The interface font, Inter, has a glyph for every character of every text the app ships
/// in every language it speaks, in each of its weights: headings (bold) as much as running
/// text. Japanese and Chinese characters are the system's CJK fonts' (theme.dart's
/// cjkFonts), and emoji the colour font's, so only those are left to them; anything else
/// Inter lacked would be drawn by whichever font the system found, in another design.
void main() {
  final weights = [
    'Inter-Regular.ttf',
    'Inter-Medium.ttf',
    'Inter-SemiBold.ttf',
    'Inter-Bold.ttf',
  ];
  final languages = Directory('lib/l10n')
      .listSync()
      .whereType<File>()
      .where((f) => f.path.endsWith('.arb'))
      .toList();

  test('there are the nine languages', () {
    expect(languages, hasLength(9));
  });

  for (final weight in weights) {
    test('$weight covers every language but CJK characters and emoji', () {
      final covered = _codePoints(
        File('assets/fonts/$weight').readAsBytesSync(),
      );
      final missing = <String, Set<int>>{};
      for (final arb in languages) {
        final strings =
            jsonDecode(arb.readAsStringSync()) as Map<String, dynamic>;
        for (final MapEntry(:key, :value) in strings.entries) {
          if (key.startsWith('@') || value is! String) continue;
          for (final rune in value.runes) {
            if (_leftToOthers(rune) || covered.contains(rune)) continue;
            (missing[arb.uri.pathSegments.last] ??= {}).add(rune);
          }
        }
      }
      expect(
        missing.map(
          (file, runes) => MapEntry(
            file,
            runes
                .map(
                  (r) =>
                      'U+${r.toRadixString(16).toUpperCase()} ${String.fromCharCode(r)}',
                )
                .toList(),
          ),
        ),
        isEmpty,
      );
    });
  }

  test('the reading of a font sees what it lacks', () {
    final covered = _codePoints(
      File('assets/fonts/Inter-Bold.ttf').readAsBytesSync(),
    );
    // Latin, Cyrillic and the Euro are Inter's; an ideograph and Thai are not.
    expect(
      ['A', 'ж', '€'].every((c) => covered.contains(c.runes.first)),
      isTrue,
    );
    expect(covered.contains(0x4E00) || covered.contains(0x0E01), isFalse);
  });

  test('the wordmark face has the wordmark', () {
    final covered = _codePoints(
      File('assets/fonts/FogataEmber-Heavy.ttf').readAsBytesSync(),
    );
    expect('Fogata'.runes.where((r) => !covered.contains(r)), isEmpty);
  });
}

/// Characters no font draws (controls such as a line break, and the invisible joiners)
/// or other fonts draw: CJK (punctuation, kana, ideographs, full-width forms), emoji and
/// the variation selector that asks for them in colour.
bool _leftToOthers(int r) =>
    r < 0x20 ||
    (r >= 0x200B && r <= 0x200D) ||
    r == 0x2060 ||
    r == 0xFEFF ||
    (r >= 0x3000 && r <= 0x30FF) ||
    (r >= 0x3400 && r <= 0x9FFF) ||
    (r >= 0xF900 && r <= 0xFAFF) ||
    (r >= 0xFF00 && r <= 0xFFEF) ||
    r == 0xFE0F ||
    r >= 0x1F000;

/// The code points a TrueType or OpenType font maps to glyphs: its Unicode cmap subtable,
/// format 12 when it has one, else format 4.
Set<int> _codePoints(Uint8List font) {
  final data = ByteData.sublistView(font);
  final tables = data.getUint16(4);
  int? cmap;
  for (var i = 0; i < tables; i++) {
    final record = 12 + 16 * i;
    if (String.fromCharCodes(font.sublist(record, record + 4)) == 'cmap') {
      cmap = data.getUint32(record + 8);
    }
  }
  if (cmap == null) throw StateError('no cmap');
  final subtables = <int, int>{};
  for (var i = 0; i < data.getUint16(cmap + 2); i++) {
    final record = cmap + 4 + 8 * i;
    final offset = cmap + data.getUint32(record + 4);
    subtables[data.getUint16(offset)] ??= offset;
  }
  final points = <int>{};
  if (subtables[12] case final t?) {
    for (var g = 0; g < data.getUint32(t + 12); g++) {
      final group = t + 16 + 12 * g;
      final glyph = data.getUint32(group + 8);
      for (var c = data.getUint32(group); c <= data.getUint32(group + 4); c++) {
        if (glyph != 0) points.add(c);
      }
    }
    return points;
  }
  final t = subtables[4];
  if (t == null) throw StateError('no Unicode cmap');
  final segments = data.getUint16(t + 6) ~/ 2;
  final ends = t + 14, starts = ends + 2 * segments + 2;
  final deltas = starts + 2 * segments, ranges = deltas + 2 * segments;
  for (var s = 0; s < segments; s++) {
    final end = data.getUint16(ends + 2 * s),
        start = data.getUint16(starts + 2 * s);
    final delta = data.getInt16(deltas + 2 * s);
    final range = data.getUint16(ranges + 2 * s);
    for (var c = start; c <= end && c != 0xFFFF; c++) {
      final int glyph;
      if (range == 0) {
        glyph = (c + delta) & 0xFFFF;
      } else {
        final at = ranges + 2 * s + range + 2 * (c - start);
        final raw = data.getUint16(at);
        glyph = raw == 0 ? 0 : (raw + delta) & 0xFFFF;
      }
      if (glyph != 0) points.add(c);
    }
  }
  return points;
}
