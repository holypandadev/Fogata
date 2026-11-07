import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../moving.dart';
import '../safe_image.dart';
import '../theme.dart';
import '../widgets.dart';

/// How tall a group's banner is at the top of the side column.
const groupBannerHeight = 136.0;

/// The top of the side column: the group's name and its settings button, alone in a bar,
/// or over the top of the group's banner when it has one.
class ColumnHeader extends StatelessWidget {
  const ColumnHeader({
    super.key,
    required this.title,
    this.banner,
    this.onSettings,
  });

  final String title;
  final Uint8List? banner;
  final VoidCallback? onSettings;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final banner = this.banner;
    if (banner == null) {
      return Container(
        height: 48,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          border: Border(bottom: BorderSide(color: c.border)),
        ),
        child: Row(
          children: [
            Expanded(
              child: Text(
                title,
                overflow: TextOverflow.ellipsis,
                style: hx(c.fg, size: 14),
              ),
            ),
            if (onSettings != null)
              FgButton(
                icon: Ic.settings,
                kind: Kind.ghost,
                height: 28,
                tooltip: l10n.groupBannerSettings,
                onTap: onSettings,
              ),
          ],
        ),
      );
    }
    return Container(
      key: const ValueKey('group-banner'),
      height: groupBannerHeight,
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: c.border)),
      ),
      child: Stack(
        fit: StackFit.expand,
        children: [
          BannerPicture(bytes: banner),
          // Darker at the top, so the name reads over any picture.
          const DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                stops: [0, .45],
                colors: [
                  Color.fromRGBO(0, 0, 0, .6),
                  Color.fromRGBO(0, 0, 0, 0),
                ],
              ),
            ),
          ),
          Positioned(
            left: 16,
            right: 10,
            top: 0,
            height: 48,
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    title,
                    overflow: TextOverflow.ellipsis,
                    style: tx(Colors.white, weight: w600),
                  ),
                ),
                if (onSettings != null) _OverButton(onTap: onSettings!),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The settings button over a banner: a dark pill that reads over any picture.
class _OverButton extends StatelessWidget {
  const _OverButton({required this.onTap});
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Springy(
    onTap: onTap,
    tooltip: l10n.groupBannerSettings,
    builder: (context, hovered) => AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      width: 28,
      height: 28,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: Color.fromRGBO(0, 0, 0, hovered ? .6 : .42),
      ),
      child: FgIcon(Ic.settings, size: 15, color: Colors.white),
    ),
  );
}

/// A banner someone else chose, covering its box: an animation plays, a still picture
/// shows as it is.
class BannerPicture extends StatelessWidget {
  const BannerPicture({super.key, required this.bytes});
  final Uint8List bytes;

  @override
  Widget build(BuildContext context) {
    final still = LayoutBuilder(
      builder: (context, box) => Image(
        image: safePicture(
          bytes,
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
    if (!isMovingKind(bytes)) return still;
    return MovingPicture(
      bytes: bytes,
      maxPixels: maxMovingGroupBannerPixels,
      still: const SizedBox.expand(),
    );
  }
}

/// Lets the person pick a group's banner from the computer; tests put their own here.
@visibleForTesting
Future<Uint8List?> Function() pickBannerFile = pickGroupBanner;

/// Asks for a group's banner from the computer: the picture, or null when none was
/// chosen or it could not be used (the person is told why).
Future<Uint8List?> chooseGroupBanner(AppState state) async {
  try {
    return await pickBannerFile();
  } on PictureProblem catch (e) {
    state.say(e.message, error: true);
  } catch (_) {
    state.say(l10n.groupBannerPictureUnreadable, error: true);
  }
  return null;
}

/// The banner's box before one is chosen: a quiet shading of the theme, so the preview
/// keeps its shape.
class EmptyBanner extends StatelessWidget {
  const EmptyBanner({super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [c.hover, c.bg2],
        ),
      ),
    );
  }
}
