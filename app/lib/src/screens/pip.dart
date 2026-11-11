import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';

import '../app_state.dart';
import '../icons.dart';
import '../fair_size.dart';
import '../l10n.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';

/// What fills the room on this device: a shared screen being watched or an enlarged
/// camera, someone else's or this device's own.
class Spotlit {
  const Spotlit({
    required this.person,
    required this.renderer,
    required this.screen,
    required this.mine,
  });

  final UiPerson person;
  final RTCVideoRenderer renderer;

  /// A shared screen, as opposed to a camera.
  final bool screen;

  /// This device's own screen or camera.
  final bool mine;

  /// How it is named over the picture.
  String get title => screen
      ? (mine ? l10n.pipYourScreen : l10n.pipScreenOf(person.name))
      : (mine ? l10n.pipYourCamera : person.name);

  bool same(Spotlit? other) =>
      other != null &&
      other.person.id == person.id &&
      other.screen == screen &&
      other.mine == mine;
}

/// What fills the room of the call this device is in, as the room draws it; null when the
/// room shows its tiles.
Spotlit? spotlitOf(AppState state) {
  final call = state.callState.call;
  final room = state.roomState.room;
  if (call == null || room == null || call.group != state.roomState.roomGroup) {
    return null;
  }
  UiPerson? person(String id) =>
      room.participants.where((p) => p.person.id == id).firstOrNull?.person;
  final me = state.contacts.profile?.person;
  if (call.ownScreen && call.sharing && me != null) {
    return Spotlit(
      person: me,
      renderer: call.localScreen,
      screen: true,
      mine: true,
    );
  }
  final focused = call.focused;
  if (focused != null) {
    final who = person(focused);
    if (who == null) return null;
    if (focused == me?.id) {
      return call.camera
          ? Spotlit(
              person: who,
              renderer: call.localCamera,
              screen: false,
              mine: true,
            )
          : null;
    }
    final peer = call.peers[focused];
    if (peer == null || !peer.connected || !peer.cam) return null;
    return Spotlit(
      person: who,
      renderer: peer.camera,
      screen: false,
      mine: false,
    );
  }
  final watching = call.watching;
  if (watching != null) {
    final who = person(watching);
    final peer = call.peers[watching];
    if (who == null || peer == null || !peer.connected || !peer.sharing) {
      return null;
    }
    return Spotlit(
      person: who,
      renderer: peer.screen,
      screen: true,
      mine: false,
    );
  }
  return null;
}

/// Someone else's screen or camera that fills the room, while the room is out of sight:
/// another page is shown, or the settings cover it. It then floats over the window.
Spotlit? floatingOf(AppState state) {
  if (state.nav.page == Screen.room && !state.nav.settingsOpen) return null;
  final shown = spotlitOf(state);
  return shown == null || shown.mine ? null : shown;
}

/// Back to the room or call the floating picture comes from, where it fills the room again.
Future<void> backToRoom(AppState state) async {
  if (state.nav.settingsOpen && !await state.closeSettings()) return;
  final group = state.roomState.roomGroup;
  if (state.nav.page == Screen.room || group == null) return;
  // Only the page changes: the room and its call go on as they are.
  if (state.callState.callIn(group) != null) {
    state.showCall();
  } else {
    state.selectGroup(group);
  }
}

/// The size of the cards in the window's bottom right corner (see `CornerCards`), zero
/// while there are none: the floating picture sits above them and the toasts beside them.
final cornerCards = ValueNotifier<Size>(Size.zero);

/// The floating picture's size while it sits in the corner; zero otherwise.
final pipInCorner = ValueNotifier<Size>(Size.zero);

