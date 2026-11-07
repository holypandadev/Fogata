import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/theme.dart';

/// The contrast ratio of two opaque colours, as WCAG works it out.
double contrast(Color a, Color b) {
  final la = a.computeLuminance();
  final lb = b.computeLuminance();
  return (math.max(la, lb) + .05) / (math.min(la, lb) + .05);
}

void main() {
  test('every theme reads at 4.5:1 or more, and its controls at 3:1', () {
    for (final look in Look.values) {
      final c = look.tokens;
      final surfaces = {
        'bg': c.bg,
        'bg1': c.bg1,
        'bg2': c.bg2,
        'hover': c.hover,
        'tile': c.tile,
      };
      final texts = {
        'fg': c.fg,
        'fg2': c.fg2,
        'fg3': c.fg3,
        'green': c.green,
        'red': c.red,
        'amber': c.amber,
        'blue': c.blue,
      };
      for (final MapEntry(key: text, value: color) in texts.entries) {
        for (final MapEntry(key: surface, value: under) in surfaces.entries) {
          expect(
            contrast(color, under),
            greaterThanOrEqualTo(4.5),
            reason: '${look.name}: $text on $surface',
          );
        }
      }
      // The chosen part of a segmented control, a pill over the window, and text on the
      // main button.
      for (final MapEntry(key: text, value: color) in texts.entries) {
        expect(
          contrast(color, c.raised),
          greaterThanOrEqualTo(4.5),
          reason: '${look.name}: $text on raised',
        );
      }
      expect(contrast(c.onPri, c.pri), greaterThanOrEqualTo(4.5));
      expect(contrast(Colors.white, greenSolid), greaterThanOrEqualTo(4.5));
      // Words in a colour on that colour's soft background: the pills and notes.
      for (final MapEntry(key: surface, value: under) in surfaces.entries) {
        for (final (name, color, soft) in [
          ('green', c.green, c.greenSoft),
          ('amber', c.amber, c.amberSoft),
          ('red', c.red, c.redSoft),
          ('blue', c.blue, c.blueSoft),
        ]) {
          expect(
            contrast(color, Color.alphaBlend(soft, under)),
            greaterThanOrEqualTo(4.5),
            reason: '${look.name}: $name on its soft background on $surface',
          );
        }
        // What a control is seen by (a field's edge, a box not ticked, a switch that is
        // off), the focus ring and the status dots: 3:1, as WCAG asks of controls.
        for (final (name, color) in [
          ('edge', c.edge),
          ('online dot', c.green),
          ('away dot', c.amber),
          ('offline dot', c.fg3),
        ]) {
          expect(
            contrast(color, under),
            greaterThanOrEqualTo(3),
            reason: '${look.name}: $name on $surface',
          );
        }
      }
      // A control's edge never fainter than the strong border it took the place of, and
      // the knob of a switch that is off seen on its track.
      for (final MapEntry(key: surface, value: under) in surfaces.entries) {
        expect(
          contrast(c.edge, under),
          greaterThanOrEqualTo(contrast(c.borderStrong, under)),
          reason: '${look.name}: edge against strong border on $surface',
        );
      }
      expect(
        contrast(c.switchKnobOff, c.edge),
        greaterThanOrEqualTo(3),
        reason: '${look.name}: knob of a switch that is off',
      );
      // The focus ring, drawn on the edge of what has the focus: seen against the
      // surfaces and the main button's fill.
      for (final MapEntry(key: surface, value: under) in {
        'bg': c.bg,
        'bg1': c.bg1,
        'bg2': c.bg2,
        'pri': c.pri,
      }.entries) {
        expect(
          contrast(c.blueSolid, under),
          greaterThanOrEqualTo(3),
          reason: '${look.name}: focus ring on $surface',
        );
      }
      expect(contrast(Colors.white, c.redSolid), greaterThanOrEqualTo(4.5));
      expect(contrast(Colors.white, c.blueSolid), greaterThanOrEqualTo(4.5));
      // Lines are seen against the surfaces they part.
      expect(
        contrast(c.borderStrong, c.bg),
        greaterThanOrEqualTo(1.4),
        reason: '${look.name}: strong border',
      );
    }
  });

  test(
    'the saved name of a theme comes back to it, and unknown ones are Dark',
    () {
      for (final look in Look.values) {
        expect(Look.named(look.name), look);
      }
      expect(Look.named(null), Look.dark);
      expect(Look.named('neon'), Look.dark);
      // Names saved before there were more themes.
      expect(Look.named('light').mode, ThemeMode.light);
      expect(Look.named('system').mode, ThemeMode.system);
      expect(Look.named('dark').mode, ThemeMode.dark);
      expect(Look.midnight.mode, ThemeMode.dark);
    },
  );
}
