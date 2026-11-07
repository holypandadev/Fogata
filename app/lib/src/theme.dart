import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'host.dart';
import 'l10n.dart';

/// The design's colour tokens, for each of the window's themes.
@immutable
class Fg extends ThemeExtension<Fg> {
  const Fg({
    required this.dark,
    required this.bg,
    required this.bg1,
    required this.bg2,
    required this.hover,
    required this.border,
    required this.borderStrong,
    required this.fg,
    required this.fg2,
    required this.fg3,
    required this.pri,
    required this.onPri,
    required this.blue,
    required this.blueSolid,
    required this.blueSoft,
    required this.green,
    required this.greenSoft,
    required this.greenLine,
    required this.amber,
    required this.amberSoft,
    required this.amberLine,
    required this.red,
    required this.redSolid,
    required this.redSoft,
    required this.redLine,
    required this.tile,
    required this.scrim,
    required this.grid,
    required this.raised,
    required this.edge,
  });

  final bool dark;
  final Color bg, bg1, bg2, hover, border, borderStrong;
  final Color fg, fg2, fg3, pri, onPri;
  final Color blue, blueSolid, blueSoft;
  final Color green, greenSoft, greenLine;
  final Color amber, amberSoft, amberLine;
  final Color red, redSolid, redSoft, redLine;
  final Color tile, scrim, grid;

  /// What stands out of a surface: the chosen part of a segmented control, a chosen
  /// emoji, a pill over the window.
  final Color raised;

  /// The knob of a switch that is off: white, or the window's background where the off
  /// track ([edge]) is too light for white to stand out from it at 3:1.
  Color get switchKnobOff =>
      contrastOf(Colors.white, edge) >= 3 ? Colors.white : bg;

  /// The edge of a control that has to be seen to be used: a field, a box not ticked, a
  /// switch that is off. At least 3:1 against the surfaces, as WCAG asks of controls,
  /// where [border] only parts surfaces.
  final Color edge;

