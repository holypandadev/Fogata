import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;
import 'dart:ui' show ImageFilter;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'icons.dart';
import 'l10n.dart';
import 'motion.dart';
import 'moving.dart';
import 'names.dart';
import 'round.dart';
import 'safe_image.dart';
import 'rust/api/types.dart';
import 'shown.dart';
import 'theme.dart';

/// Something that can be clicked or activated with the keyboard, drawn by [builder] with
/// its hover and focus state.
class Pressable extends StatefulWidget {
  const Pressable({
    super.key,
    required this.builder,
    this.onTap,
    this.tooltip,
    this.label,
    this.selected,
    this.expanded,
    this.focusRadius = 6,
  });

  final Widget Function(BuildContext context, bool hovered) builder;
  final VoidCallback? onTap;
  final String? tooltip;

  /// What screen readers say it is; its [tooltip] when not given.
  final String? label;

  /// Whether it is the one chosen among several, for screen readers; null where nothing
  /// is chosen.
  final bool? selected;

  /// Whether what it opens is open, for screen readers; null where it opens nothing.
  final bool? expanded;
  final double focusRadius;

  @override
  State<Pressable> createState() => _PressableState();
}

class _PressableState extends State<Pressable> {
  bool _hovered = false;
  bool _focused = false;

  @override
  void initState() {
    super.initState();
    KeyboardFocus.watch();
  }

  @override
  Widget build(BuildContext context) {
    final enabled = widget.onTap != null;
    Widget child = FocusableActionDetector(
      enabled: enabled,
      mouseCursor: enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onShowFocusHighlight: (v) => setState(() => _focused = v),
      actions: {
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            widget.onTap?.call();
            return null;
          },
        ),
      },
      // The pointer on it, from the pointer itself: the detector's own hover highlight
      // goes with the focus rings, which a click turns off (KeyboardFocus), and nothing
      // would show under the pointer after the first click.
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: Semantics(
            button: true,
            enabled: enabled,
            selected: widget.selected,
            expanded: widget.expanded,
            label: widget.label ?? widget.tooltip,
            child: widget.builder(context, _hovered && enabled),
          ),
        ),
      ),
    );
    // Always there, drawn only while focused: wrapping the detector only then rebuilt
    // it, and the focus it had just been given went with the old one.
    child = DecoratedBox(
      position: DecorationPosition.foreground,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(widget.focusRadius + 2),
        border: _focused
            ? Border.all(color: context.fg.blueSolid, width: 2)
            : null,
      ),
      child: child,
    );
    if (widget.tooltip != null) {
      child = Tooltip(
        message: widget.tooltip!,
        // Said once: as the label when there is no other.
        excludeFromSemantics: widget.label == null,
        child: child,
      );
    }
    return child;
  }
}

/// Focus rings show while the keyboard is used and go at a click or a touch, as elsewhere
/// on a desktop: Flutter's own rule keeps them after a click with the mouse.
class KeyboardFocus {
  KeyboardFocus._();

  static bool _watching = false;

  /// Starts following the pointer and the keyboard.
  static void watch() {
    if (!_watching) {
      _watching = true;
      GestureBinding.instance.pointerRouter.addGlobalRoute(_pointer);
    }
    // Added again each time: the keyboard's handlers can be cleared (between tests).
    HardwareKeyboard.instance
      ..removeHandler(_key)
      ..addHandler(_key);
  }

  static void _pointer(PointerEvent event) {
    if (event is PointerDownEvent) _rings(false);
  }

  static bool _key(KeyEvent event) {
    // A modifier alone, as when a click comes with Ctrl or Shift held, is not keyboard use.
    if (event is KeyDownEvent && !_modifiers.contains(event.logicalKey)) {
      _rings(true);
    }
    return false;
  }

  static final _modifiers = {
    LogicalKeyboardKey.shiftLeft,
    LogicalKeyboardKey.shiftRight,
    LogicalKeyboardKey.controlLeft,
    LogicalKeyboardKey.controlRight,
    LogicalKeyboardKey.altLeft,
    LogicalKeyboardKey.altRight,
    LogicalKeyboardKey.metaLeft,
    LogicalKeyboardKey.metaRight,
    LogicalKeyboardKey.capsLock,
    LogicalKeyboardKey.fn,
  };

  static void _rings(bool shown) {
    final strategy = shown
        ? FocusHighlightStrategy.alwaysTraditional
        : FocusHighlightStrategy.alwaysTouch;
    final focus = FocusManager.instance;
    if (focus.highlightStrategy != strategy) focus.highlightStrategy = strategy;
  }
}

/// The springy curve of the controls: a little past the target, then back; at the
/// target at once for people who asked for less motion ([MotionCurve]).
const spring = MotionCurve(Curves.easeOutBack);
const springTime = Duration(milliseconds: 320);

/// Grows a little under the pointer and shrinks while pressed, with a spring. Draws
/// [child], or [builder] with the hover state.
class Springy extends StatefulWidget {
  const Springy({
    super.key,
    this.child,
    this.builder,
    this.onTap,
    this.tooltip,
    this.label,
    this.selected,
    this.expanded,
    this.hoverScale = 1.06,
    this.pressScale = .94,
    this.focusRadius = 999,
  }) : assert(child != null || builder != null);

  final Widget? child;
  final Widget Function(BuildContext context, bool hovered)? builder;
  final VoidCallback? onTap;
  final String? tooltip;
  final String? label;

  /// See [Pressable.selected].
  final bool? selected;

  /// See [Pressable.expanded].
  final bool? expanded;
  final double hoverScale;
  final double pressScale;
  final double focusRadius;

  @override
  State<Springy> createState() => _SpringyState();
}

class _SpringyState extends State<Springy> {
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final enabled = widget.onTap != null;
    return Listener(
      onPointerDown: enabled ? (_) => setState(() => _pressed = true) : null,
      onPointerUp: (_) => setState(() => _pressed = false),
      onPointerCancel: (_) => setState(() => _pressed = false),
      child: Pressable(
        onTap: widget.onTap,
        tooltip: widget.tooltip,
        label: widget.label,
        selected: widget.selected,
        expanded: widget.expanded,
        focusRadius: widget.focusRadius,
        builder: (context, hovered) => AnimatedScale(
          scale: _pressed && enabled
              ? widget.pressScale
              : hovered
              ? widget.hoverScale
              : 1,
          duration: springTime,
          curve: spring,
          child: widget.builder?.call(context, hovered) ?? widget.child,
        ),
      ),
    );
  }
}

enum Kind { primary, secondary, ghost, danger }

/// Whether the system asks for text larger than usual: some rows then lay out
/// otherwise (one part under another) so the words fit, and stay as drawn at the usual
/// size.
bool largeText(BuildContext context) =>
    MediaQuery.textScalerOf(context).scale(1) > 1;

/// The height of a control that holds a line of [fontSize] text: [height], or taller when
/// the system's text size makes the line taller than it, so the words are never cut. At
/// the usual text size every control of the design stays as it is drawn.
double heightForText(BuildContext context, double height, double fontSize) =>
    math.max(
      height,
      MediaQuery.textScalerOf(context).scale(fontSize) * 1.2 + 4,
    );

/// A button of the design, in a pill: primary (solid), secondary (soft), ghost or danger
/// (red). It grows a little under the pointer and sinks when pressed; while [busy] it
/// shrinks to a spinning circle.
class FgButton extends StatelessWidget {
  const FgButton({
    super.key,
    this.text,
    this.icon,
    this.trailing,
    this.onTap,
    this.kind = Kind.secondary,
    this.height = 36,
    this.padding,
    this.radius = 999,
    this.fontSize = 14,
    this.tooltip,
    this.expand = false,
    this.busy = false,
    this.iconColor,
    this.open,
  });

