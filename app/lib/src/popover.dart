import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'l10n.dart';
import 'widgets.dart';

/// How far a popover keeps from the window's edges.
const popoverMargin = 8.0;

/// Opens what [builder] makes as a popover attached to [anchor]'s box: over it when there
/// is more room above than below, else under it, [gap] away; its left edge on the
/// anchor's, or its right edge with [alignRight]. It always stays inside the window: moved
/// in from an edge, and no taller than the room on its side (what it holds should scroll
/// when it has more); when the window changes size, it moves with the anchor. A click beside it or Esc closes it, Tab goes round inside it, and the
/// focus goes back to where it was, the button that opened it.
Future<T?> showPopover<T>(
  BuildContext anchor, {
  required WidgetBuilder builder,
  required String label,
  bool alignRight = false,
  double gap = 8,
}) async {
  final box = anchor.findRenderObject() as RenderBox?;
  final overlay =
      Overlay.of(anchor, rootOverlay: true).context.findRenderObject()
          as RenderBox?;
  if (box == null || overlay == null || !box.hasSize) return null;
  var from = box.localToGlobal(Offset.zero, ancestor: overlay) & box.size;
  final above = from.top > overlay.size.height - from.bottom;
  final before = FocusManager.instance.primaryFocus;
  final child = Semantics(
    scopesRoute: true,
    namesRoute: true,
    explicitChildNodes: true,
    label: label,
    child: Material(
      type: MaterialType.transparency,
      child: Builder(builder: builder),
    ),
  );
  final result = await showGeneralDialog<T>(
    context: anchor,
    barrierDismissible: true,
    barrierLabel: l10n.commonClose,
    barrierColor: Colors.transparent,
    transitionDuration: const Duration(milliseconds: 260),
    // Placed again whenever the window changes size, beside where the anchor is then
    // (the page under the popover lays out first); where it was last, once it is gone.
    pageBuilder: (context, _, _) => LayoutBuilder(
      builder: (context, window) {
        final box = anchor.mounted
            ? anchor.findRenderObject() as RenderBox?
            : null;
        if (box != null && box.attached && box.hasSize && overlay.attached) {
          from = box.localToGlobal(Offset.zero, ancestor: overlay) & box.size;
        }
        return CustomSingleChildLayout(
          delegate: PopoverPlace(
            from,
            above: from.top > window.maxHeight - from.bottom,
            alignRight: alignRight,
            gap: gap,
          ),
          child: child,
        );
      },
    ),
    // Grows out of the button with the controls' spring.
    transitionBuilder: (context, animation, _, child) => FadeTransition(
      opacity: animation,
      child: ScaleTransition(
        alignment: Alignment(alignRight ? 1 : -1, above ? 1 : -1),
        scale: Tween(begin: .94, end: 1.0).animate(
          CurvedAnimation(
            parent: animation,
            curve: spring,
            reverseCurve: Curves.easeIn,
          ),
        ),
        child: child,
      ),
    ),
  );
  if (before != null && before.context != null && before.canRequestFocus) {
    before.requestFocus();
  }
  return result;
}

/// Where a popover goes beside the box it opens from (see [showPopover]), inside the
/// window [popoverMargin] from its edges.
class PopoverPlace extends SingleChildLayoutDelegate {
  PopoverPlace(
    this.from, {
    required this.above,
    this.alignRight = false,
    this.gap = 8,
  });

  /// The box it opens from, in the window.
  final Rect from;
  final bool above;
  final bool alignRight;
  final double gap;

  /// Below this, the room on its side is too small to be worth keeping to: it may then
  /// cover the box it opens from, still inside the window.
  static const _least = 160.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) {
    final size = constraints.biggest;
    final room = above
        ? from.top - gap - popoverMargin
        : size.height - from.bottom - gap - popoverMargin;
    final all = math.max(0.0, size.height - 2 * popoverMargin);
    return BoxConstraints(
      maxWidth: math.max(0.0, size.width - 2 * popoverMargin),
      maxHeight: room >= _least ? math.min(room, all) : all,
    );
  }

  @override
  Offset getPositionForChild(Size size, Size childSize) {
    final x = alignRight ? from.right - childSize.width : from.left;
    final y = above ? from.top - gap - childSize.height : from.bottom + gap;
    final right = size.width - childSize.width - popoverMargin;
    final bottom = size.height - childSize.height - popoverMargin;
    return Offset(
      x.clamp(popoverMargin, math.max(popoverMargin, right)),
      y.clamp(popoverMargin, math.max(popoverMargin, bottom)),
    );
  }

  @override
  bool shouldRelayout(PopoverPlace old) =>
      old.from != from ||
      old.above != above ||
      old.alignRight != alignRight ||
      old.gap != gap;
}