  static const darkTokens = Fg(
    dark: true,
    bg: Color(0xFF070707),
    bg1: Color(0xFF0A0A0A),
    bg2: Color(0xFF111111),
    hover: Color(0xFF1A1A1A),
    border: Color(0xFF242424),
    borderStrong: Color(0xFF3A3A3A),
    fg: Color(0xFFEDEDED),
    fg2: Color(0xFFA1A1A1),
    fg3: Color(0xFF8E8E8E),
    pri: Color(0xFFEDEDED),
    onPri: Color(0xFF0A0A0A),
    blue: Color(0xFF52A8FF),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .16),
    green: Color(0xFF3DD68C),
    greenSoft: Color.fromRGBO(61, 214, 140, .12),
    greenLine: Color.fromRGBO(61, 214, 140, .38),
    amber: Color(0xFFFFB224),
    amberSoft: Color.fromRGBO(255, 178, 36, .10),
    amberLine: Color.fromRGBO(255, 178, 36, .36),
    red: Color(0xFFFF6166),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(255, 97, 102, .10),
    redLine: Color.fromRGBO(255, 97, 102, .36),
    tile: Color(0xFF0F0F0F),
    scrim: Color.fromRGBO(0, 0, 0, .66),
    grid: Color.fromRGBO(255, 255, 255, .07),
    raised: Color(0xFF262626),
    edge: Color(0xFF6B6B6B),
  );

  static const lightTokens = Fg(
    dark: false,
    bg: Color(0xFFFAFAFA),
    bg1: Color(0xFFFFFFFF),
    bg2: Color(0xFFFAFAFA),
    hover: Color(0xFFF2F2F2),
    border: Color(0xFFEAEAEA),
    borderStrong: Color(0xFFD4D4D4),
    fg: Color(0xFF171717),
    fg2: Color(0xFF666666),
    fg3: Color(0xFF6E6E6E),
    pri: Color(0xFF171717),
    onPri: Color(0xFFFFFFFF),
    blue: Color(0xFF0060D1),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .08),
    green: Color(0xFF0F762F),
    greenSoft: Color.fromRGBO(16, 125, 50, .08),
    greenLine: Color.fromRGBO(16, 125, 50, .30),
    amber: Color(0xFFA35200),
    amberSoft: Color.fromRGBO(255, 178, 36, .14),
    amberLine: Color.fromRGBO(204, 120, 0, .35),
    red: Color(0xFFC0272C),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(218, 47, 53, .07),
    redLine: Color.fromRGBO(218, 47, 53, .30),
    tile: Color(0xFFF2F2F2),
    scrim: Color.fromRGBO(0, 0, 0, .32),
    grid: Color.fromRGBO(0, 0, 0, .06),
    raised: Color(0xFFFFFFFF),
    edge: Color(0xFF8A8A8A),
  );

  /// Dark with true black surfaces, which screens that light each pixel turn off.
  static const midnightTokens = Fg(
    dark: true,
    bg: Color(0xFF000000),
    bg1: Color(0xFF050505),
    bg2: Color(0xFF0C0C0C),
    hover: Color(0xFF161616),
    border: Color(0xFF1F1F1F),
    borderStrong: Color(0xFF363636),
    fg: Color(0xFFEDEDED),
    fg2: Color(0xFFA1A1A1),
    fg3: Color(0xFF8C8C8C),
    pri: Color(0xFFEDEDED),
    onPri: Color(0xFF000000),
    blue: Color(0xFF52A8FF),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .16),
    green: Color(0xFF3DD68C),
    greenSoft: Color.fromRGBO(61, 214, 140, .12),
    greenLine: Color.fromRGBO(61, 214, 140, .38),
    amber: Color(0xFFFFB224),
    amberSoft: Color.fromRGBO(255, 178, 36, .10),
    amberLine: Color.fromRGBO(255, 178, 36, .36),
    red: Color(0xFFFF6166),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(255, 97, 102, .10),
    redLine: Color.fromRGBO(255, 97, 102, .36),
    tile: Color(0xFF0A0A0A),
    scrim: Color.fromRGBO(0, 0, 0, .72),
    grid: Color.fromRGBO(255, 255, 255, .07),
    raised: Color(0xFF222222),
    edge: Color(0xFF666666),
  );

  /// Dark in softer greys, easier on the eyes in a lit room.
  static const dimTokens = Fg(
    dark: true,
    bg: Color(0xFF1C1C1F),
    bg1: Color(0xFF212124),
    bg2: Color(0xFF27272B),
    hover: Color(0xFF303034),
    border: Color(0xFF36363B),
    borderStrong: Color(0xFF4A4A50),
    fg: Color(0xFFEEEEF0),
    fg2: Color(0xFFB4B4BB),
    fg3: Color(0xFFA8A8B0),
    pri: Color(0xFFEEEEF0),
    onPri: Color(0xFF1C1C1F),
    blue: Color(0xFF6BB5FF),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .2),
    green: Color(0xFF4ADE94),
    greenSoft: Color.fromRGBO(74, 222, 148, .13),
    greenLine: Color.fromRGBO(74, 222, 148, .4),
    amber: Color(0xFFFFBA3D),
    amberSoft: Color.fromRGBO(255, 186, 61, .12),
    amberLine: Color.fromRGBO(255, 186, 61, .38),
    red: Color(0xFFFF8488),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(255, 122, 126, .12),
    redLine: Color.fromRGBO(255, 122, 126, .38),
    tile: Color(0xFF242427),
    scrim: Color.fromRGBO(0, 0, 0, .6),
    grid: Color.fromRGBO(255, 255, 255, .07),
    raised: Color(0xFF3C3C41),
    edge: Color(0xFF7C7C82),
  );

  /// Dark with warm, brown-tinted greys, like the light of a fire.
  static const emberTokens = Fg(
    dark: true,
    bg: Color(0xFF0D0B09),
    bg1: Color(0xFF110E0C),
    bg2: Color(0xFF181411),
    hover: Color(0xFF221D18),
    border: Color(0xFF2B241E),
    borderStrong: Color(0xFF443A31),
    fg: Color(0xFFF0EAE2),
    fg2: Color(0xFFB3A89B),
    fg3: Color(0xFF9C9083),
    pri: Color(0xFFF0EAE2),
    onPri: Color(0xFF110E0C),
    blue: Color(0xFF52A8FF),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .16),
    green: Color(0xFF3DD68C),
    greenSoft: Color.fromRGBO(61, 214, 140, .12),
    greenLine: Color.fromRGBO(61, 214, 140, .38),
    amber: Color(0xFFFFB224),
    amberSoft: Color.fromRGBO(255, 178, 36, .10),
    amberLine: Color.fromRGBO(255, 178, 36, .36),
    red: Color(0xFFFF6166),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(255, 97, 102, .10),
    redLine: Color.fromRGBO(255, 97, 102, .36),
    tile: Color(0xFF15120F),
    scrim: Color.fromRGBO(0, 0, 0, .66),
    grid: Color.fromRGBO(255, 236, 214, .07),
    raised: Color(0xFF2F2822),
    edge: Color(0xFF74695F),
  );

  /// Black and white with stronger lines and brighter colours, for text and edges that
  /// stand out more.
  static const contrastTokens = Fg(
    dark: true,
    bg: Color(0xFF000000),
    bg1: Color(0xFF000000),
    bg2: Color(0xFF0A0A0A),
    hover: Color(0xFF1F1F1F),
    border: Color(0xFF6B6B6B),
    borderStrong: Color(0xFF9E9E9E),
    fg: Color(0xFFFFFFFF),
    fg2: Color(0xFFE6E6E6),
    fg3: Color(0xFFCCCCCC),
    pri: Color(0xFFFFFFFF),
    onPri: Color(0xFF000000),
    blue: Color(0xFF85C2FF),
    blueSolid: Color(0xFF0070F3),
    blueSoft: Color.fromRGBO(0, 112, 243, .28),
    green: Color(0xFF5CE6A5),
    greenSoft: Color.fromRGBO(92, 230, 165, .16),
    greenLine: Color.fromRGBO(92, 230, 165, .7),
    amber: Color(0xFFFFC452),
    amberSoft: Color.fromRGBO(255, 196, 82, .14),
    amberLine: Color.fromRGBO(255, 196, 82, .7),
    red: Color(0xFFFF8C90),
    redSolid: Color(0xFFDA2F35),
    redSoft: Color.fromRGBO(255, 140, 144, .14),
    redLine: Color.fromRGBO(255, 140, 144, .7),
    tile: Color(0xFF0A0A0A),
    scrim: Color.fromRGBO(0, 0, 0, .8),
    grid: Color.fromRGBO(255, 255, 255, .12),
    raised: Color(0xFF333333),
    edge: Color(0xFFA6A6A6),
  );

  @override
  Fg copyWith() => this;

  @override
  Fg lerp(Fg? other, double t) => t < .5 || other == null ? this : other;

  /// Made once per set of tokens.
  ThemeData get themeData => _themes[this] ??= _theme();

  static var _themes = Expando<ThemeData>('theme');

  ThemeData _theme() {
    final base = ThemeData(
      brightness: dark ? Brightness.dark : Brightness.light,
      fontFamily: sans,
      fontFamilyFallback: _fallback,
      scaffoldBackgroundColor: bg,
      canvasColor: bg,
      splashFactory: NoSplash.splashFactory,
      highlightColor: Colors.transparent,
      hoverColor: hover,
      focusColor: Colors.transparent,
      textSelectionTheme: TextSelectionThemeData(
        cursorColor: fg,
        selectionColor: blueSoft.withValues(alpha: dark ? .5 : .25),
      ),
      scrollbarTheme: ScrollbarThemeData(
        thumbColor: WidgetStatePropertyAll(borderStrong),
        thickness: const WidgetStatePropertyAll(6),
        radius: const Radius.circular(3),
      ),
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: pri,
          borderRadius: BorderRadius.circular(6),
        ),
        textStyle: TextStyle(
          fontFamily: sans,
          fontFamilyFallback: _fallback,
          fontSize: 12,
          color: onPri,
        ),
        waitDuration: const Duration(milliseconds: 400),
      ),
      extensions: [this],
    );
    return base.copyWith(
      textTheme: base.textTheme.apply(
        fontFamily: sans,
        fontFamilyFallback: _fallback,
        bodyColor: fg,
        displayColor: fg,
      ),
    );
  }
}