  final String? text;
  final Ic? icon;
  final Ic? trailing;
  final VoidCallback? onTap;
  final Kind kind;
  final double height;
  final EdgeInsets? padding;
  final double radius;
  final double fontSize;
  final String? tooltip;
  final bool expand;
  final bool busy;
  final Color? iconColor;

  /// Whether the menu it opens is open: drawn as under the pointer meanwhile, and said to
  /// screen readers. Null for a button that opens nothing.
  final bool? open;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final enabled = onTap != null && !busy;
    final (bg, fgColor, border) = switch (kind) {
      Kind.primary => (c.pri, c.onPri, null),
      Kind.secondary => (c.hover, c.fg, c.border),
      Kind.ghost => (Colors.transparent, c.fg2, null),
      Kind.danger => (c.redSolid, Colors.white, null),
    };
    final square = text == null;
    final circle = square || (busy && !expand);
    final height = square
        ? this.height
        : heightForText(context, this.height, fontSize);
    return Opacity(
      opacity: onTap == null ? .45 : 1,
      child: Springy(
        onTap: enabled ? onTap : null,
        tooltip: tooltip,
        label: text ?? tooltip,
        expanded: open,
        focusRadius: radius,
        hoverScale: square ? 1.08 : 1.03,
        pressScale: square ? .92 : .96,
        builder: (context, hovered) {
          var fill = bg;
          if (hovered || (open ?? false)) {
            fill = switch (kind) {
              Kind.primary || Kind.danger => bg.withValues(alpha: .88),
              Kind.secondary => Color.lerp(bg, c.fg, .06)!,
              Kind.ghost => c.hover,
            };
          }
          final content = <Widget>[
            if (busy)
              Spinner(size: 13, color: fgColor)
            else ...[
              if (icon != null)
                FgIcon(
                  icon!,
                  size: square ? 16 : 15,
                  color: iconColor ?? fgColor,
                ),
              // Said once, as the button's label.
              if (text != null)
                Flexible(
                  child: ExcludeSemantics(
                    child: Text(
                      text!,
                      style: tx(
                        fgColor,
                        size: fontSize,
                        weight: w500,
                        height: 1.2,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ),
              if (trailing != null) FgIcon(trailing!, size: 15, color: fgColor),
            ],
          ];
          // The width changes through AnimatedSize: AnimatedContainer cannot tween
          // between a free width and a fixed one.
          return AnimatedSize(
            duration: springTime,
            curve: spring,
            child: SizedBox(
              width: circle ? height : null,
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                height: height,
                padding: circle
                    ? null
                    : padding ?? const EdgeInsets.symmetric(horizontal: 16),
                decoration: BoxDecoration(
                  color: fill,
                  borderRadius: BorderRadius.circular(radius),
                  border: border == null ? null : Border.all(color: border),
                ),
                child: Row(
                  mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    for (var i = 0; i < content.length; i++) ...[
                      if (i > 0) const SizedBox(width: 8),
                      content[i],
                    ],
                  ],
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// A round avatar: a preset picture or a photo, with a [ring] around it and [ringGap]
/// between.
class Avatar extends StatelessWidget {
  const Avatar({
    super.key,
    required this.avatar,
    this.size = 24,
    this.ring,
    this.ringGap,
    this.dim = false,
    this.id,
    this.moving,
  });

  Avatar.preset(int preset, {Key? key, double size = 24})
    : this(
        key: key,
        avatar: UiAvatar(preset: preset),
        size: size,
      );

  final UiAvatar avatar;
  final double size;
  final Color? ring;
  final Color? ringGap;
  final bool dim;

  /// Whose avatar it is: given, a large one plays the person's animation (moving.dart).
  final String? id;

  /// An animation to play in place of the photo, known here (a picture just picked).
  final Uint8List? moving;

  @override
  Widget build(BuildContext context) {
    final preset = avatarPresets[avatar.preset % avatarPresets.length];
    final photo = avatar.photo;
    final still = RoundPicture(
      source: photo == null ? AssetImage(preset.image) : SafeMemoryImage(photo),
      size: size,
      fill: preset.color,
      ring: ring,
      ringGap: ringGap ?? context.fg.bg1,
      gapWidth: size < 24 ? 1.5 : 2,
    );
    Widget play(Uint8List? moving) => moving == null
        ? still
        : Stack(
            clipBehavior: Clip.none,
            children: [
              still,
              SizedBox.square(
                dimension: size,
                child: MovingPicture(
                  key: ValueKey(moving),
                  bytes: moving,
                  maxPixels: maxMovingPhotoPixels,
                  round: true,
                  under: preset.color,
                  still: const SizedBox.shrink(),
                ),
              ),
            ],
          );
    final id = this.id;
    Widget child;
    if (photo == null || size < minMovingAvatar) {
      child = still;
    } else if (moving != null || id == null) {
      child = play(moving);
    } else {
      child = ListenableBuilder(
        listenable: movingBook,
        builder: (context, _) => play(movingBook.photo(id, photo)),
      );
    }
    if (dim) child = Opacity(opacity: .45, child: child);
    return child;
  }
}

/// A picture in a circle, or in a square with round corners ([corner] of its side), cut
/// with a smooth edge in its own pixels (round.dart), so it looks the same with every
/// renderer; [fill] shows until it is there and under its see-through parts. A [ring] goes
/// around a circle, with [ringGap] between, outside the box like a shadow.
class RoundPicture extends StatefulWidget {
  const RoundPicture({
    super.key,
    required this.source,
    required this.size,
    this.corner = .5,
    this.fill,
    this.ring,
    this.ringGap = Colors.transparent,
    this.ringWidth = 1.5,
    this.gapWidth = 2,
  });

  /// The whole picture: decoded here at about twice the size drawn.
  final ImageProvider source;
  final double size;
  final double corner;
  final Color? fill;
  final Color? ring;
  final Color ringGap;
  final double ringWidth;
  final double gapWidth;

  @override
  State<RoundPicture> createState() => _RoundPictureState();
}

class _RoundPictureState extends State<RoundPicture> {
  ImageStream? _stream;
  ImageStreamListener? _listener;
  ui.Image? _image;

  /// Decoded at about twice the drawn size in physical pixels, so the edge is filtered
  /// from more of them.
  ImageProvider _provider(BuildContext context) {
    final px = twiceAsLarge(
      widget.size * MediaQuery.devicePixelRatioOf(context),
    );
    return RoundImage(
      ResizeImage(widget.source, width: px, policy: ResizeImagePolicy.fit),
      corner: widget.corner,
      under: widget.fill,
    );
  }

  void _resolve() {
    final stream = _provider(context)
        .resolve(createLocalImageConfiguration(context));
    if (stream.key == _stream?.key) return;
    _stop();
    // The picture shown stays until the new one is there. Only the first frame of one that
    // moves: rounding each frame again would cost every time, and nothing would stop it
    // (a GIF moves through [MovingPicture]).
    // Each listener is handed a handle of its own on the picture: this one keeps it while
    // it draws it, and lets go of it when it draws another or goes, rather than leaving
    // the decoded pixels to whenever the garbage collector comes.
    _listener = ImageStreamListener(
      (info, _) {
        if (_listener != null) _stream?.removeListener(_listener!);
        _listener = null;
        if (!mounted) {
          info.dispose();
          return;
        }
        _show(info.image);
      },
      onError: (_, _) {
        if (mounted) _show(null);
      },
    );
    _stream = stream..addListener(_listener!);
  }

  void _stop() {
    if (_listener != null) _stream?.removeListener(_listener!);
    _stream = null;
    _listener = null;
  }

  /// Draws [image] from now on, letting go of the one drawn before.
  void _show(ui.Image? image) {
    final before = _image;
    setState(() => _image = image);
    before?.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _resolve();
  }

  @override
  void didUpdateWidget(RoundPicture old) {
    super.didUpdateWidget(old);
    if (old.source != widget.source ||
        old.size != widget.size ||
        old.corner != widget.corner ||
        old.fill != widget.fill) {
      _resolve();
    }
  }

  @override
  void dispose() {
    _stop();
    _image?.dispose();
    _image = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => CustomPaint(
    size: Size.square(widget.size),
    painter: _RoundPainter(
      image: _image,
      scale: MediaQuery.devicePixelRatioOf(context),
      fill: widget.corner == .5 ? widget.fill : null,
      ring: widget.ring,
      ringGap: widget.ringGap,
      ringWidth: widget.ringWidth,
      gapWidth: widget.gapWidth,
    ),
  );
}

class _RoundPainter extends CustomPainter {
  _RoundPainter({
    required this.image,
    required this.scale,
    required this.fill,
    required this.ring,
    required this.ringGap,
    required this.ringWidth,
    required this.gapWidth,
  }) : super(repaint: Discs.ready);

  final ui.Image? image;
  final double scale;
  final Color? fill;
  final Color? ring;
  final Color ringGap;
  final double ringWidth;
  final double gapWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final box = Offset.zero & size;
    // Outside the box, like a shadow: the ring, then the gap, then the picture.
    if (ring != null) {
      Discs.paint(canvas, box.inflate(gapWidth + ringWidth), ring!, scale);
      Discs.paint(canvas, box.inflate(gapWidth), ringGap, scale);
    }
    final image = this.image;
    if (image != null) {
      paintRound(canvas, box, image);
    } else if (fill != null) {
      Discs.paint(canvas, box, fill!, scale);
    }
  }

  @override
  bool shouldRepaint(_RoundPainter old) =>
      old.image != image ||
      old.scale != scale ||
      old.fill != fill ||
      old.ring != ring ||
      old.ringGap != ringGap ||
      old.ringWidth != ringWidth ||
      old.gapWidth != gapWidth;
}

/// Animations that run for as long as something is shown (someone talking) step at this
/// rate rather than at every frame: a slow pulse looks the same and the window is drawn far
/// less often.
const slowFrame = Duration(milliseconds: 66);

/// The clock every [SlowPulse] follows: one timer for all of them, so the window is drawn
/// once per step however many pulse, and only while one is on screen and the window can
/// be seen.
class _PulseClock {
  /// Milliseconds the clock has run, a step at a time.
  static final now = ValueNotifier<int>(0);
  static Timer? _timer;
  static int _pulses = 0;
  static bool _watching = false;

  static void add() {
    _pulses++;
    if (!_watching) {
      _watching = true;
      windowShown.addListener(_run);
    }
    _run();
  }

  static void remove() {
    _pulses--;
    _run();
  }

  static void _run() {
    final want = _pulses > 0 && windowShown.value;
    if (want && _timer == null) {
      _timer = Timer.periodic(
        slowFrame,
        (_) => now.value += slowFrame.inMilliseconds,
      );
    } else if (!want) {
      _timer?.cancel();
      _timer = null;
    }
  }
}

/// A phase from 0 to 1 that advances at [slowFrame] over [period], in step with every
/// other pulse; the child is drawn in its own layer so the rest of the window is not
/// repainted with it.
class SlowPulse extends StatefulWidget {
  const SlowPulse({super.key, required this.period, required this.builder});
  final Duration period;
  final Widget Function(BuildContext context, double phase) builder;

  @override
  State<SlowPulse> createState() => _SlowPulseState();
}

class _SlowPulseState extends State<SlowPulse> {
  @override
  void initState() {
    super.initState();
    _PulseClock.add();
  }

  @override
  void dispose() {
    _PulseClock.remove();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => RepaintBoundary(
    child: ValueListenableBuilder<int>(
      valueListenable: _PulseClock.now,
      builder: (context, now, _) {
        final period = widget.period.inMilliseconds;
        return widget.builder(context, (now % period) / period);
      },
    ),
  );
}

/// A plain dot.
class Dot extends StatelessWidget {
  const Dot(this.color, {super.key, this.size = 6});
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) => Container(
    width: size,
    height: size,
    decoration: BoxDecoration(shape: BoxShape.circle, color: color),
  );
}

/// A spinning ring with a gap.
class Spinner extends StatefulWidget {
  const Spinner({super.key, this.size = 14, this.color, this.stroke = 2});
  final double size;
  final Color? color;
  final double stroke;

  @override
  State<Spinner> createState() => _SpinnerState();
}

class _SpinnerState extends State<Spinner> with SingleTickerProviderStateMixin {
  late final _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 800),
  )..repeat();

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => RepaintBoundary(
    child: RotationTransition(
      turns: _c,
      child: CustomPaint(
        size: Size.square(widget.size),
        painter: _RingPainter(widget.color ?? context.fg.blue, widget.stroke),
      ),
    ),
  );
}

class _RingPainter extends CustomPainter {
  _RingPainter(this.color, this.stroke);
  final Color color;
  final double stroke;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawArc(
      rect.deflate(stroke / 2),
      math.pi * .25,
      math.pi * 1.5,
      false,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = stroke,
    );
  }

  @override
  bool shouldRepaint(_RingPainter old) => old.color != color;
}

/// A shimmering placeholder.
class Skeleton extends StatefulWidget {
  const Skeleton({
    super.key,
    this.width,
    this.height,
    this.radius = 4,
    this.child,
  });
  final double? width;
  final double? height;
  final double radius;
  final Widget? child;

  @override
  State<Skeleton> createState() => _SkeletonState();
}

class _SkeletonState extends State<Skeleton>
    with SingleTickerProviderStateMixin {
  late final _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1400),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // With less motion, the shimmer holds still.
    if (lessMotion(context)) {
      _c.stop();
    } else if (!_c.isAnimating) {
      _c.repeat();
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return RepaintBoundary(
      child: AnimatedBuilder(
        animation: _c,
        builder: (context, child) {
          final shift = 1.5 - 3 * _c.value;
          return Container(
            width: widget.width,
            height: widget.height,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(widget.radius),
              gradient: LinearGradient(
                begin: Alignment(shift - 1, 0),
                end: Alignment(shift + 1, 0),
                colors: [c.hover, c.border, c.hover],
                stops: const [.25, .37, .63],
              ),
            ),
            child: child,
          );
        },
        child: widget.child,
      ),
    );
  }
}

/// A rounded pill with a soft background.
class Pill extends StatelessWidget {
  const Pill({
    super.key,
    required this.text,
    required this.color,
    required this.background,
    this.leading,
    this.height = 22,
    this.monoTail,
  });

  final String text;
  final Color color;
  final Color background;
  final Widget? leading;
  final double height;
  final String? monoTail;

  @override
  Widget build(BuildContext context) => Container(
    height: heightForText(context, height, 12),
    padding: const EdgeInsets.symmetric(horizontal: 8),
    decoration: BoxDecoration(
      color: background,
      borderRadius: BorderRadius.circular(999),
    ),
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (leading != null) ...[leading!, const SizedBox(width: 6)],
        Text(text, style: tx(color, size: 12, weight: w500, height: 1.2)),
        if (monoTail != null)
          Text(
            monoTail!,
            style: tx(
              color,
              size: 12,
              weight: w500,
              height: 1.2,
              monospace: true,
            ),
          ),
      ],
    ),
  );
}

/// A count badge.
class CountBadge extends StatelessWidget {
  const CountBadge(this.count, {super.key, this.color, this.background});
  final int count;
  final Color? color;
  final Color? background;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final height = heightForText(context, 18, 11);
    return Container(
      height: height,
      constraints: BoxConstraints(minWidth: height),
      padding: const EdgeInsets.symmetric(horizontal: 6),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: background ?? c.blueSoft,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        '$count',
        style: tx(color ?? c.blue, size: 11, weight: w600, height: 1.2),
      ),
    );
  }
}

