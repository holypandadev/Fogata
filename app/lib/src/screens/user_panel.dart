import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../moving.dart';
import '../names.dart';
import '../safe_image.dart';
import '../theme.dart';
import '../widgets.dart';
import 'home.dart' show Ticker;

/// The layer the window's floating pieces are drawn in; panels opened from something in
/// the window are placed with [boundsInLayer].
final shellLayerKey = GlobalKey();

/// Where `context`'s box is in the window's layer. The box is kept, so a panel placed
/// by what this gave can follow it ([followBounds]).
Rect? boundsInLayer(BuildContext context) {
  final bounds = _bounds(context);
  if (bounds != null) {
    _boxes
      ..remove(bounds)
      ..[bounds] = context;
    // The last few are plenty: one panel is open at a time.
    while (_boxes.length > 8) {
      _boxes.remove(_boxes.keys.first);
    }
  }
  return bounds;
}

/// The boxes the last bounds [boundsInLayer] gave were taken from.
final _boxes = <Rect, BuildContext>{};

Rect? _bounds(BuildContext context) {
  final box = context.findRenderObject() as RenderBox?;
  final layer = shellLayerKey.currentContext?.findRenderObject() as RenderBox?;
  if (box == null || layer == null || !box.attached || !box.hasSize) {
    return null;
  }
  return box.localToGlobal(Offset.zero, ancestor: layer) & box.size;
}

/// Where the box that [from] was taken from by [boundsInLayer] is now: panels in the
/// window's layer call it as they lay out (in a LayoutBuilder, after the page under them),
/// so they stay beside it when the window changes size. [from] once the box is gone.
Rect followBounds(Rect from) {
  final context = _boxes[from];
  if (context == null || !context.mounted) return from;
  return _bounds(context) ?? from;
}

/// Opens the person's panel next to `context`'s box, or closes it if it is open.
void toggleUserPanel(BuildContext context) {
  final state = AppScope.read(context);
  if (state.nav.userPanelFrom != null) {
    state.closeUserPanel();
    return;
  }
  final from = boundsInLayer(context);
  if (from != null) state.openUserPanel(from);
}

/// The person's own panel, over the window above their card, which opens it. A click
/// beside it or Esc closes it.
class UserPanelLayer extends StatelessWidget {
  const UserPanelLayer({super.key});