/// The contrast ratio of two opaque colours, as WCAG works it out.
double contrastOf(Color a, Color b) {
  final la = a.computeLuminance();
  final lb = b.computeLuminance();
  return (math.max(la, lb) + .05) / (math.min(la, lb) + .05);
}

/// The solid green of buttons and labels with white text on them (Accept, Copied, Ready):
/// deep enough for the text to read at 4.7:1 in both themes.
const greenSolid = Color(0xFF1C844E);

/// The window's themes, as chosen in Settings: [dark] unless another is chosen, [system]
/// following the computer between [dark] and [light].
enum Look {
  dark(Fg.darkTokens),
  midnight(Fg.midnightTokens),
  dim(Fg.dimTokens),
  ember(Fg.emberTokens),
  contrast(Fg.contrastTokens),
  light(Fg.lightTokens),
  system(Fg.darkTokens);

  const Look(this.tokens);

  /// The theme's name, as Settings shows it.
  String get label => switch (this) {
    Look.dark => l10n.themeDark,
    Look.midnight => l10n.themeMidnight,
    Look.dim => l10n.themeDim,
    Look.ember => l10n.themeEmber,
    Look.contrast => l10n.themeContrast,
    Look.light => l10n.themeLight,
    Look.system => l10n.themeSystem,
  };