/// A layer over the window that holds the keyboard and the screen reader while it is
/// there, as a dialog does: the focus goes into it as it opens, even from the control of
/// the page that opened it, Tab goes round it and not the page behind, the screen reader
/// reads it and not the page behind, and the focus goes back to where it was as it
/// closes, or to the layer under it.
///
/// A [polite] layer, one that comes by itself (a call ringing), does not take the focus
/// from a field being written in or from a dialog over the window: F6 ([focusTop]) goes
/// to it then.
class FocusLayer extends StatefulWidget {
  const FocusLayer({super.key, required this.child, this.polite = false});
  final Widget child;
  final bool polite;

  /// Gives the focus to the layer on top, if one is open; whether one was.
  static bool focusTop() {
    final top = _FocusLayerState._open.lastOrNull;
    if (top == null) return false;
    top._scope.requestFocus();
    return true;
  }

  @override
  State<FocusLayer> createState() => _FocusLayerState();
}

class _FocusLayerState extends State<FocusLayer> {
  /// The layers open now, the last on top.
  static final _open = <_FocusLayerState>[];

  final _scope = FocusScopeNode(debugLabel: 'FocusLayer');

  /// What had the focus before the layer opened, to give it back.
  FocusNode? _before;

  /// Whether the focus was in the layer when last seen: a layer that never had it does
  /// not move it as it goes.
  bool _held = false;

