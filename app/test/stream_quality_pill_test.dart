import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'a watched picture’s figures fit, the long name of a member too',
    (tester) async {
      for (final theme in [Fg.darkTokens, Fg.lightTokens]) {
        await tester.pumpWidget(
          MaterialApp(
            theme: theme.themeData,
            home: const Material(
              child: Align(
                alignment: Alignment.topRight,
                child: StreamQualityPill(
                  quality: StreamQuality(
                    level: 2,
                    delayMs: 6,
                    lossPercent: 0,
                    width: 1600,
                    height: 900,
                    fps: 3,
                  ),
                  path: UiPath.throughMember,
                  through: 'Ana Isabel Montenegro de la Vega Ramírez',
                  open: true,
                ),
              ),
            ),
          ),
        );
        await tester.pump(const Duration(seconds: 1));
        expect(tester.takeException(), isNull);
        // The frame rate says what it is.
        expect(find.text('1600×900 · 3 fps'), findsOneWidget);
        expect(find.text('900p · 3 fps'), findsOneWidget);
        final path = find.textContaining('Through Ana Isabel');
        expect(path, findsOneWidget);
        expect(tester.getSize(path).width, lessThan(216));
      }
    },
  );
}