  /// The tokens drawn while this theme is chosen; for [system], those of its dark side.
  final Fg tokens;

  /// What the window follows: the computer for [system], else this theme's own side.
  ThemeMode get mode => switch (this) {
    Look.system => ThemeMode.system,
    Look.light => ThemeMode.light,
    _ => ThemeMode.dark,
  };

  /// The theme a name saved in the settings stands for; [dark] for none or one this
  /// version does not know, as versions before more themes read every name but `light`
  /// and `system`.
  static Look named(Object? name) =>
      values.firstWhere((l) => l.name == name, orElse: () => Look.dark);
}

/// The interface font: on macOS the system's own (SF Pro), elsewhere Inter, which is
/// drawn after it.
String get sans => onMacOS ? '.AppleSystemUIFont' : 'Inter';
const mono = 'GeistMono';

/// The system's colour emoji font.
String get emojiFont => onMacOS
    ? 'Apple Color Emoji'
    : onWindows
    ? 'Segoe UI Emoji'
    : 'Noto Color Emoji';

/// The system's fonts for Japanese and Chinese, which the interface font lacks: the
/// language in use first, so its characters take their own forms (the same character is
/// drawn differently in Japanese and in Chinese), then the other.
List<String> cjkFonts(String language) {
  final (japanese, chinese) = onMacOS
      ? (const ['Hiragino Sans'], const ['PingFang SC'])
      : onWindows
      ? (
          const ['Yu Gothic UI', 'Meiryo UI', 'Meiryo'],
          const ['Microsoft YaHei UI', 'Microsoft YaHei'],
        )
      : (
          const ['Noto Sans CJK JP', 'Source Han Sans JP'],
          const [
            'Noto Sans CJK SC',
            'Source Han Sans SC',
            'Source Han Sans CN',
            'WenQuanYi Micro Hei',
          ],
        );
  return language == 'ja'
      ? [...japanese, ...chinese]
      : [...chinese, ...japanese];
}

/// What text tries after the interface font: the colour emoji, so ❤️ or ☺️ come in colour
/// rather than from a symbol font, then Japanese and Chinese; then the system's others.
var _fallback = [emojiFont, ...cjkFonts('en')];

