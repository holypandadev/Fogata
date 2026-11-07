import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'l10n.dart';

const _regular = 'PhosphorRegular';
const _bold = 'PhosphorBold';

/// The design's icons: Phosphor's glyphs (MIT, drawn as a font, so they are smooth at
/// every size) by code point, in a regular and a bold weight, and whether a slash is
/// drawn across.
enum Ic {
  lock(
    IconData(0xe308, fontFamily: _regular),
    IconData(0xe308, fontFamily: _bold),
  ),
  nodes(
    IconData(0xeb58, fontFamily: _regular),
    IconData(0xeb58, fontFamily: _bold),
  ),
  hourglass(
    IconData(0xe2ba, fontFamily: _regular),
    IconData(0xe2ba, fontFamily: _bold),
  ),
  plus(
    IconData(0xe3d4, fontFamily: _regular),
    IconData(0xe3d4, fontFamily: _bold),
  ),
  eye(
    IconData(0xe220, fontFamily: _regular),
    IconData(0xe220, fontFamily: _bold),
  ),
  eyeOff(
    IconData(0xe224, fontFamily: _regular),
    IconData(0xe224, fontFamily: _bold),
  ),
  swap(
    IconData(0xe0a0, fontFamily: _regular),
    IconData(0xe0a0, fontFamily: _bold),
  ),
  laptop(
    IconData(0xe586, fontFamily: _regular),
    IconData(0xe586, fontFamily: _bold),
  ),
  monitor(
    IconData(0xe560, fontFamily: _regular),
    IconData(0xe560, fontFamily: _bold),
  ),
  checkCircle(
    IconData(0xe184, fontFamily: _regular),
    IconData(0xe184, fontFamily: _bold),
  ),
  warningCircle(
    IconData(0xe4e2, fontFamily: _regular),
    IconData(0xe4e2, fontFamily: _bold),
  ),
  crown(
    IconData(0xe616, fontFamily: _regular),
    IconData(0xe616, fontFamily: _bold),
  ),
  link(
    IconData(0xe2e6, fontFamily: _regular),
    IconData(0xe2e6, fontFamily: _bold),
  ),
  userMinus(
    IconData(0xe4ce, fontFamily: _regular),
    IconData(0xe4ce, fontFamily: _bold),
  ),
  shield(
    IconData(0xe40c, fontFamily: _regular),
    IconData(0xe40c, fontFamily: _bold),
  ),
  arrowRight(
    IconData(0xe06c, fontFamily: _regular),
    IconData(0xe06c, fontFamily: _bold),
  ),
  search(
    IconData(0xe30c, fontFamily: _regular),
    IconData(0xe30c, fontFamily: _bold),
  ),
  grid(
    IconData(0xe464, fontFamily: _regular),
    IconData(0xe464, fontFamily: _bold),
  ),
  users(
    IconData(0xe4d6, fontFamily: _regular),
    IconData(0xe4d6, fontFamily: _bold),
  ),
  sliders(
    IconData(0xe434, fontFamily: _regular),
    IconData(0xe434, fontFamily: _bold),
  ),
  settings(
    IconData(0xe272, fontFamily: _regular),
    IconData(0xe272, fontFamily: _bold),
  ),
  star(
    IconData(0xe46a, fontFamily: _regular),
    IconData(0xe46a, fontFamily: _bold),
  ),
  gamepadSmall(
    IconData(0xe26e, fontFamily: _regular),
    IconData(0xe26e, fontFamily: _bold),
  ),
  waves(
    IconData(0xe6de, fontFamily: _regular),
    IconData(0xe6de, fontFamily: _bold),
  ),
  music(
    IconData(0xe340, fontFamily: _regular),
    IconData(0xe340, fontFamily: _bold),
  ),
  book(
    IconData(0xe0e6, fontFamily: _regular),
    IconData(0xe0e6, fontFamily: _bold),
  ),
  micOff(
    IconData(0xe328, fontFamily: _regular),
    IconData(0xe328, fontFamily: _bold),
  ),
  camera(
    IconData(0xe4da, fontFamily: _regular),
    IconData(0xe4da, fontFamily: _bold),
  ),
  clock(
    IconData(0xe19a, fontFamily: _regular),
    IconData(0xe19a, fontFamily: _bold),
  ),
  mic(
    IconData(0xe326, fontFamily: _regular),
    IconData(0xe326, fontFamily: _bold),
  ),
  cameraOff(
    IconData(0xe4dc, fontFamily: _regular),
    IconData(0xe4dc, fontFamily: _bold),
  ),
  gamepad(
    IconData(0xe26e, fontFamily: _regular),
    IconData(0xe26e, fontFamily: _bold),
  ),
  dots(
    IconData(0xe1fe, fontFamily: _regular),
    IconData(0xe1fe, fontFamily: _bold),
  ),
  close(
    IconData(0xe4f6, fontFamily: _regular),
    IconData(0xe4f6, fontFamily: _bold),
  ),
  mountain(
    IconData(0xe7ae, fontFamily: _regular),
    IconData(0xe7ae, fontFamily: _bold),
  ),
  coffee(
    IconData(0xe1c2, fontFamily: _regular),
    IconData(0xe1c2, fontFamily: _bold),
  ),
  heart(
    IconData(0xe2a8, fontFamily: _regular),
    IconData(0xe2a8, fontFamily: _bold),
  ),
  bug(
    IconData(0xe5f4, fontFamily: _regular),
    IconData(0xe5f4, fontFamily: _bold),
  ),
  paperPlane(
    IconData(0xe398, fontFamily: _regular),
    IconData(0xe398, fontFamily: _bold),
  ),
  fileText(
    IconData(0xe23a, fontFamily: _regular),
    IconData(0xe23a, fontFamily: _bold),
  ),
  film(
    IconData(0xe792, fontFamily: _regular),
    IconData(0xe792, fontFamily: _bold),
  ),
  check(
    IconData(0xe182, fontFamily: _regular),
    IconData(0xe182, fontFamily: _bold),
  ),
  warning(
    IconData(0xe4e0, fontFamily: _regular),
    IconData(0xe4e0, fontFamily: _bold),
  ),
  wifiOff(
    IconData(0xe4f2, fontFamily: _regular),
    IconData(0xe4f2, fontFamily: _bold),
  ),
  retry(
    IconData(0xe036, fontFamily: _regular),
    IconData(0xe036, fontFamily: _bold),
  ),
  paperclip(
    IconData(0xe39a, fontFamily: _regular),
    IconData(0xe39a, fontFamily: _bold),
  ),
  leave(
    IconData(0xe42a, fontFamily: _regular),
    IconData(0xe42a, fontFamily: _bold),
  ),
  upload(
    IconData(0xe4c0, fontFamily: _regular),
    IconData(0xe4c0, fontFamily: _bold),
  ),
  file(
    IconData(0xe230, fontFamily: _regular),
    IconData(0xe230, fontFamily: _bold),
  ),
  download(
    IconData(0xe20c, fontFamily: _regular),
    IconData(0xe20c, fontFamily: _bold),
  ),
  send(
    IconData(0xe08e, fontFamily: _regular),
    IconData(0xe08e, fontFamily: _bold),
  ),
  chat(
    IconData(0xe168, fontFamily: _regular),
    IconData(0xe168, fontFamily: _bold),
  ),
  info(
    IconData(0xe2ce, fontFamily: _regular),
    IconData(0xe2ce, fontFamily: _bold),
  ),
  copy(
    IconData(0xe1ca, fontFamily: _regular),
    IconData(0xe1ca, fontFamily: _bold),
  ),
  sun(
    IconData(0xe472, fontFamily: _regular),
    IconData(0xe472, fontFamily: _bold),
  ),
  moon(
    IconData(0xe330, fontFamily: _regular),
    IconData(0xe330, fontFamily: _bold),
  ),
  screen(
    IconData(0xe58a, fontFamily: _regular),
    IconData(0xe58a, fontFamily: _bold),
  ),
  screenOff(
    IconData(0xe32e, fontFamily: _regular),
    IconData(0xe32e, fontFamily: _bold),
    slashed: true,
  ),
  folder(
    IconData(0xe25a, fontFamily: _regular),
    IconData(0xe25a, fontFamily: _bold),
  ),
  fullScreen(
    IconData(0xe1d0, fontFamily: _regular),
    IconData(0xe1d0, fontFamily: _bold),
  ),
  fullScreenExit(
    IconData(0xe1ce, fontFamily: _regular),
    IconData(0xe1ce, fontFamily: _bold),
  ),
  volume(
    IconData(0xe44a, fontFamily: _regular),
    IconData(0xe44a, fontFamily: _bold),
  ),
  volumeOff(
    IconData(0xe45a, fontFamily: _regular),
    IconData(0xe45a, fontFamily: _bold),
  ),
  headphones(
    IconData(0xe2a6, fontFamily: _regular),
    IconData(0xe2a6, fontFamily: _bold),
  ),
  headphonesOff(
    IconData(0xe2a6, fontFamily: _regular),
    IconData(0xe2a6, fontFamily: _bold),
    slashed: true,
  ),
  chevronDown(
    IconData(0xe136, fontFamily: _regular),
    IconData(0xe136, fontFamily: _bold),
  ),
  chevronLeft(
    IconData(0xe138, fontFamily: _regular),
    IconData(0xe138, fontFamily: _bold),
  ),
  chevronRight(
    IconData(0xe13a, fontFamily: _regular),
    IconData(0xe13a, fontFamily: _bold),
  ),
  minimize(
    IconData(0xe32a, fontFamily: _regular),
    IconData(0xe32a, fontFamily: _bold),
  ),
  maximize(
    IconData(0xe45e, fontFamily: _regular),
    IconData(0xe45e, fontFamily: _bold),
  ),
  restore(
    IconData(0xe1cc, fontFamily: _regular),
    IconData(0xe1cc, fontFamily: _bold),
  ),
  pencil(
    IconData(0xe3b4, fontFamily: _regular),
    IconData(0xe3b4, fontFamily: _bold),
  ),
  image(
    IconData(0xe2ca, fontFamily: _regular),
    IconData(0xe2ca, fontFamily: _bold),
  ),
  back(
    IconData(0xe058, fontFamily: _regular),
    IconData(0xe058, fontFamily: _bold),
  ),
  phone(
    IconData(0xe3b8, fontFamily: _regular),
    IconData(0xe3b8, fontFamily: _bold),
  ),
  hangUp(
    IconData(0xe3bc, fontFamily: _regular),
    IconData(0xe3bc, fontFamily: _bold),
  ),
  phoneIncoming(
    IconData(0xe3be, fontFamily: _regular),
    IconData(0xe3be, fontFamily: _bold),
  ),
  missedCall(
    IconData(0xe3c4, fontFamily: _regular),
    IconData(0xe3c4, fontFamily: _bold),
  ),
  bell(
    IconData(0xe0ce, fontFamily: _regular),
    IconData(0xe0ce, fontFamily: _bold),
  ),
  trash(
    IconData(0xe4a6, fontFamily: _regular),
    IconData(0xe4a6, fontFamily: _bold),
  ),
  userPlus(
    IconData(0xe4d0, fontFamily: _regular),
    IconData(0xe4d0, fontFamily: _bold),
  ),
  play(
    IconData(0xe3d0, fontFamily: _regular),
    IconData(0xe3d0, fontFamily: _bold),
  ),
  pause(
    IconData(0xe39e, fontFamily: _regular),
    IconData(0xe39e, fontFamily: _bold),
  ),
  timer(
    IconData(0xe492, fontFamily: _regular),
    IconData(0xe492, fontFamily: _bold),
  ),
  pin(
    IconData(0xe3e2, fontFamily: _regular),
    IconData(0xe3e2, fontFamily: _bold),
    shift: Offset(-1 / 32, 1 / 32),
  ),
  unpin(
    IconData(0xe3e4, fontFamily: _regular),
    IconData(0xe3e4, fontFamily: _bold),
    shift: Offset(-1 / 32, 1 / 32),
  ),
  fileX(
    IconData(0xe23c, fontFamily: _regular),
    IconData(0xe23c, fontFamily: _bold),
  ),
  user(
    IconData(0xe4c2, fontFamily: _regular),
    IconData(0xe4c2, fontFamily: _bold),
  ),
  palette(
    IconData(0xe6c8, fontFamily: _regular),
    IconData(0xe6c8, fontFamily: _bold),
  ),
  appWindow(
    IconData(0xe5da, fontFamily: _regular),
    IconData(0xe5da, fontFamily: _bold),
  ),
  globe(
    IconData(0xe288, fontFamily: _regular),
    IconData(0xe288, fontFamily: _bold),
  ),
  translate(
    IconData(0xe4a2, fontFamily: _regular),
    IconData(0xe4a2, fontFamily: _bold),
  ),
  hardDrives(
    IconData(0xe2a0, fontFamily: _regular),
    IconData(0xe2a0, fontFamily: _bold),
  ),
  smiley(
    IconData(0xe436, fontFamily: _regular),
    IconData(0xe436, fontFamily: _bold),
  ),
  history(
    IconData(0xe1a0, fontFamily: _regular),
    IconData(0xe1a0, fontFamily: _bold),
  ),
  leaf(
    IconData(0xe2da, fontFamily: _regular),
    IconData(0xe2da, fontFamily: _bold),
  ),
  forkKnife(
    IconData(0xe262, fontFamily: _regular),
    IconData(0xe262, fontFamily: _bold),
  ),
  ball(
    IconData(0xe20e, fontFamily: _regular),
    IconData(0xe20e, fontFamily: _bold),
  ),
  airplane(
    IconData(0xe002, fontFamily: _regular),
    IconData(0xe002, fontFamily: _bold),
  ),
  lightbulb(
    IconData(0xe2dc, fontFamily: _regular),
    IconData(0xe2dc, fontFamily: _bold),
  ),
  flag(
    IconData(0xe244, fontFamily: _regular),
    IconData(0xe244, fontFamily: _bold),
  );