/// Where the toasts go, [bottom] above the window's bottom edge when nothing is in the
/// corner, in a window of [window] size: in the middle of the window, as far from its right
/// edge as the side column reaches from its left, so the person's own card at the column's
/// foot stays clear. With cards or the floating picture in the corner, as far from both
/// edges as they reach or, when that leaves too little room, above what the corner holds;
/// so they cover neither.
({double left, double right, double bottom}) toastsPlace(
  Size window,
  double bottom,
) {
  // The rail, and the side column beside it in a wide enough window.
  final side = window.width >= 860 ? 312.0 : 72.0;
  final cards = cornerCards.value;
  final pip = pipInCorner.value;
  final width = math.max(cards.width, pip.width);
  if (width <= 0) return (left: side, right: side, bottom: bottom);
  final height =
      cards.height +
      (pip.height > 0 ? pip.height + (cards.height > 0 ? _gap : 0) : 0);
  final inset = math.max(side, pipCorner + width + _gap);
  if (window.width - 2 * inset >= 400) {
    return (left: inset, right: inset, bottom: bottom);
  }
  return (
    left: side,
    right: side,
    bottom: math.max(bottom, pipCorner + height + _gap),
  );
}

/// The distance from the window's right and bottom edges to the corner's cards.
const pipCorner = 24.0;

/// The floating picture's largest size: its own shape inside it.
const pipBox = Size(320, 200);

/// How close to the window's edges it may go, and its distance from the cards under it.
const _edge = 12.0;
const _gap = 12.0;

/// The application's header, which the picture stays under.
const _header = 40.0;

/// Where the floating picture is: in the corner above the corner's cards, or where it was
/// dropped, as fractions of the room left around it so it stays inside the window as the
/// window changes size.
@immutable
class PipSpot {
  const PipSpot.corner() : at = null;
  const PipSpot.at(Offset this.at);

  /// Fractions (0 to 1) of the room around the picture; null in the corner.
  final Offset? at;

  bool get inCorner => at == null;

  /// Where the picture may go in a window of [window] size.
  static Rect area(Size window) => Rect.fromLTRB(
    _edge,
    _header + _edge,
    math.max(_edge, window.width - _edge),
    math.max(_header + _edge, window.height - _edge),
  );

  /// The picture's place in a window of [window] size, with [cards] the size of the
  /// corner's cards.
  Rect place(Size window, Size picture, {Size cards = Size.zero}) {
    final room = area(window);
    final at = this.at;
    if (at == null) {
      final below = cards.height > 0 ? cards.height + _gap : 0;
      return _inside(
        room,
        Offset(
          window.width - pipCorner - picture.width,
          window.height - pipCorner - below - picture.height,
        ),
        picture,
      );
    }
    return Offset(
          room.left + at.dx * math.max(0, room.width - picture.width),
          room.top + at.dy * math.max(0, room.height - picture.height),
        ) &
        picture;
  }

  /// The picture moved to [topLeft], kept inside the window. Dropped close to its place
  /// in the corner, or pushed into the bottom right corner, it goes back to that place.
  static PipSpot dropped(
    Size window,
    Size picture,
    Offset topLeft, {
    Size cards = Size.zero,
  }) {
    final corner = const PipSpot.corner().place(window, picture, cards: cards);
    final room = area(window);
    final rect = _inside(room, topLeft, picture);
    if ((corner.topLeft - rect.topLeft).distance < 48 ||
        (room.right - rect.right < 48 && room.bottom - rect.bottom < 48)) {
      return const PipSpot.corner();
    }
    final kept = rect.topLeft;
    double part(double offset, double free) =>
        free <= 0 ? 0 : (offset / free).clamp(0.0, 1.0);
    return PipSpot.at(
      Offset(
        part(kept.dx - room.left, room.width - picture.width),
        part(kept.dy - room.top, room.height - picture.height),
      ),
    );
  }

  static Rect _inside(Rect room, Offset topLeft, Size picture) =>
      Offset(
        topLeft.dx.clamp(
          room.left,
          math.max(room.left, room.right - picture.width),
        ),
        topLeft.dy.clamp(
          room.top,
          math.max(room.top, room.bottom - picture.height),
        ),
      ) &
      picture;
}