  @override
  void initState() {
    super.initState();
    _open.add(this);
    _before = FocusManager.instance.primaryFocus;
    _scope.addListener(_seen);
    // Once the layer is built, rather than only when nothing has the focus as
    // autofocus does: a layer opened with Enter would leave it on the button behind.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _scope.hasFocus) return;
      if (widget.polite && _busy(context)) return;
      _scope.requestFocus();
    });
  }

  void _seen() {
    if (_scope.hasFocus) _held = true;
  }

  /// Whether the focus is where a layer that comes by itself should leave it: in a field
  /// being written in, or in a dialog over the window.
  static bool _busy(BuildContext context) {
    final focus = FocusManager.instance.primaryFocus;
    final there = focus?.context;
    if (focus == null || there == null || !there.mounted) return false;
    if (there.widget is EditableText ||
        there.findAncestorWidgetOfExactType<EditableText>() != null) {
      return true;
    }
    final route = ModalRoute.of(context);
    return route != null && !route.isCurrent;
  }

  /// Whether [node] is in this layer.
  bool _holds(FocusNode node) =>
      node == _scope || node.ancestors.contains(_scope);

  @override
  void dispose() {
    _open.remove(this);
    _scope.removeListener(_seen);
    final before = _before;
    // A layer opened from this one goes back, once it closes, to where this one would.
    for (final other in _open) {
      if (other._before case final theirs? when _holds(theirs)) {
        other._before = before;
      }
    }
    if (_held) {
      // After the frame, once the layer's own controls are gone: to where the focus
      // was, if it is still there and not under another layer; else into the layer on
      // top, unless the focus is in it already.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        final top = _open.lastOrNull;
        final now = FocusManager.instance.primaryFocus;
        if (top != null &&
            now != null &&
            top._holds(now) &&
            now != top._scope) {
          return;
        }
        final back = before != null && (before.context?.mounted ?? false)
            ? before
            : null;
        if (back != null && (top == null || top._holds(back))) {
          back.requestFocus();
        } else {
          top?._scope.requestFocus();
        }
      });
    }
    _scope.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => BlockSemantics(
    child: FocusScope(node: _scope, child: widget.child),
  );
}

/// What lies behind a layer over the window (a floating window, a popover, a call
/// ringing): a click on it calls [onTap], and a right click too where [rightClick], and
/// it is tinted [color] if given. Screen readers skip it: the layer's own Close and Esc
/// do the same for them.
class Backdrop extends StatelessWidget {
  const Backdrop({
    super.key,
    required this.onTap,
    this.color,
    this.rightClick = false,
  });
  final VoidCallback onTap;
  final Color? color;
  final bool rightClick;

  @override
  Widget build(BuildContext context) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    excludeFromSemantics: true,
    onTap: onTap,
    onSecondaryTap: rightClick ? onTap : null,
    child: color == null ? null : ColoredBox(color: color!),
  );
}

/// A card: border, radius 12, raised background.
class Panel extends StatelessWidget {
  const Panel({
    super.key,
    required this.child,
    this.padding,
    this.dashed = false,
  });
  final Widget child;
  final EdgeInsets? padding;
  final bool dashed;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      // Inside the outline, as when it was drawn under what is inside.
      padding: (padding ?? EdgeInsets.zero).add(const EdgeInsets.all(1)),
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(12),
      ),
      // Over what is inside, which would cover it at the rounded corners.
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: child,
    );
  }
}

/// A rectangle with a dashed outline.
class Dashed extends StatelessWidget {
  const Dashed({
    super.key,
    required this.child,
    this.radius = 8,
    this.color,
    this.width = 1,
  });
  final Widget child;
  final double radius;
  final Color? color;

  /// How thick the line is; its dashes grow with it.
  final double width;

  @override
  Widget build(BuildContext context) => CustomPaint(
    foregroundPainter: _DashPainter(
      color ?? context.fg.borderStrong,
      radius,
      width,
    ),
    child: child,
  );
}

class _DashPainter extends CustomPainter {
  _DashPainter(this.color, this.radius, this.width);
  final Color color;
  final double radius;
  final double width;

  @override
  void paint(Canvas canvas, Size size) {
    final path = Path()
      ..addRRect(
        RRect.fromRectAndRadius(
          (Offset.zero & size).deflate(width / 2),
          Radius.circular(radius),
        ),
      );
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = width;
    for (final metric in path.computeMetrics()) {
      var d = 0.0;
      while (d < metric.length) {
        canvas.drawPath(metric.extractPath(d, d + 3 * width), paint);
        d += 6 * width;
      }
    }
  }

  @override
  bool shouldRepaint(_DashPainter old) =>
      old.color != color || old.radius != radius || old.width != width;
}

/// A text input: 1 px border, blue ring when focused.
/// Keeps a text to at most [max] characters as the core counts them (Unicode scalar
/// values, so an emoji made of several counts as several), cutting between whole
/// symbols.
class CharacterLimit extends TextInputFormatter {
  const CharacterLimit(this.max);
  final int max;

  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) {
    if (newValue.text.runes.length <= max) return newValue;
    final kept = StringBuffer();
    var count = 0;
    for (final symbol in newValue.text.characters) {
      final n = symbol.runes.length;
      if (count + n > max) break;
      kept.write(symbol);
      count += n;
    }
    final text = kept.toString();
    return TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }
}

/// Under a name's field: why what is typed there cannot be a name (nothing in it shows, or
/// it has hidden characters); nothing when it can.
class NameNote extends StatelessWidget {
  const NameNote(this.text, {super.key});
  final String text;

  @override
  Widget build(BuildContext context) {
    final problem = nameProblem(text);
    if (problem == null) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 6),
      child: Text(problem, style: tx(context.fg.red, size: 12)),
    );
  }
}

class FgField extends StatefulWidget {
  const FgField({
    super.key,
    this.controller,
    this.placeholder,
    this.height = 40,
    this.leading,
    this.trailing,
    this.monospace = false,
    this.fontSize = 14,
    this.onSubmitted,
    this.onChanged,
    this.autofocus = false,
    this.focusNode,
    this.borderless = false,
    this.strong = false,
    this.maxLength,
    this.background,
    this.label,
    this.radius = 8,
  });