  const Ic(
    this.regular,
    this.bold, {
    this.slashed = false,
    this.shift = Offset.zero,
  });

  // Constant, so release builds keep only the glyphs the app uses.
  final IconData regular;
  final IconData bold;

  /// A slash across the icon: the "off" state of one that has no own glyph for it.
  final bool slashed;

  /// How far the glyph is moved, in parts of its size, so its drawing sits in the middle
  /// of its box: a few, like the pin, are drawn off centre in the font.
  final Offset shift;
}

/// The icons a group can have, with their names.
List<(Ic, String)> get groupIcons => [
  (Ic.mountain, l10n.iconsGroupMountain),
  (Ic.star, l10n.iconsGroupStar),
  (Ic.gamepad, l10n.iconsGroupGameController),
  (Ic.music, l10n.iconsGroupMusic),
  (Ic.book, l10n.iconsGroupBook),
  (Ic.waves, l10n.iconsGroupWaves),
  (Ic.coffee, l10n.iconsGroupCoffee),
  (Ic.heart, l10n.iconsGroupHeart),
  (Ic.film, l10n.iconsGroupFilm),
];

Ic groupIcon(int index, {bool small = false}) {
  final icon = groupIcons[index < groupIcons.length ? index : 0].$1;
  return small && icon == Ic.gamepad ? Ic.gamepadSmall : icon;
}

/// An icon of the design. [stroke] of 2.4 or more picks the bold weight.
class FgIcon extends StatelessWidget {
  const FgIcon(this.icon, {super.key, this.size = 16, this.color, this.stroke});

  final Ic icon;
  final double size;
  final Color? color;
  final double? stroke;

  @override
  Widget build(BuildContext context) {
    final color = this.color ?? DefaultTextStyle.of(context).style.color!;
    final bold = (stroke ?? 0) >= 2.4;
    Widget glyph = Icon(
      bold ? icon.bold : icon.regular,
      size: size,
      color: color,
    );
    if (icon.shift != Offset.zero) {
      glyph = Transform.translate(offset: icon.shift * size, child: glyph);
    }
    if (icon.slashed) {
      glyph = Stack(
        alignment: Alignment.center,
        children: [
          glyph,
          Transform.rotate(
            angle: -math.pi / 4,
            child: Container(
              width: math.max(1.5, size * .09),
              height: size * 1.04,
              decoration: BoxDecoration(
                color: color,
                borderRadius: BorderRadius.circular(size),
              ),
            ),
          ),
        ],
      );
    }
    return SizedBox(width: size, height: size, child: glyph);
  }
}