/// The floating picture's size for a picture of [aspect] (width over height).
Size pipSize(double aspect) {
  final a = aspect.isFinite ? aspect.clamp(minAspect, maxAspect) : 16 / 9;
  var width = pipBox.width;
  var height = width / a;
  if (height > pipBox.height) {
    height = pipBox.height;
    width = height * a;
  }
  return Size(width, height);
}

/// Over the whole window: someone else's screen or camera that fills the room, small and
/// floating while another page is shown, so it can still be followed. It starts in the
/// bottom right corner above the corner's cards and can be dragged anywhere in the window;
/// a click goes back to the room, where it fills the room again, and its button stops
/// watching. The picture comes from the renderer the room uses: nothing more is received.
class PipLayer extends StatefulWidget {
  const PipLayer({super.key, this.unread = false});

  /// Not read by screen readers: a layer under it holds them.
  final bool unread;

  @override
  State<PipLayer> createState() => _PipLayerState();
}

class _PipLayerState extends State<PipLayer>
    with SingleTickerProviderStateMixin {
  late final _shown = AnimationController(
    vsync: this,
    duration: springTime,
    reverseDuration: const Duration(milliseconds: 180),
  );

  /// What it shows, or showed until it began to leave.
  Spotlit? _pip;

  /// Its picture is no longer there: it leaves without it.
  bool _gone = false;

  PipSpot _spot = const PipSpot.corner();

  /// Where its top left corner is while dragged.
  Offset? _drag;
  bool _hovered = false;

  /// The window's size at the last build: the picture follows a resize at once.
  Size? _window;

  @override
  void initState() {
    super.initState();
    _shown.addStatusListener((status) {
      if (status == AnimationStatus.dismissed && mounted) {
        setState(() {
          _pip = null;
          _gone = false;
          _drag = null;
          _hovered = false;
        });
        _reportCorner(Size.zero);
      }
    });
    cornerCards.addListener(_cardsChanged);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final state = AppScope.of(context);
    final now = floatingOf(state);
    if (now != null) {
      _pip = now;
      _gone = false;
      if (_shown.status != AnimationStatus.forward &&
          _shown.status != AnimationStatus.completed) {
        _shown.forward();
      }
    } else if (_pip != null) {
      // Back in the room, the picture is still there; when it ended, or the person left,
      // its renderer may already be let go of.
      _gone = !_pip!.same(spotlitOf(state));
      if (_shown.status != AnimationStatus.reverse &&
          _shown.status != AnimationStatus.dismissed) {
        _shown.reverse();
      }
    }
  }

  @override
  void dispose() {
    cornerCards.removeListener(_cardsChanged);
    _shown.dispose();
    super.dispose();
  }

  void _cardsChanged() {
    if (mounted && _pip != null && _spot.inCorner) setState(() {});
  }

  void _reportCorner(Size size) {
    if (pipInCorner.value == size) return;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      pipInCorner.value = size;
    });
  }

  @override
  Widget build(BuildContext context) {
    final pip = _pip;
    if (pip == null) return const SizedBox.shrink();
    return Positioned.fill(
      child: ExcludeSemantics(
        excluding: widget.unread,
        child: LayoutBuilder(
          builder: (context, box) {
            final window = box.biggest;
            final resized = _window != null && _window != window;
            _window = window;
            return ValueListenableBuilder<RTCVideoValue>(
              valueListenable: pip.renderer,
              builder: (context, video, _) {
                // The picture's own shape once it arrives, 16:9 until then.
                final aspect = _gone
                    ? 16 / 9
                    : fairAspect(video.width, video.height);
                final size = pipSize(aspect);
                final drag = _drag;
                final rect = drag != null
                    ? PipSpot._inside(PipSpot.area(window), drag, size)
                    : _spot.place(window, size, cards: cornerCards.value);
                _reportCorner(
                  _spot.inCorner && drag == null && !_gone ? size : Size.zero,
                );
                return Stack(
                  children: [
                    AnimatedPositioned(
                      duration: drag != null || resized
                          ? Duration.zero
                          : springTime,
                      curve: spring,
                      left: rect.left,
                      top: rect.top,
                      width: rect.width,
                      height: rect.height,
                      child: AnimatedBuilder(
                        animation: _shown,
                        builder: (context, child) {
                          final t = _shown.status == AnimationStatus.reverse
                              ? Curves.easeIn.transform(_shown.value)
                              : spring.transform(_shown.value);
                          return Opacity(
                            opacity: t.clamp(0.0, 1.0),
                            child: Transform.scale(
                              scale: .88 + .12 * t,
                              child: child,
                            ),
                          );
                        },
                        child: _gesture(context, pip, window, size, rect),
                      ),
                    ),
                  ],
                );
              },
            );
          },
        ),
      ),
    );
  }

  Widget _gesture(
    BuildContext context,
    Spotlit pip,
    Size window,
    Size size,
    Rect rect,
  ) {
    final state = AppScope.read(context);
    final dragging = _drag != null;
    final leaving = _shown.status == AnimationStatus.reverse;
    return IgnorePointer(
      ignoring: leaving,
      child: MouseRegion(
        cursor: dragging
            ? SystemMouseCursors.grabbing
            : SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          onTap: () => backToRoom(state),
          // It moves with the pointer from where it was pressed.
          dragStartBehavior: DragStartBehavior.down,
          onPanStart: (_) => setState(() => _drag = rect.topLeft),
          onPanUpdate: (d) => setState(
            () => _drag = PipSpot._inside(
              PipSpot.area(window),
              _drag! + d.delta,
              size,
            ).topLeft,
          ),
          onPanEnd: (_) => _drop(window, size),
          onPanCancel: () => _drop(window, size),
          child: Semantics(
            button: true,
            label: state.callState.callIn(state.roomState.roomGroup) != null
                ? l10n.pipBackToCall
                : l10n.pipBackToRoom,
            child: AnimatedScale(
              scale: dragging ? 1.04 : 1,
              duration: springTime,
              curve: spring,
              child: _PipView(
                key: const ValueKey('pip'),
                pip: pip,
                gone: _gone,
                hovered: _hovered || dragging,
                onClose: () => _close(state, pip),
              ),
            ),
          ),
        ),
      ),
    );
  }

  void _drop(Size window, Size size) {
    final drag = _drag;
    if (drag == null) return;
    setState(() {
      _spot = PipSpot.dropped(window, size, drag, cards: cornerCards.value);
      _drag = null;
    });
  }

  /// Stops watching the screen, or stops enlarging the camera, as the room's button does.
  void _close(AppState state, Spotlit pip) {
    final call = state.callState.call;
    if (call == null) return;
    if (pip.screen) {
      call.watch(null);
    } else {
      call.focus(null);
    }
  }
}