  final TextEditingController? controller;
  final String? placeholder;
  final double height;
  final Widget? leading;
  final Widget? trailing;
  final bool monospace;
  final double fontSize;
  final ValueChanged<String>? onSubmitted;
  final ValueChanged<String>? onChanged;
  final bool autofocus;
  final FocusNode? focusNode;
  final bool borderless;
  final bool strong;
  final int? maxLength;
  final Color? background;
  final String? label;
  final double radius;

  @override
  State<FgField> createState() => _FgFieldState();
}

class _FgFieldState extends State<FgField> {
  late final FocusNode _focus = widget.focusNode ?? FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(_changed);
  }

  void _changed() => setState(() {});

  @override
  void dispose() {
    _focus.removeListener(_changed);
    if (widget.focusNode == null) _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final focused = _focus.hasFocus && !widget.borderless;
    final repeated =
        widget.placeholder != null && widget.placeholder == widget.label;
    final hintStyle = tx(
      c.fg3,
      size: widget.fontSize,
      monospace: widget.monospace,
      height: 1.3,
    );
    return AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      height: widget.height,
      decoration: BoxDecoration(
        color: widget.background ?? (widget.borderless ? null : c.bg1),
        borderRadius: BorderRadius.circular(widget.radius),
        // Focused, a stronger border; no halo around it.
        border: widget.borderless
            ? null
            // Focused, the strongest; a field that asks to be seen ([strong]), stronger
            // than the rest.
            : Border.all(
                color: focused
                    ? c.fg2
                    : widget.strong
                    ? c.fg3
                    : c.edge,
              ),
      ),
      child: Row(
        children: [
          if (widget.leading != null)
            Padding(
              padding: const EdgeInsets.only(left: 10, right: 2),
              child: widget.leading,
            ),
          Expanded(
            // The label goes on the field's own node: one marked as a text field here
            // would be a second field around it.
            child: Semantics(
              label: widget.label,
              child: TextField(
                controller: widget.controller,
                focusNode: _focus,
                autofocus: widget.autofocus,
                onSubmitted: widget.onSubmitted,
                onChanged: widget.onChanged,
                inputFormatters: [
                  if (widget.maxLength case final max?) CharacterLimit(max),
                ],
                cursorColor: c.fg,
                cursorWidth: 1.5,
                style: tx(
                  c.fg,
                  size: widget.fontSize,
                  monospace: widget.monospace,
                  height: 1.3,
                ),
                decoration: InputDecoration(
                  isCollapsed: true,
                  border: InputBorder.none,
                  counterText: '',
                  // A placeholder that only repeats the field's name is not said
                  // twice.
                  hintText: repeated ? null : widget.placeholder,
                  hint: repeated
                      ? ExcludeSemantics(
                          child: Text(
                            widget.placeholder!,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: hintStyle,
                          ),
                        )
                      : null,
                  hintStyle: hintStyle,
                  contentPadding: EdgeInsets.symmetric(
                    horizontal: widget.leading == null ? 12 : 6,
                  ),
                ),
              ),
            ),
          ),
          if (widget.trailing != null)
            Padding(
              padding: const EdgeInsets.only(right: 6),
              child: widget.trailing,
            ),
        ],
      ),
    );
  }
}

/// Tabs in a pill: the chosen one sits on a raised pill that slides to it with a spring.
/// Each tab is a label, an optional count and the count's colour.
class Segmented extends StatefulWidget {
  const Segmented({
    super.key,
    required this.tabs,
    required this.selected,
    required this.onSelect,
    this.height = 34,
    this.icons,
    this.leading,
    this.wrap = false,
  });

  final List<(String, int?, Color?)> tabs;
  final int selected;
  final ValueChanged<int> onSelect;
  final double height;

  /// An icon before each label, if given.
  final List<Ic>? icons;

  /// Something else before each label, such as a status dot.
  final List<Widget>? leading;

  /// Tabs that do not fit on one line go on to the next, whole, rather than sharing the
  /// width and cutting their words short.
  final bool wrap;

  @override
  State<Segmented> createState() => _SegmentedState();
}

class _SegmentedState extends State<Segmented> {
  final _row = GlobalKey();
  final _tabs = <GlobalKey>[];
  Rect? _thumb;

  /// Whether [_measure] runs after the frame being built: once per frame, however many
  /// builds it has.
  bool _measurePending = false;

  void _measure() {
    if (!mounted) return;
    final row = _row.currentContext?.findRenderObject() as RenderBox?;
    final selected = widget.selected;
    if (row == null || selected < 0 || selected >= _tabs.length) return;
    final tab =
        _tabs[selected].currentContext?.findRenderObject() as RenderBox?;
    if (tab == null || !tab.hasSize || !row.hasSize) return;
    final at = tab.localToGlobal(Offset.zero, ancestor: row);
    final rect = at & tab.size;
    if (rect != _thumb) setState(() => _thumb = rect);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    while (_tabs.length < widget.tabs.length) {
      _tabs.add(GlobalKey());
    }
    if (!_measurePending) {
      _measurePending = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _measurePending = false;
        _measure();
      });
    }
    final inner = heightForText(context, widget.height - 8, 13);
    final thumb = _thumb;
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: c.bg2,
        // Round at the ends of each line, also when the tabs take more than one.
        borderRadius: BorderRadius.circular(widget.wrap ? inner / 2 + 3 : 999),
        border: Border.all(color: c.border),
      ),
      child: Stack(
        children: [
          if (thumb != null)
            AnimatedPositioned(
              duration: springTime,
              curve: spring,
              left: thumb.left,
              top: thumb.top,
              width: thumb.width,
              height: thumb.height,
              child: Container(
                decoration: BoxDecoration(
                  color: c.raised,
                  borderRadius: BorderRadius.circular(999),
                  boxShadow: [
                    BoxShadow(
                      color: c.dark
                          ? Colors.white.withValues(alpha: .06)
                          : c.border,
                      spreadRadius: 1,
                    ),
                  ],
                ),
              ),
            ),
          // Where the width is limited and the labels are too long for it, the tabs share
          // it, longer labels more, and cut their words short.
          LayoutBuilder(
            builder: (context, box) => _line([
              for (var i = 0; i < widget.tabs.length; i++) ...[
                if (i > 0 && !widget.wrap) const SizedBox(width: 2),
                _tab(
                  box.hasBoundedWidth && !widget.wrap,
                  widget.tabs[i].$1.length,
                  Pressable(
                    key: _tabs[i],
                    onTap: () => widget.onSelect(i),
                    selected: i == widget.selected,
                    focusRadius: 999,
                    builder: (context, hovered) {
                      final on = i == widget.selected;
                      final color = on || hovered ? c.fg : c.fg2;
                      final (label, count, countColor) = widget.tabs[i];
                      return Container(
                        height: inner,
                        padding: const EdgeInsets.symmetric(horizontal: 12),
                        // Until the thumb is measured, the chosen tab draws its own.
                        decoration: thumb == null && on
                            ? BoxDecoration(
                                color: c.raised,
                                borderRadius: BorderRadius.circular(999),
                              )
                            : null,
                        // As wide as its words, not as the room it was given.
                        child: Align(
                          widthFactor: 1,
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              if (widget.icons != null) ...[
                                FgIcon(
                                  widget.icons![i],
                                  size: 14,
                                  color: color,
                                ),
                                const SizedBox(width: 6),
                              ],
                              if (widget.leading != null) ...[
                                widget.leading![i],
                                const SizedBox(width: 6),
                              ],
                              AnimatedDefaultTextStyle(
                                duration: const Duration(milliseconds: 160),
                                style: tx(
                                  color,
                                  size: 13,
                                  weight: on ? w500 : FontWeight.w400,
                                  height: 1.2,
                                ),
                                child: box.hasBoundedWidth && !widget.wrap
                                    ? Flexible(
                                        child: Text(
                                          label,
                                          maxLines: 1,
                                          softWrap: false,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      )
                                    : Text(label),
                              ),
                              if (count != null) ...[
                                const SizedBox(width: 6),
                                CountBadge(
                                  count,
                                  color: countColor ?? c.fg,
                                  background: on ? c.borderStrong : c.hover,
                                ),
                              ],
                            ],
                          ),
                        ),
                      );
                    },
                  ),
                ),
              ],
            ]),
          ),
        ],
      ),
    );
  }

  Widget _tab(bool bounded, int length, Widget tab) =>
      bounded ? Flexible(flex: length + 4, child: tab) : tab;

  Widget _line(List<Widget> tabs) => widget.wrap
      ? Wrap(key: _row, spacing: 2, runSpacing: 2, children: tabs)
      : Row(key: _row, mainAxisSize: MainAxisSize.min, children: tabs);
}

