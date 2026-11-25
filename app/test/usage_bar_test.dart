import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/screens/direct.dart' show UsageBar;
import 'package:fogata/src/theme.dart';

void main() {
  Widget bar(List<(int, Color)> parts, int total) => MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Center(
      child: SizedBox(
        width: 241,
        child: UsageBar(parts: parts, total: total, height: 6),
      ),
    ),
  );

  /// The widths of the bar's parts as drawn.
  List<double> widths(WidgetTester tester) {
    final row = tester.renderObject<RenderFlex>(
      find.descendant(of: find.byType(UsageBar), matching: find.byType(Row)),
    );
    final out = <double>[];
    row.visitChildren((child) => out.add((child as RenderBox).size.width));
    return out;
  }

  testWidgets('a part that appears grows into the bar without overflowing it', (
    tester,
  ) async {
    // What a chat takes: all messages, then a file of about the same size arrives.
    await tester.pumpWidget(bar([(100, Colors.red), (0, Colors.blue)], 100));
    expect(widths(tester), [241]);
    await tester.pumpWidget(bar([(100, Colors.red), (100, Colors.blue)], 200));
    for (var ms = 0; ms <= 600; ms += 16) {
      await tester.pump(const Duration(milliseconds: 16));
      expect(tester.takeException(), isNull, reason: 'at $ms ms');
      final w = widths(tester);
      expect(w.every((x) => x >= 0), isTrue, reason: 'at $ms ms: $w');
      expect(w.fold(0.0, (a, b) => a + b), lessThanOrEqualTo(241.0001));
    }
    final w = widths(tester);
    expect(w, hasLength(2));
    expect(w[0], moreOrLessEquals(120.5, epsilon: 0.01));
    expect(w[1], moreOrLessEquals(120.5, epsilon: 0.01));
  });

  testWidgets('a part that goes shrinks to nothing, never below', (
    tester,
  ) async {
    // The springy curve overshoots: a part going from most of the bar to almost nothing
    // went below nothing.
    await tester.pumpWidget(
      bar([(90, Colors.red), (5, Colors.green), (5, Colors.blue)], 100),
    );
    await tester.pumpWidget(
      bar([(1, Colors.red), (0, Colors.green), (99, Colors.blue)], 100),
    );
    for (var ms = 0; ms <= 600; ms += 16) {
      await tester.pump(const Duration(milliseconds: 16));
      expect(tester.takeException(), isNull, reason: 'at $ms ms');
      final w = widths(tester);
      expect(w.every((x) => x >= 0), isTrue, reason: 'at $ms ms: $w');
      expect(w.fold(0.0, (a, b) => a + b), lessThanOrEqualTo(241.0001));
    }
    final w = widths(tester);
    expect(w, hasLength(2));
    expect(w[0], moreOrLessEquals(2.41, epsilon: 0.01));
    expect(w[1], moreOrLessEquals(238.59, epsilon: 0.01));
  });
}