/// The floating picture: the screen or camera in its own shape, whose it is, and the
/// button that stops watching, shown while the pointer is over it.
class _PipView extends StatelessWidget {
  const _PipView({
    super.key,
    required this.pip,
    required this.gone,
    required this.hovered,
    required this.onClose,
  });

  final Spotlit pip;
  final bool gone;
  final bool hovered;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      clipBehavior: Clip.antiAlias,
      // Inside the outline, as when it was drawn under what is inside.
      padding: const EdgeInsets.all(1),
      decoration: BoxDecoration(
        color: Colors.black,
        borderRadius: BorderRadius.circular(14),
      ),
      // Over what is inside, which would cover it at the rounded corners.
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(14),
        border: Border.all(
          color: hovered
              ? (c.dark ? Colors.white.withValues(alpha: .22) : c.borderStrong)
              : (c.dark ? Colors.white.withValues(alpha: .1) : c.border),
        ),
      ),
      child: Stack(
        children: [
          // Only until the first picture arrives.
          if (!gone)
            Positioned.fill(
              child: ValueListenableBuilder(
                valueListenable: pip.renderer,
                builder: (context, video, spinner) =>
                    video.width > 0 && video.height > 0
                    ? const SizedBox.shrink()
                    : spinner!,
                child: const Center(
                  child: Spinner(size: 16, color: Colors.white54),
                ),
              ),
            ),
          if (!gone)
            Positioned.fill(
              child: RTCVideoView(
                pip.renderer,
                mirror:
                    pip.mine &&
                    !pip.screen &&
                    AppScope.only(context, const {Prefs}).prefs.mirrorCamera,
                objectFit: RTCVideoViewObjectFit.RTCVideoViewObjectFitContain,
              ),
            ),
          Positioned(
            left: 8,
            top: 8,
            right: 44,
            child: Align(
              alignment: Alignment.centerLeft,
              child: _Glass(
                height: 26,
                padding: EdgeInsets.fromLTRB(pip.screen ? 9 : 4, 0, 10, 0),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (pip.screen)
                      Container(
                        width: 7,
                        height: 7,
                        decoration: BoxDecoration(
                          color: c.redSolid,
                          shape: BoxShape.circle,
                        ),
                      )
                    else
                      Avatar(avatar: pip.person.avatar, size: 18),
                    const SizedBox(width: 7),
                    Flexible(
                      child: Text(
                        pip.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: tx(
                          Colors.white,
                          size: 12,
                          weight: w500,
                          height: 1.2,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Positioned(
            right: 8,
            top: 8,
            child: IgnorePointer(
              ignoring: !hovered,
              child: AnimatedOpacity(
                opacity: hovered ? 1 : 0,
                duration: const Duration(milliseconds: 160),
                child: AnimatedScale(
                  scale: hovered ? 1 : .8,
                  duration: springTime,
                  curve: spring,
                  child: Springy(
                    onTap: onClose,
                    tooltip: pip.screen
                        ? l10n.pipStopWatching
                        : l10n.commonClose,
                    builder: (context, over) => _Glass(
                      width: 26,
                      height: 26,
                      color: over ? const Color.fromRGBO(48, 48, 48, .8) : null,
                      child: const Center(
                        child: FgIcon(Ic.close, size: 13, color: Colors.white),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// A dark see-through pill over the picture, as over the room's picture.
class _Glass extends StatelessWidget {
  const _Glass({
    required this.child,
    this.padding,
    this.width,
    this.height,
    this.color,
  });
  final Widget child;
  final EdgeInsets? padding;
  final double? width;
  final double? height;
  final Color? color;

  @override
  // A solid tint rather than a blur: a blur over a playing video reads the picture
  // back on every frame.
  Widget build(BuildContext context) => Container(
    width: width,
    height: height,
    padding: padding,
    decoration: BoxDecoration(
      color: color ?? const Color.fromRGBO(16, 16, 16, .82),
      borderRadius: BorderRadius.circular(999),
      border: Border.all(color: Colors.white.withValues(alpha: .08)),
    ),
    child: child,
  );
}

/// Calls [onSize] with its child's size after each layout that changes it.
class SizeReport extends SingleChildRenderObjectWidget {
  const SizeReport({super.key, required this.onSize, super.child});
  final ValueChanged<Size> onSize;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderSizeReport(onSize);

  @override
  void updateRenderObject(BuildContext context, RenderObject renderObject) =>
      (renderObject as _RenderSizeReport).onSize = onSize;
}

class _RenderSizeReport extends RenderProxyBox {
  _RenderSizeReport(this.onSize);
  ValueChanged<Size> onSize;
  Size? _last;

  @override
  void performLayout() {
    super.performLayout();
    if (size == _last) return;
    _last = size;
    final now = size;
    SchedulerBinding.instance.addPostFrameCallback((_) => onSize(now));
  }
}