/// A bar of pill tabs along the top of a panel, over its bottom border.
class LineTabs extends StatelessWidget {
  const LineTabs({
    super.key,
    required this.tabs,
    required this.selected,
    required this.onSelect,
    this.height = 52,
    this.padding = const EdgeInsets.symmetric(horizontal: 16),
    this.center = false,
  });

  final List<(String, int?)> tabs;
  final int selected;
  final ValueChanged<int> onSelect;
  final double height;
  final EdgeInsets padding;

  /// Tabs in the middle of the bar instead of at its start.
  final bool center;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      height: height,
      padding: padding,
      alignment: center ? Alignment.center : Alignment.centerLeft,
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: c.border)),
      ),
      child: Segmented(
        tabs: [for (final (label, count) in tabs) (label, count, null)],
        selected: selected,
        onSelect: onSelect,
      ),
    );
  }
}

/// A round on/off switch; the knob stretches while it travels.
class FgSwitch extends StatelessWidget {
  const FgSwitch({super.key, required this.on, this.small = false});
  final bool on;
  final bool small;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final width = small ? 32.0 : 38.0;
    final height = small ? 18.0 : 22.0;
    final knob = height - 4;
    // On or off to a screen reader too, on the control it is part of.
    return Semantics(
      toggled: on,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        width: width,
        height: height,
        decoration: BoxDecoration(
          color: on ? const Color(0xFF2EAF6B) : c.edge,
          borderRadius: BorderRadius.circular(999),
        ),
        child: Stack(
          children: [
            AnimatedPositioned(
              duration: springTime,
              curve: spring,
              left: on ? width - knob - 2 : 2,
              top: 2,
              width: knob,
              height: knob,
              child: Container(
                decoration: BoxDecoration(
                  color: on ? Colors.white : c.switchKnobOff,
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: Colors.black.withValues(alpha: .12),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A button that copies [text]; it turns into a green "Copied" for a moment instead of a
/// message elsewhere.
class CopyButton extends StatefulWidget {
  const CopyButton({
    super.key,
    required this.text,
    required this.what,
    this.label,
    this.height = 32,
    this.onCopied,
  });

  final String text;

  /// "Link", "Code", "QR" or "Key", in English: which secret it copies, named in the tooltip in
  /// the language in use.
  final String what;

  /// Shown beside the icon; without it the button is round.
  final String? label;
  final double height;
  final VoidCallback? onCopied;

  @override
  State<CopyButton> createState() => _CopyButtonState();
}

class _CopyButtonState extends State<CopyButton> {
  bool _copied = false;
  Timer? _back;

  @override
  void dispose() {
    _back?.cancel();
    super.dispose();
  }

  void _copy() {
    Clipboard.setData(ClipboardData(text: widget.text));
    widget.onCopied?.call();
    _back?.cancel();
    setState(() => _copied = true);
    _back = Timer(const Duration(milliseconds: 1500), () {
      if (mounted) setState(() => _copied = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    const green = greenSolid;
    final label = _copied ? l10n.commonCopied : widget.label;
    return Springy(
      onTap: _copy,
      tooltip: _copied ? null : l10n.widgetsCopyWhat(widget.what.toLowerCase()),
      label: l10n.widgetsCopyWhat(widget.what.toLowerCase()),
      hoverScale: 1.04,
      builder: (context, hovered) => AnimatedSize(
        duration: springTime,
        curve: spring,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          height: widget.height,
          constraints: BoxConstraints(minWidth: widget.height),
          padding: EdgeInsets.symmetric(horizontal: label == null ? 0 : 12),
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: _copied
                ? green
                : hovered
                ? Color.lerp(c.hover, c.fg, .06)
                : c.hover,
            borderRadius: BorderRadius.circular(999),
            border: _copied ? null : Border.all(color: c.border),
          ),
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 180),
            transitionBuilder: (child, animation) => FadeTransition(
              opacity: animation,
              child: ScaleTransition(
                scale: Tween(begin: .8, end: 1.0).animate(animation),
                child: child,
              ),
            ),
            child: Row(
              key: ValueKey(_copied),
              mainAxisSize: MainAxisSize.min,
              children: [
                FgIcon(
                  _copied ? Ic.check : Ic.copy,
                  size: 14,
                  color: _copied ? Colors.white : c.fg,
                ),
                if (label != null) ...[
                  const SizedBox(width: 7),
                  Text(
                    label,
                    style: tx(
                      _copied ? Colors.white : c.fg,
                      size: 13,
                      weight: w500,
                      height: 1.2,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A square checkbox.
class FgCheck extends StatelessWidget {
  const FgCheck(this.checked, {super.key, this.size = 16});
  final bool checked;
  final double size;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: checked ? c.pri : c.bg1,
        borderRadius: BorderRadius.circular(size / 4),
        border: checked ? null : Border.all(color: c.edge),
      ),
      child: checked
          ? FgIcon(Ic.check, size: size * 11 / 16, color: c.onPri)
          : null,
    );
  }
}

/// A square tile with a group's icon.
class GroupBadge extends StatelessWidget {
  const GroupBadge({
    super.key,
    required this.icon,
    this.picture,
    this.size = 22,
    this.radius = 6,
    this.iconSize = 12,
    this.selected = false,
    this.play = false,
  });

  final int icon;

  /// The group's own picture, shown instead of the icon.
  final Uint8List? picture;
  final double size;
  final double radius;
  final double iconSize;
  final bool selected;

  /// A picture that moves plays at any size and shape; otherwise only a large round one
  /// does, as a person's avatar.
  final bool play;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final picture = this.picture;
    if (picture != null) {
      return MovingGroupPicture(
        bytes: picture,
        size: size,
        radius: radius,
        play: play || (size >= minMovingAvatar && radius >= size / 2),
      );
    }
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: selected ? c.bg1 : c.bg2,
        borderRadius: BorderRadius.circular(radius),
        border: Border.all(color: selected ? c.borderStrong : c.border),
      ),
      child: FgIcon(
        groupIcon(icon, small: iconSize < 16),
        size: iconSize,
        color: c.fg,
      ),
    );
  }
}

/// A group's picture in a square with round corners of [radius] (a circle at half its
/// [size]): one that moves (a GIF or WebP of several frames) plays over its first frame
/// while [play], by the rules of every animation here (moving.dart).
class MovingGroupPicture extends StatelessWidget {
  const MovingGroupPicture({
    super.key,
    required this.bytes,
    required this.size,
    required this.radius,
    this.play = true,
  });

  final Uint8List bytes;
  final double size;
  final double radius;
  final bool play;

  @override
  Widget build(BuildContext context) {
    final still = RoundedPicture(bytes: bytes, size: size, radius: radius);
    if (!play || !isMovingKind(bytes)) return still;
    final round = radius >= size / 2;
    return Stack(
      clipBehavior: Clip.none,
      children: [
        still,
        SizedBox.square(
          dimension: size,
          child: MovingPicture(
            key: ValueKey(bytes),
            bytes: bytes,
            maxPixels: maxMovingPhotoPixels,
            round: round,
            corner: round || size <= 0 ? null : radius / size,
            still: const SizedBox.shrink(),
          ),
        ),
      ],
    );
  }
}

/// A square picture with rounded corners, someone else's: [RoundPicture] of its bytes.
class RoundedPicture extends StatelessWidget {
  const RoundedPicture({
    super.key,
    required this.bytes,
    required this.size,
    required this.radius,
  });

  final Uint8List bytes;
  final double size;
  final double radius;

  @override
  Widget build(BuildContext context) => RoundPicture(
    source: SafeMemoryImage(bytes),
    size: size,
    corner: size > 0 ? (radius / size).clamp(0, .5) : .5,
  );
}

/// A chat message's text, cut after [shownLines] lines with "Show more" under it, and
/// whole with "Show less" once opened. A message may be 2000 line breaks and nothing else:
/// whole, it would be one bubble tens of thousands of pixels tall for everyone who reads
/// the chat. Only how it is drawn: what is sent, kept and copied is the whole text.
class LongText extends StatefulWidget {
  const LongText(this.text, {super.key, this.style});

  /// What it says, drawn as [Text.rich] draws it.
  final InlineSpan text;
  final TextStyle? style;

  /// The most lines it shows until "Show more".
  static const shownLines = 20;

  @override
  State<LongText> createState() => _LongTextState();
}

class _LongTextState extends State<LongText> {
  bool _open = false;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final whole = Text.rich(widget.text, style: widget.style);
      if (!box.hasBoundedWidth) return whole;
      final painter = TextPainter(
        text: TextSpan(
          style: DefaultTextStyle.of(context).style.merge(widget.style),
          children: [widget.text],
        ),
        textDirection: Directionality.of(context),
        textScaler: MediaQuery.textScalerOf(context),
        maxLines: LongText.shownLines,
      )..layout(maxWidth: box.maxWidth);
      final long = painter.didExceedMaxLines;
      painter.dispose();
      if (!long) return whole;
      final c = context.fg;
      final words = _open ? l10n.widgetsShowLess : l10n.widgetsShowMore;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _open
              ? whole
              : Text.rich(
                  widget.text,
                  style: widget.style,
                  maxLines: LongText.shownLines,
                  overflow: TextOverflow.fade,
                ),
          const SizedBox(height: 4),
          Pressable(
            onTap: () => setState(() => _open = !_open),
            expanded: _open,
            label: words,
            builder: (context, hovered) => Text(
              words,
              style: tx(
                hovered ? c.fg : c.fg2,
                size: 12.5,
                height: 1.45,
                decoration: TextDecoration.underline,
              ),
            ),
          ),
        ],
      );
    },
  );
}

/// A horizontal rule with text in the middle; text too long for one line wraps, with a
/// little of the rule left on each side.
class RuleText extends StatelessWidget {
  const RuleText(this.text, {super.key});
  final String text;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return LayoutBuilder(
      builder: (context, box) => Row(
        children: [
          Expanded(child: Container(height: 1, color: c.border)),
          ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: math.max(0, box.maxWidth - 48),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: Text(
                text,
                textAlign: TextAlign.center,
                style: tx(c.fg3, size: 12),
              ),
            ),
          ),
          Expanded(child: Container(height: 1, color: c.border)),
        ],
      ),
    );
  }
}

/// A modal card over a dimmed background, as the design draws dialogs.
Future<T?> showFgDialog<T>(
  BuildContext context, {
  required Widget Function(BuildContext) builder,
  double maxWidth = 480,
}) {
  final c = context.fg;
  return showGeneralDialog<T>(
    context: context,
    barrierDismissible: true,
    barrierLabel: l10n.commonClose,
    barrierColor: c.scrim,
    transitionDuration: const Duration(milliseconds: 260),
    pageBuilder: (context, _, _) => SafeArea(
      child: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 40),
          child: ConstrainedBox(
            constraints: BoxConstraints(maxWidth: maxWidth),
            child: Material(
              type: MaterialType.transparency,
              child: Container(
                clipBehavior: Clip.antiAlias,
                // Inside the outline, as when it was drawn under what is inside.
                padding: const EdgeInsets.all(1),
                decoration: BoxDecoration(
                  color: c.bg1,
                  borderRadius: BorderRadius.circular(16),
                ),
                // Over what is inside, which would cover it at the rounded corners.
                foregroundDecoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: c.border),
                ),
                child: builder(context),
              ),
            ),
          ),
        ),
      ),
    ),
    // Fades in and grows from a little smaller, with the controls' spring.
    transitionBuilder: (context, animation, _, child) => FadeTransition(
      opacity: animation,
      child: ScaleTransition(
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
}

/// The footer of a card or dialog: top border, sunken background. Buttons alone in it,
/// after a leading [Spacer], go in the middle.
class Footer extends StatelessWidget {
  const Footer({super.key, required this.children, this.padding});
  final List<Widget> children;
  final EdgeInsets? padding;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      padding:
          padding ?? const EdgeInsets.symmetric(horizontal: 24, vertical: 14),
      decoration: BoxDecoration(
        color: c.bg2,
        border: Border(top: BorderSide(color: c.border)),
      ),
      child: children.firstOrNull is Spacer
          ? Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: children.sublist(1),
            )
          : Row(children: children),
    );
  }
}

