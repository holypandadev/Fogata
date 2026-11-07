// Text cut by the box it was given, for the layout tests: an overflow of a Row or a Column
// says so itself, but a line of text in a control of fixed height is cut without a word.

import 'dart:ui' show BoxHeightStyle;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// The text on screen whose letters do not fit the height it was given, each with its
/// size, its words and the widgets it is in. The room a line keeps above and below its
/// letters may be lost; the letters themselves may not. Where the letters are is asked
/// of the laid out text, with the font's own ascent and descent.
List<String> cutText() {
  final cut = <String>[];
  for (final element in find.byType(RichText).evaluate()) {
    final text = element.renderObject;
    if (text is! RenderParagraph || !text.attached || !text.hasSize) continue;
    final length = text.text.toPlainText(includeSemanticsLabels: false).length;
    if (length == 0) continue;
    // Each line's letters, from the font's ascent to its descent, whatever the height
    // of the line around them.
    final boxes = text.getBoxesForSelection(
      TextSelection(baseOffset: 0, extentOffset: length),
      boxHeightStyle: BoxHeightStyle.tight,
    );
    if (boxes.isEmpty) continue;
    var top = double.infinity;
    var bottom = double.negativeInfinity;
    for (final box in boxes) {
      if (box.top < top) top = box.top;
      if (box.bottom > bottom) bottom = box.bottom;
    }
    // The font's ascent and descent leave room above and below the letters for accents
    // and the lowest tails: a little of it may go unseen.
    var size = text.text.style?.fontSize ?? 14;
    text.text.visitChildren((span) {
      final own = span.style?.fontSize;
      if (own != null && own > size) size = own;
      return true;
    });
    final spare = text.textScaler.scale(size) * .15 + .5;
    if (top < -spare || bottom > text.size.height + spare) {
      cut.add(
        'letters from ${top.toStringAsFixed(1)} to ${bottom.toStringAsFixed(1)} '
        'in ${text.size.height.toStringAsFixed(1)}: '
        '"${text.text.toPlainText()}" in ${element.debugGetCreatorChain(10)}',
      );
    }
  }
  return cut;
}
