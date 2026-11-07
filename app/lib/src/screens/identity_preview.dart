import 'package:flutter/material.dart';

import '../icons.dart';
import '../theme.dart';
import '../widgets.dart';

/// What the others see of a profile or a group, as it is being changed: the banner across
/// the top, the picture over the banner's lower edge, the name and a line under it. The
/// buttons that change the banner sit on it ([bannerActions]), the one that changes the
/// picture on the picture ([onPicture]), so each is where what it changes shows.
class IdentityPreview extends StatelessWidget {
  const IdentityPreview({
    super.key,
    required this.banner,
    required this.picture,
    required this.title,
    this.titleMuted = false,
    this.below,
    this.bannerActions = const [],
    this.onPicture,
    this.pictureTooltip,
    this.pictureBusy = false,
  });

  /// Fills the banner's box.
  final Widget banner;

  /// The picture, [pictureSize] across.
  final Widget picture;

  final String title;

  /// The title is a placeholder (no name yet): drawn fainter.
  final bool titleMuted;

  /// Under the name: a bio, a note.
  final Widget? below;
  final List<Widget> bannerActions;
  final VoidCallback? onPicture;
  final String? pictureTooltip;
  final bool pictureBusy;

  static const pictureSize = 80.0;
  static const _bannerHeight = 108.0;
  static const _ring = 4.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    const top = _bannerHeight - pictureSize / 2 - _ring;
    return Container(
      clipBehavior: Clip.antiAlias,
      // Inside the outline, as when it was drawn under what is inside.
      padding: const EdgeInsets.all(1),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(18),
      ),
      // Over what is inside, which would cover it at the rounded corners.
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: c.border),
      ),
      child: Stack(
        children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(height: _bannerHeight, child: banner),
              Padding(
                padding: EdgeInsets.fromLTRB(20, pictureSize / 2 + 12, 20, 18),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: hx(titleMuted ? c.fg3 : c.fg, size: 18),
                    ),
                    ?below,
                  ],
                ),
              ),
            ],
          ),
          if (bannerActions.isNotEmpty)
            Positioned(
              top: 10,
              right: 10,
              child: Row(
                children: [
                  for (final (i, action) in bannerActions.indexed) ...[
                    if (i > 0) const SizedBox(width: 6),
                    action,
                  ],
                ],
              ),
            ),
          Positioned(
            left: 16,
            top: top,
            child: Container(
              padding: const EdgeInsets.all(_ring),
              decoration: BoxDecoration(color: c.bg1, shape: BoxShape.circle),
              child: SizedBox(
                width: pictureSize,
                height: pictureSize,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    picture,
                    if (onPicture != null)
                      Positioned(
                        right: -2,
                        bottom: -2,
                        child: _PictureButton(
                          tooltip: pictureTooltip,
                          busy: pictureBusy,
                          onTap: onPicture!,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// The button on the picture that changes it: a pencil in a solid round button, ringed
/// in the card's colour so it stands apart from any picture.
class _PictureButton extends StatelessWidget {
  const _PictureButton({required this.onTap, this.tooltip, this.busy = false});

  final VoidCallback onTap;
  final String? tooltip;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Springy(
      onTap: busy ? null : onTap,
      tooltip: tooltip,
      label: tooltip,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        width: 32,
        height: 32,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? c.fg : c.bg2,
          shape: BoxShape.circle,
          border: Border.all(color: c.bg1, width: 3),
        ),
        child: busy
            ? SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(strokeWidth: 1.6, color: c.fg),
              )
            : FgIcon(Ic.pencil, size: 14, color: hovered ? c.bg : c.fg),
      ),
    );
  }
}

/// A button over a picture: dark and see-through, so it reads over any banner. With
/// [text] a pill, without a round button; [ring] draws a ring of that colour around it.
class OverPill extends StatelessWidget {
  const OverPill({
    super.key,
    required this.icon,
    this.text,
    this.tooltip,
    this.onTap,
    this.busy = false,
    this.ring,
  });

  final Ic icon;
  final String? text;
  final String? tooltip;
  final VoidCallback? onTap;
  final bool busy;
  final Color? ring;

  @override
  Widget build(BuildContext context) {
    final text = this.text;
    return Springy(
      onTap: busy ? null : onTap,
      tooltip: tooltip,
      label: text ?? tooltip,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        height: 30,
        constraints: const BoxConstraints(minWidth: 30),
        padding: EdgeInsets.symmetric(horizontal: text == null ? 0 : 11),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: Color.fromRGBO(0, 0, 0, hovered ? .72 : .55),
          borderRadius: BorderRadius.circular(999),
          border: ring == null ? null : Border.all(color: ring!, width: 2),
        ),
        child: busy
            ? const SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(
                  strokeWidth: 1.6,
                  color: Colors.white,
                ),
              )
            : Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  FgIcon(icon, size: 14, color: Colors.white),
                  if (text != null) ...[
                    const SizedBox(width: 6),
                    Text(text, style: tx(Colors.white, size: 12, weight: w500)),
                  ],
                ],
              ),
      ),
    );
  }
}

/// A row of the icons a group can show instead of a picture, as round buttons; the one in
/// use ringed. [selected] is null while a picture shows instead.
class GroupIconRow extends StatelessWidget {
  const GroupIconRow({super.key, required this.selected, required this.onPick});

  final int? selected;
  final ValueChanged<int> onPick;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final (i, (icon, label)) in groupIcons.indexed)
          Springy(
            onTap: () => onPick(i),
            label: label,
            tooltip: label,
            selected: selected == i,
            focusRadius: 20,
            builder: (context, hovered) {
              final on = selected == i;
              return Container(
                width: 40,
                height: 40,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: hovered || on ? c.hover : c.bg2,
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: on ? c.fg : c.border,
                    width: on ? 2 : 1,
                  ),
                ),
                child: FgIcon(icon, size: 17, color: on ? c.fg : c.fg2),
              );
            },
          ),
      ],
    );
  }
}