/// How long a turn of the mouse wheel glides.
const wheelGlide = Duration(milliseconds: 200);

/// A scroll controller whose mouse wheel glides each step rather than jumping it, as
/// browsers do; turns that come quickly add up and keep going smoothly. Small steps (a
/// touchpad, a wheel without notches) follow the hand as they come.
class GlideScrollController extends ScrollController {
  @override
  ScrollPosition createScrollPosition(
    ScrollPhysics physics,
    ScrollContext context,
    ScrollPosition? oldPosition,
  ) => _GlidePosition(
    physics: physics,
    context: context,
    initialPixels: initialScrollOffset,
    keepScrollOffset: keepScrollOffset,
    oldPosition: oldPosition,
    debugLabel: debugLabel,
  );
}

class _GlidePosition extends ScrollPositionWithSingleContext {
  _GlidePosition({
    required super.physics,
    required super.context,
    super.initialPixels,
    super.keepScrollOffset,
    super.oldPosition,
    super.debugLabel,
  });

  /// Where the glide under way ends.
  double? _target;

  @override
  void pointerScroll(double delta) {
    if (delta.abs() < 20) {
      _target = null;
      super.pointerScroll(delta);
      return;
    }
    // A turn during a glide goes on from where that one was going.
    final from = _target != null && activity is DrivenScrollActivity
        ? _target!
        : pixels;
    final to = (from + delta).clamp(minScrollExtent, maxScrollExtent);
    _target = to;
    if (to == pixels) return;
    animateTo(to, duration: wheelGlide, curve: Curves.easeOutCubic);
  }
}

/// A page that scrolls, with a scrollbar tied to it; the mouse wheel glides.
class PageScroll extends StatefulWidget {
  const PageScroll({super.key, required this.child});
  final Widget child;

  @override
  State<PageScroll> createState() => _PageScrollState();
}

class _PageScrollState extends State<PageScroll> {
  final _controller = GlideScrollController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scrollbar(
    controller: _controller,
    child: SingleChildScrollView(
      controller: _controller,
      // Drawn once and moved as it scrolls, rather than drawn again every frame.
      child: RepaintBoundary(child: widget.child),
    ),
  );
}

