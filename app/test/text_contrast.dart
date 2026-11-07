// The contrast of the text on screen with what is painted under it, worked out from the
// colours the widgets paint with rather than from a picture of the screen. Flutter's own
// textContrastGuideline takes the commonest light and dark pixels around each label, and
// how thin a font's letters are drawn changes with the platform: on Linux the commonest
// light pixel of a 14 px word is often a half-covered edge, a blend of the text and its
// surface, so it measured the theme's red (#FF6166 on black, 6.8:1) as 2.8:1. This walks
// the render tree in the order it paints, at the middle of each text, and composites the
// fills, the translucent layers and the text itself as the screen would, so it gives the
// same answer on every platform and fails only for colours that do not read.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'theme_test.dart' show contrast;

/// Text on screen reads at WCAG's AA contrast against what is painted under it: 4.5:1,
/// or 3:1 for large text (18 px, or 14 px bold). Text no one sees (offstage, or under an
/// opacity of zero) and icon glyphs are not text to read and are left out. A surface
/// under text whose colour cannot be told (a gradient or an image) fails, rather than
/// pass unchecked.
const AccessibilityGuideline tokenTextContrastGuideline = _TokenTextContrast();

class _TokenTextContrast extends AccessibilityGuideline {
  const _TokenTextContrast();

  @override
  String get description => 'Text contrast should follow WCAG guidelines';

  @override
  Future<Evaluation> evaluate(WidgetTester tester) async {
    // The colour of each ColoredBox, whose render object is private to Flutter.
    final colored = <RenderObject, Color>{
      for (final element in find.byType(ColoredBox).evaluate())
        if (element.renderObject != null)
          element.renderObject!: (element.widget as ColoredBox).color,
    };
    var result = const Evaluation.pass();
    var checked = 0;
    for (final view in tester.binding.renderViews) {
      final root = view.child;
      if (root == null) continue;
      final screen = Offset.zero & view.size;
      final texts = <RenderParagraph>[];
      void collect(RenderObject node) {
        if (node is RenderParagraph) texts.add(node);
        node.visitChildren(collect);
      }

      collect(root);
      for (final text in texts) {
        if (!text.attached || !text.hasSize || text.size.isEmpty) continue;
        final runs = _runs(text.text);
        if (runs.isEmpty) continue;
        final middle = MatrixUtils.transformPoint(
          text.getTransformTo(null),
          text.size.center(Offset.zero),
        );
        if (!screen.contains(middle)) continue;
        final painter = _Painter(colored, middle, text);
        final under = painter.paint(root, null);
        // Not painted at all: offstage, or under an opacity of zero.
        if (!painter.reached) continue;
        for (final run in runs) {
          final seen = _Painter(colored, middle, text).paint(root, run.color);
          final problem = painter.problem;
          if (problem != null) {
            result += Evaluation.fail('"${run.words}": $problem');
            continue;
          }
          if (under.a < 1) {
            result += Evaluation.fail(
              '"${run.words}": nothing opaque is painted under it',
            );
            continue;
          }
          checked++;
          final large =
              run.size >= 18 ||
              (run.size >= 14 && run.weight.value >= FontWeight.bold.value);
          final needed = large ? 3.0 : 4.5;
          final ratio = contrast(seen, under);
          if (ratio < needed) {
            result += Evaluation.fail(
              '"${run.words}" (${run.size} px, ${run.weight}): expected a '
              'contrast of at least $needed but found '
              '${ratio.toStringAsFixed(2)}, its colour ${run.color} showing '
              'as $seen on $under',
            );
          }
        }
      }
    }
    // A check that finds no text checks nothing, and must not pass as if it had.
    if (checked == 0) result += const Evaluation.fail('no text was checked');
    return result;
  }
}

/// A stretch of text of one colour, size and weight.
class _Run {
  _Run(this.words, this.color, this.size, this.weight);
  final String words;
  final Color color;
  final double size;
  final FontWeight weight;
}