/// Orders the fallback fonts for [language] (see [cjkFonts]); the themes are made again.
void useFontsFor(String language) {
  final fallback = [emojiFont, ...cjkFonts(language)];
  if (fallback.join() == _fallback.join()) return;
  _fallback = fallback;
  Fg._themes = Expando<ThemeData>('theme');
}

extension FgContext on BuildContext {
  Fg get fg => Theme.of(this).extension<Fg>()!;
}

/// Text in the design's type scale: the interface font, 14 px, line height 1.5 unless told
/// otherwise.
TextStyle tx(
  Color color, {
  double size = 14,
  FontWeight weight = FontWeight.w400,
  double height = 1.5,
  double spacing = 0,
  bool monospace = false,
  TextDecoration? decoration,
}) => TextStyle(
  fontFamily: monospace ? mono : sans,
  fontFamilyFallback: _fallback,
  // Half the leading above and half below, so text sits in the middle of its line.
  leadingDistribution: TextLeadingDistribution.even,
  fontSize: size,
  fontWeight: weight,
  height: height,
  letterSpacing: spacing,
  color: color,
  decoration: decoration,
  decorationColor: color,
);

const w500 = FontWeight.w500;
const w600 = FontWeight.w600;

/// The wordmark's face: Fogata Ember, the app's own, for the name "Fogata" alone
/// ([wordmark]). Headings and everything else are in the interface font.
const display = 'FogataEmber';

/// The wordmark, "Fogata", in its own face.
TextStyle wordmark(Color color, {double size = 14}) => TextStyle(
  fontFamily: display,
  fontFamilyFallback: [sans, ..._fallback],
  leadingDistribution: TextLeadingDistribution.even,
  fontSize: size,
  fontWeight: FontWeight.w800,
  height: 1.5,
  color: color,
);

/// A heading (page titles, the group's name over its column, the voice rooms' names): the
/// interface font in bold, a little tighter as it grows. Characters it lacks (Japanese,
/// Chinese, emoji, a group's name in another script) fall back to the system's fonts, and
/// the line height is the size times [height] whatever the font, so a heading in any script
/// keeps its line.
TextStyle hx(
  Color color, {
  double size = 20,
  double height = 1.5,
  double spacing = 0,
}) => TextStyle(
  fontFamily: sans,
  fontFamilyFallback: _fallback,
  leadingDistribution: TextLeadingDistribution.even,
  fontSize: size,
  fontWeight: FontWeight.w700,
  height: height,
  letterSpacing: spacing != 0 ? spacing : (size >= 24 ? -0.02 * size : 0),
  color: color,
);

/// A preset avatar: a picture shipped with the app, and the colour of its background,
/// shown while the picture loads.
typedef AvatarPreset = ({String image, String name, Color color});

/// The preset avatars, in the order their numbers travel in profiles.
const avatarPresets = <AvatarPreset>[
  (
    image: 'assets/avatars/fogata.jpg',
    name: 'Fogata',
    color: Color(0xFF3D21AD),
  ),
  (image: 'assets/avatars/berly.jpg', name: 'Berly', color: Color(0xFF260E52)),
  (
    image: 'assets/avatars/duckly.jpg',
    name: 'Duckly',
    color: Color(0xFFE7B045),
  ),
  (image: 'assets/avatars/jiri.jpg', name: 'Jiri', color: Color(0xFF5B3014)),
  (image: 'assets/avatars/willy.jpg', name: 'Willy', color: Color(0xFF14110F)),
];

/// The plain colours a person can give the top of their panel, with their names.
List<(Color, String)> get bannerColors => [
  (const Color(0xFF3D21AD), l10n.themeBannerViolet),
  (const Color(0xFF2563EB), l10n.themeBannerBlue),
  (const Color(0xFF0E7C66), l10n.themeBannerGreen),
  (const Color(0xFFB4532A), l10n.themeBannerClay),
  (const Color(0xFFA3325B), l10n.themeBannerRaspberry),
  (const Color(0xFFC99A2E), l10n.themeBannerMustard),
  (const Color(0xFF4B5563), l10n.themeBannerSlate),
  (const Color(0xFF1A1A1A), l10n.themeBannerNight),
];