/// Three bars that bounce while someone talks.
class SpeakingBars extends StatefulWidget {
  const SpeakingBars({super.key, this.color, this.height = 10, this.lasts});
  final Color? color;
  final double height;

  /// How long the bars move before they hold still, for bars that only show what a voice
  /// looks like rather than someone talking now; null while they are shown. They hold
  /// still too while something else (a dialog) covers their page.
  final Duration? lasts;

  @override
  State<SpeakingBars> createState() => _SpeakingBarsState();
}

class _SpeakingBarsState extends State<SpeakingBars> {
  Timer? _stop;
  bool _still = false;

  @override
  void initState() {
    super.initState();
    if (widget.lasts case final lasts?) {
      _stop = Timer(lasts, () {
        if (mounted) setState(() => _still = true);
      });
    }
  }

  @override
  void dispose() {
    _stop?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = widget.color ?? context.fg.green;
    // Bars that would move for as long as the page shows redraw the window 15 times a
    // second for nothing: several processors' worth without a graphics card.
    final still =
        (widget.lasts != null &&
            (_still || !(ModalRoute.of(context)?.isCurrent ?? true))) ||
        lessMotion(context);
    if (still) return _bars(.42, color);
    return SlowPulse(
      period: const Duration(milliseconds: 1100),
      builder: (context, phase) => _bars(phase, color),
    );
  }

  Widget _bars(double phase, Color color) => Row(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.end,
    children: [
      for (var i = 0; i < 3; i++) ...[
        if (i > 0) const SizedBox(width: 2),
        Container(
          width: 2,
          height:
              widget.height *
              (.3 +
                  .7 *
                      (0.5 -
                          0.5 *
                              math.cos(2 * math.pi * ((phase - i * .16) % 1)))),
          decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(1),
          ),
        ),
      ],
    ],
  );
}

/// Something secret shown on screen (a link, a code, a QR, a recovery key): blurred until the eye button
/// shows it for a few seconds, so nothing leaks when the window is on a shared screen. The
/// copy button copies it without showing it.
class SecretBox extends StatefulWidget {
  const SecretBox({
    super.key,
    required this.text,
    required this.child,
    required this.what,
    this.onCopied,
    this.buttonsBelow = false,
    this.copy = true,
    this.stays = false,
  });

  /// What the copy button copies.
  final String text;

  /// How the secret is drawn.
  final Widget child;

  /// "Link", "Code", "QR" or "Key", in English: which secret it is, named in the buttons'
  /// tooltips in the language in use.
  final String what;
  final VoidCallback? onCopied;

  /// The buttons centred under the secret (for the QR) instead of beside it.
  final bool buttonsBelow;

  /// Whether it has a copy button. A recovery key has none: it is written down, not left
  /// on the clipboard.
  final bool copy;

  /// Whether it stays shown until the eye button hides it again, rather than for
  /// [shownFor]: a recovery key is written down by hand, which takes far longer.
  final bool stays;

  /// How long the secret stays shown after the eye button.
  static const shownFor = Duration(seconds: 10);

  @override
  State<SecretBox> createState() => _SecretBoxState();
}

class _SecretBoxState extends State<SecretBox> {
  bool _shown = false;
  Timer? _hide;

  @override
  void dispose() {
    _hide?.cancel();
    super.dispose();
  }

  void _toggle() {
    _hide?.cancel();
    setState(() => _shown = !_shown);
    if (_shown && !widget.stays) {
      _hide = Timer(SecretBox.shownFor, () {
        if (mounted) setState(() => _shown = false);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    Widget secret = widget.child;
    if (!_shown) {
      secret = ExcludeSemantics(
        child: ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: ImageFiltered(
            imageFilter: ImageFilter.blur(sigmaX: 6, sigmaY: 6),
            child: secret,
          ),
        ),
      );
    }
    final buttons = [
      FgButton(
        icon: _shown ? Ic.eyeOff : Ic.eye,
        height: 32,
        tooltip: _shown
            ? l10n.widgetsHide
            : l10n.widgetsShowWhat(widget.what.toLowerCase()),
        onTap: _toggle,
      ),
      if (widget.copy) ...[
        const SizedBox(width: 6),
        CopyButton(
          text: widget.text,
          what: widget.what,
          label: l10n.commonCopy,
          onCopied: widget.onCopied,
        ),
      ],
    ];
    if (widget.buttonsBelow) {
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          secret,
          const SizedBox(height: 12),
          Row(mainAxisSize: MainAxisSize.min, children: buttons),
        ],
      );
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Expanded(child: secret),
        const SizedBox(width: 10),
        ...buttons,
      ],
    );
  }
}

/// Rings going out from its child and fading, one after another: something rings.
class Rings extends StatefulWidget {
  const Rings({
    super.key,
    required this.child,
    required this.color,
    this.size = 260,
  });
  final Widget child;
  final Color color;

  /// Where the rings end, across.
  final double size;

  @override
  State<Rings> createState() => _RingsState();
}

class _RingsState extends State<Rings> with SingleTickerProviderStateMixin {
  late final _turn = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 2400),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // With less motion, the rings hold still where they are.
    if (lessMotion(context)) {
      _turn.stop();
    } else if (!_turn.isAnimating) {
      _turn.repeat();
    }
  }

  @override
  void dispose() {
    _turn.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => SizedBox(
    width: widget.size,
    height: widget.size,
    child: AnimatedBuilder(
      animation: _turn,
      builder: (context, child) => CustomPaint(
        painter: _RingsPainter(_turn.value, widget.color),
        child: child,
      ),
      child: Center(child: widget.child),
    ),
  );
}

class _RingsPainter extends CustomPainter {
  _RingsPainter(this.t, this.color);
  final double t;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final centre = size.center(Offset.zero);
    final outer = size.shortestSide / 2;
    final inner = outer * .5;
    for (var i = 0; i < 3; i++) {
      final p = (t + i / 3) % 1;
      final radius = inner + (outer - inner) * Curves.easeOut.transform(p);
      canvas.drawCircle(
        centre,
        radius,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2 - p
          ..color = color.withValues(alpha: .6 * (1 - p)),
      );
    }
  }

  @override
  bool shouldRepaint(_RingsPainter old) => old.t != t || old.color != color;
}

/// An entry in the column of a settings window: an icon and a word on a pill that shows
/// when it is hovered or chosen.
class NavItem extends StatelessWidget {
  const NavItem({
    super.key,
    required this.text,
    required this.icon,
    required this.onTap,
    this.selected = false,
    this.danger = false,
    this.iconColor,
  });
  final String text;
  final Ic icon;
  final VoidCallback onTap;
  final bool selected;
  final bool danger;

  /// The icon's own colour, whether chosen or not.
  final Color? iconColor;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Pressable(
      onTap: onTap,
      label: text,
      selected: selected,
      focusRadius: 999,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        // A pill, taller with rounded ends when the words take two lines.
        constraints: const BoxConstraints(minHeight: 34),
        margin: const EdgeInsets.only(bottom: 2),
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: selected || hovered ? c.hover : null,
          borderRadius: BorderRadius.circular(17),
          border: Border.all(color: selected ? c.border : Colors.transparent),
        ),
        child: Row(
          children: [
            FgIcon(
              icon,
              size: 15,
              color: iconColor ?? (danger ? c.red : (selected ? c.fg : c.fg2)),
            ),
            const SizedBox(width: 9),
            Flexible(
              child: Text(
                text,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: tx(
                  danger ? c.red : (selected || hovered ? c.fg : c.fg2),
                  size: 13,
                  weight: w500,
                  height: 1.35,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