/// The stretches of readable text of a paragraph, each with the style it inherits from
/// the spans around it. Icon glyphs (private use characters) are not words to read.
List<_Run> _runs(InlineSpan span) {
  final runs = <_Run>[];
  void visit(InlineSpan span, TextStyle inherited) {
    final style = inherited.merge(span.style);
    if (span is TextSpan) {
      final words = span.text ?? '';
      final readable = words.runes.any(
        (r) =>
            String.fromCharCode(r).trim().isNotEmpty &&
            !(r >= 0xE000 && r <= 0xF8FF) &&
            r < 0xF0000,
      );
      if (readable) {
        runs.add(
          _Run(
            words,
            style.foreground?.color ?? style.color ?? const Color(0xFF000000),
            style.fontSize ?? 14,
            style.fontWeight ?? FontWeight.normal,
          ),
        );
      }
      for (final child in span.children ?? const <InlineSpan>[]) {
        visit(child, style);
      }
    }
  }

  visit(span, const TextStyle());
  return runs;
}

/// Paints one point of the screen, the middle of [text], as the render tree would: each
/// fill that covers it over what was there, layers of partial opacity mixed with what
/// is under them, and, when given a colour, [text] as that colour over its surface.
class _Painter {
  _Painter(this.colored, this.point, this.text);

  final Map<RenderObject, Color> colored;
  final Offset point;
  final RenderParagraph text;

  /// Whether [text] was painted at all.
  bool reached = false;

  /// Why the colour under the text cannot be told, when it cannot.
  String? problem;

  Color paint(RenderObject root, Color? textColor) =>
      _paint(root, const Color(0x00000000), textColor);

  bool _covers(RenderObject node) {
    if (node is! RenderBox || !node.hasSize) return false;
    final area = MatrixUtils.transformRect(
      node.getTransformTo(null),
      Offset.zero & node.size,
    );
    return area.contains(point);
  }

  /// The fill [node] paints under its children, if it covers the point.
  Color? _fill(RenderObject node, {required bool behind}) {
    Decoration? decoration;
    Color? color;
    if (node is RenderDecoratedBox) {
      if ((node.position == DecorationPosition.background) != behind) {
        return null;
      }
      decoration = node.decoration;
    } else if (!behind) {
      return null;
    } else if (node is RenderPhysicalModel) {
      color = node.color;
    } else if (node is RenderPhysicalShape) {
      color = node.color;
    } else {
      color = colored[node];
    }
    if (decoration == null && color == null) return null;
    if (!_covers(node)) return null;
    if (decoration is BoxDecoration) {
      if (decoration.gradient != null || decoration.image != null) {
        problem ??= 'it is over a gradient or an image';
      }
      return decoration.color;
    }
    if (decoration is ShapeDecoration) {
      if (decoration.gradient != null || decoration.image != null) {
        problem ??= 'it is over a gradient or an image';
      }
      return decoration.color;
    }
    if (decoration != null) {
      problem ??= 'it is over a ${decoration.runtimeType}';
    }
    return color;
  }

  Color _paint(RenderObject node, Color under, Color? textColor) {
    if (node is RenderOffstage && node.offstage) return under;
    var opacity = 1.0;
    if (node is RenderOpacity) opacity = node.opacity;
    if (node is RenderAnimatedOpacity) opacity = node.opacity.value;
    if (opacity == 0) return under;
    var color = under;
    final behind = _fill(node, behind: true);
    if (behind != null) color = Color.alphaBlend(behind, color);
    if (identical(node, text)) {
      reached = true;
      if (textColor != null) color = Color.alphaBlend(textColor, color);
    }
    if (node is RenderIndexedStack) {
      final shown = node.index;
      var i = 0;
      node.visitChildren((child) {
        if (i++ == shown) color = _paint(child, color, textColor);
      });
    } else {
      node.visitChildren((child) => color = _paint(child, color, textColor));
    }
    final over = _fill(node, behind: false);
    if (over != null) color = Color.alphaBlend(over, color);
    return opacity == 1 ? color : Color.lerp(under, color, opacity)!;
  }
}