  static const width = 300.0;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final opened = state.nav.userPanelFrom;
    if (opened == null) return const SizedBox.shrink();
    return Positioned.fill(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape):
              state.closeUserPanel,
        },
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusLayer(
          child: LayoutBuilder(
            builder: (context, box) {
              final from = followBounds(opened);
              final above = from.center.dy > box.maxHeight / 2;
              final double left = (above ? from.left : from.right - width)
                  .clamp(8.0, math.max(8.0, box.maxWidth - width - 8));
              return Stack(
                children: [
                  Positioned.fill(child: Backdrop(onTap: state.closeUserPanel)),
                  Positioned(
                    left: left,
                    top: above ? null : from.bottom + 8,
                    bottom: above ? box.maxHeight - from.top + 8 : null,
                    child: TweenAnimationBuilder<double>(
                      tween: Tween(begin: 0, end: 1),
                      duration: springTime,
                      curve: spring,
                      builder: (context, t, child) => Opacity(
                        opacity: t.clamp(0, 1),
                        child: Transform.translate(
                          offset: Offset(0, (above ? 10 : -10) * (1 - t)),
                          child: Transform.scale(
                            scale: .95 + .05 * t,
                            alignment: above
                                ? Alignment.bottomLeft
                                : Alignment.topRight,
                            child: child,
                          ),
                        ),
                      ),
                      child: const _UserPanel(),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _UserPanel extends StatefulWidget {
  const _UserPanel();

  @override
  State<_UserPanel> createState() => _UserPanelState();
}

class _UserPanelState extends State<_UserPanel> {
  bool _copying = false;
  bool _copied = false;
  Timer? _back;

  @override
  void dispose() {
    _back?.cancel();
    super.dispose();
  }

  Future<void> _copy() async {
    if (_copying) return;
    setState(() => _copying = true);
    final ok = await AppScope.read(context).copyFriendLink();
    if (!mounted) return;
    _back?.cancel();
    setState(() {
      _copying = false;
      _copied = ok;
    });
    if (ok) {
      _back = Timer(const Duration(milliseconds: 1500), () {
        if (mounted) setState(() => _copied = false);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final profile = state.contacts.profile;
    if (profile == null) return const SizedBox.shrink();
    final offline = state.prefs.appearOffline;
    return Container(
      width: UserPanelLayer.width,
      clipBehavior: Clip.antiAlias,
      // Inside the outline, as when it was drawn under what is inside.
      padding: const EdgeInsets.all(1),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(22),
      ),
      // Over what is inside, which would cover it at the rounded corners.
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: c.border),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Stack(
            clipBehavior: Clip.none,
            children: [
              SizedBox(
                height: 96,
                child: PanelBanner(
                  color: state.prefs.bannerColor,
                  picture: state.prefs.bannerPicture,
                  id: state.contacts.profile?.person.id,
                ),
              ),
              Positioned(
                left: 13,
                top: 50,
                child: _RingedAvatar(
                  state: state,
                  offline: offline,
                  away: state.contacts.away,
                ),
              ),
            ],
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 8, 14, 14),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Align(
                  alignment: Alignment.centerRight,
                  child: FgButton(
                    text: l10n.userPanelEditProfile,
                    icon: Ic.pencil,
                    height: 30,
                    fontSize: 12,
                    onTap: () {
                      state.closeUserPanel();
                      state.openSettings(SettingsSection.profile);
                    },
                  ),
                ),
                const SizedBox(height: 16),
                Text(
                  profile.person.name,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 19, weight: w600, height: 1.25),
                ),
                CardBio(state.contacts.bio),
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.all(6),
                  decoration: BoxDecoration(
                    color: c.bg1,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: c.border),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(8, 6, 2, 8),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            // The choice goes under its label when the two do not
                            // fit side by side.
                            Wrap(
                              alignment: WrapAlignment.spaceBetween,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              spacing: 12,
                              runSpacing: 6,
                              children: [
                                Text(
                                  l10n.userPanelStatus,
                                  style: tx(c.fg2, size: 12, weight: w500),
                                ),
                                Segmented(
                                  height: 32,
                                  tabs: [
                                    (l10n.userPanelOnline, null, null),
                                    (l10n.userPanelOffline, null, null),
                                  ],
                                  leading: [
                                    StatusDot(offline: false, size: 7),
                                    StatusDot(offline: true, size: 8),
                                  ],
                                  selected: offline ? 1 : 0,
                                  onSelect: (i) =>
                                      state.setAppearOffline(i == 1),
                                ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            // As tall as the longer of the two notes, in any language
                            // and text size, so the buttons stay put.
                            Stack(
                              children: [
                                for (final note in [
                                  l10n.userPanelOfflineNote,
                                  l10n.userPanelOnlineNote,
                                ])
                                  Visibility(
                                    visible: false,
                                    maintainSize: true,
                                    maintainAnimation: true,
                                    maintainState: true,
                                    child: Text(
                                      note,
                                      style: tx(c.fg3, size: 12, height: 1.4),
                                    ),
                                  ),
                                Positioned.fill(
                                  child: AnimatedSwitcher(
                                    duration: const Duration(milliseconds: 200),
                                    layoutBuilder: (current, previous) => Stack(
                                      alignment: Alignment.topLeft,
                                      children: [...previous, ?current],
                                    ),
                                    child: Text(
                                      offline
                                          ? l10n.userPanelOfflineNote
                                          : l10n.userPanelOnlineNote,
                                      key: ValueKey(offline),
                                      style: tx(c.fg3, size: 12, height: 1.4),
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
                      Container(
                        height: 1,
                        margin: const EdgeInsets.symmetric(horizontal: 8),
                        color: c.border,
                      ),
                      const SizedBox(height: 2),
                      _CopyLinkRow(
                        copying: _copying,
                        copied: _copied,
                        onTap: _copy,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// A person's bio on their card, under a small heading: [defaultBio] when they never wrote
/// one, and nothing when they chose to show none.
class CardBio extends StatelessWidget {
  const CardBio(this.bio, {super.key, this.top = 12});

  /// The bio as the core keeps it; null when none was ever written.
  final String? bio;

  /// The space above it, left out with it.
  final double top;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final text = bio ?? defaultBio;
    if (text.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: EdgeInsets.only(top: top),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.userPanelBio,
            style: tx(c.fg3, size: 11, weight: w600, spacing: .4),
          ),
          const SizedBox(height: 4),
          Text(
            text,
            key: const ValueKey('card-bio'),
            maxLines: 8,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg, size: 13, height: 1.4),
          ),
        ],
      ),
    );
  }
}

/// The top of a person's panel: their picture if they chose one, else their colour.
class PanelBanner extends StatelessWidget {
  const PanelBanner({super.key, required this.color, this.picture, this.id});
  final Color color;
  final Uint8List? picture;

  /// Whose banner it is: given, the person's animation of it plays (moving.dart).
  final String? id;

  @override
  Widget build(BuildContext context) {
    final picture = this.picture;
    final id = this.id;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 300),
      color: color,
      child: picture == null
          ? null
          : id == null
          ? _still(picture)
          : ListenableBuilder(
              listenable: movingBook,
              builder: (context, _) {
                final moving = movingBook.banner(id, picture);
                if (moving == null) return _still(picture);
                return MovingPicture(
                  key: ValueKey(moving),
                  bytes: moving,
                  maxPixels: maxMovingBannerPixels,
                  still: _still(picture),
                );
              },
            ),
    );
  }

  /// Decoded as wide as it is drawn.
  Widget _still(Uint8List picture) => LayoutBuilder(
    builder: (context, box) => Image(
      image: safePicture(
        picture,
        width: box.maxWidth.isFinite
            ? (box.maxWidth * MediaQuery.devicePixelRatioOf(context)).round()
            : null,
      ),
      fit: BoxFit.cover,
      gaplessPlayback: true,
      width: double.infinity,
      height: double.infinity,
    ),
  );
}

/// Online: a green dot; away: an amber moon; appearing offline: a grey ring, so the three
/// differ in shape too.
class StatusDot extends StatelessWidget {
  const StatusDot({
    super.key,
    required this.offline,
    this.away = false,
    this.size = 10,
    this.ring = 0,
    this.inset = 0,
    this.ringColor,
  });
  final bool offline;

  /// Online, but nobody used the computer for a while (offline wins).
  final bool away;
  final double size;

  /// A ring of the background around it, to sit on a picture's edge.
  final double ring;

  /// A ring of the background inside its size instead.
  final double inset;
  final Color? ringColor;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final background = ringColor ?? c.bg2;
    final moon = away && !offline;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: offline
            ? background
            : moon
            ? c.amber
            : c.green,
        border: offline
            ? Border.all(color: c.fg3, width: math.max(1.5, size * .26))
            : inset > 0
            ? Border.all(color: background, width: inset)
            : null,
        boxShadow: ring > 0
            ? [BoxShadow(color: background, spreadRadius: ring)]
            : null,
      ),
      // The moon: a circle of the background bites into the disc from its top left.
      child: TweenAnimationBuilder<double>(
        tween: Tween(end: moon ? 1 : 0),
        duration: const Duration(milliseconds: 200),
        curve: Curves.easeOut,
        builder: (context, bite, _) => bite == 0
            ? const SizedBox.expand()
            : ClipOval(
                child: CustomPaint(
                  painter: _MoonBite(background, bite),
                  child: const SizedBox.expand(),
                ),
              ),
      ),
    );
  }
}

/// The circle cut from the moon's disc: 70 % of it across, reaching 12 % beyond its top
/// left edge; `bite` grows it from nothing.
class _MoonBite extends CustomPainter {
  _MoonBite(this.color, this.bite);
  final Color color;
  final double bite;

  @override
  void paint(Canvas canvas, Size size) {
    final d = size.shortestSide;
    canvas.drawCircle(
      Offset(d * .23, d * .23),
      d * .35 * bite,
      Paint()
        ..color = color
        ..isAntiAlias = true,
    );
  }

  @override
  bool shouldRepaint(_MoonBite old) => old.color != color || old.bite != bite;
}

class _RingedAvatar extends StatelessWidget {
  const _RingedAvatar({
    required this.state,
    required this.offline,
    required this.away,
  });
  final AppState state;
  final bool offline;
  final bool away;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final profile = state.contacts.profile!;
    return SizedBox(
      width: 92,
      height: 92,
      child: Stack(
        children: [
          Container(
            decoration: BoxDecoration(shape: BoxShape.circle, color: c.bg2),
            alignment: Alignment.center,
            child: Avatar(
              avatar: profile.person.avatar,
              size: 82,
              id: profile.person.id,
            ),
          ),
          Positioned(
            right: 5,
            bottom: 5,
            child: Tooltip(
              message: offline
                  ? l10n.userPanelAppearingOffline
                  : away
                  ? l10n.userPanelAway
                  : l10n.userPanelOnline,
              child: StatusDot(offline: offline, away: away, size: 20, ring: 4),
            ),
          ),
        ],
      ),
    );
  }
}

/// Copies a new friend link; the button turns into a green "Copied" for a moment, and
/// while the copied link works it says how long it has left.
class _CopyLinkRow extends StatelessWidget {
  const _CopyLinkRow({
    required this.copying,
    required this.copied,
    required this.onTap,
  });
  final bool copying;
  final bool copied;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final link = state.contacts.friendLink;
    return Pressable(
      onTap: copying ? null : onTap,
      label: l10n.userPanelCopyLinkLabel,
      focusRadius: 11,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        // Taller when the words take two lines.
        constraints: const BoxConstraints(minHeight: 40),
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          color: hovered || copied ? c.hover : Colors.transparent,
          borderRadius: BorderRadius.circular(11),
        ),
        child: LayoutBuilder(
          builder: (context, box) => Ticker(
            builder: (context) {
              final width = box.maxWidth;
              final left = link == null
                  ? 0
                  : link.expiresMs - DateTime.now().millisecondsSinceEpoch;
              final note = left > 0
                  ? l10n.userPanelLinkExpiresIn(countdown(link!.expiresMs))
                  : l10n.userPanelLinkTerms;
              return AnimatedSwitcher(
                duration: const Duration(milliseconds: 220),
                switchInCurve: spring,
                transitionBuilder: (child, animation) => FadeTransition(
                  opacity: animation,
                  child: ScaleTransition(
                    scale: Tween(begin: .92, end: 1.0).animate(animation),
                    child: child,
                  ),
                ),
                child: Row(
                  key: ValueKey(copied),
                  children: [
                    if (copying)
                      Spinner(size: 14, color: c.fg2)
                    else if (copied)
                      Container(
                        width: 18,
                        height: 18,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: c.greenSoft,
                        ),
                        child: FgIcon(Ic.check, size: 11, color: c.green),
                      )
                    else
                      FgIcon(Ic.link, size: 15, color: c.fg),
                    const SizedBox(width: 9),
                    Expanded(
                      child: Text(
                        copied ? l10n.commonCopied : l10n.userPanelCopyLink,
                        style: tx(
                          copied ? c.green : c.fg,
                          size: 13,
                          weight: w500,
                        ),
                      ),
                    ),
                    // With large text, never more than about half the row: it
                    // takes two lines rather than the words' room.
                    ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: largeText(context)
                            ? width * .45
                            : double.infinity,
                      ),
                      child: Text(
                        note,
                        textAlign: TextAlign.end,
                        style: tx(c.fg3, size: 11),
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}
