import 'package:flutter/material.dart';

import '../app_state.dart';
import '../l10n.dart';
import '../theme.dart';
import '../widgets.dart';

/// The window's themes side by side, each a small picture of the window in its colours;
/// the chosen one has a ring around it.
class ThemePicker extends StatelessWidget {
  const ThemePicker({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    return Wrap(
      spacing: 14,
      runSpacing: 14,
      children: [
        for (final look in Look.values)
          _ThemeCard(
            look: look,
            chosen: state.prefs.look == look,
            onTap: () => state.setLook(look),
          ),
      ],
    );
  }
}

class _ThemeCard extends StatelessWidget {
  const _ThemeCard({
    required this.look,
    required this.chosen,
    required this.onTap,
  });

  final Look look;
  final bool chosen;
  final VoidCallback onTap;

  static const _width = 112.0;
  static const _height = 72.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Springy(
      key: ValueKey('theme:${look.name}'),
      onTap: onTap,
      hoverScale: 1.03,
      focusRadius: 14,
      selected: chosen,
      label: look == Look.system
          ? l10n.themePickerSystem
          : l10n.themePickerTheme(look.label),
      builder: (context, hovered) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          AnimatedContainer(
            duration: springTime,
            curve: spring,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              // A ring with a gap, the same as around a chosen avatar.
              boxShadow: chosen
                  ? [
                      BoxShadow(color: c.fg, spreadRadius: 4),
                      BoxShadow(color: c.bg1, spreadRadius: 2),
                    ]
                  : null,
            ),
            child: Container(
              width: _width,
              height: _height,
              clipBehavior: Clip.antiAlias,
              // Inside the outline, as when it was drawn under what is inside.
              padding: const EdgeInsets.all(1),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(14),
              ),
              // Over what is inside, which would cover it at the rounded corners.
              foregroundDecoration: BoxDecoration(
                borderRadius: BorderRadius.circular(14),
                border: Border.all(
                  color: hovered && !chosen ? c.borderStrong : c.border,
                ),
              ),
              child: CustomPaint(
                painter: _Preview(
                  look == Look.system ? Fg.darkTokens : look.tokens,
                  right: look == Look.system ? Fg.lightTokens : null,
                ),
              ),
            ),
          ),
          const SizedBox(height: 8),
          Text(
            look.label,
            style: tx(chosen || hovered ? c.fg : c.fg2, size: 13, weight: w500),
          ),
        ],
      ),
    );
  }
}

/// The window drawn small in a theme's colours: the side column, a message and a green
/// button. With [right], its right half is drawn in those other colours.
class _Preview extends CustomPainter {
  _Preview(this.left, {this.right});

  final Fg left;
  final Fg? right;

  @override
  void paint(Canvas canvas, Size size) {
    _draw(canvas, size, left);
    final other = right;
    if (other != null) {
      canvas.save();
      canvas.clipRect(
        Rect.fromLTRB(size.width / 2, 0, size.width, size.height),
      );
      _draw(canvas, size, other);
      canvas.restore();
    }
  }

  void _draw(Canvas canvas, Size size, Fg c) {
    final paint = Paint();
    canvas.drawRect(Offset.zero & size, paint..color = c.bg);
    // The side column.
    final side = Rect.fromLTWH(0, 0, 34, size.height);
    canvas.drawRect(side, paint..color = c.bg1);
    canvas.drawRect(
      Rect.fromLTWH(side.right, 0, 1, size.height),
      paint..color = c.border,
    );
    for (var i = 0; i < 3; i++) {
      final row = RRect.fromRectAndRadius(
        Rect.fromLTWH(6, 10 + i * 12.0, 22, 7),
        const Radius.circular(3.5),
      );
      canvas.drawRRect(row, paint..color = i == 0 ? c.hover : c.bg1);
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(9, 12.5 + i * 12.0, i == 0 ? 14 : 11, 2),
          const Radius.circular(1),
        ),
        paint..color = i == 0 ? c.fg : c.fg3,
      );
    }
    // A message: the avatar's dot and two lines of text.
    canvas.drawCircle(const Offset(48, 17), 5, paint..color = c.bg2);
    canvas.drawCircle(
      const Offset(48, 17),
      5,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = c.borderStrong,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        const Rect.fromLTWH(58, 13, 30, 3),
        const Radius.circular(1.5),
      ),
      paint..color = c.fg,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        const Rect.fromLTWH(58, 19, 42, 3),
        const Radius.circular(1.5),
      ),
      paint..color = c.fg2,
    );
    // A panel with a green pill in it.
    final panel = RRect.fromRectAndRadius(
      Rect.fromLTWH(42, 32, size.width - 50, size.height - 40),
      const Radius.circular(6),
    );
    canvas.drawRRect(panel, paint..color = c.bg2);
    canvas.drawRRect(
      panel,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = c.border,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(48, 40, 26, 9),
        const Radius.circular(4.5),
      ),
      paint..color = c.greenSoft,
    );
    canvas.drawCircle(const Offset(53, 44.5), 2, paint..color = c.green);
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(48, 54, 36, 3),
        const Radius.circular(1.5),
      ),
      paint..color = c.fg3,
    );
  }

  @override
  bool shouldRepaint(_Preview old) => old.left != left || old.right != right;
}
