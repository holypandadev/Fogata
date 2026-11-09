import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/moving.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

/// Three frames of 8 by 8 pixels, red, green and blue, 50 ms each.
final _gif = base64Decode(
  'R0lGODlhCAAIAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQABQAAACwAAAAACAAIAAAIDwAB'
  'CBxIsKDBgwgTKkwYEAAh+QQBBQABACwAAAAACAAIAIEA/wAAAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAAh+QQB'
  'BQABACwAAAAACAAIAIEAAP8AAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAA7',
);

/// A PNG's first bytes: a picture that does not move.
final _png = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
);

Future<void> _show(WidgetTester tester, Widget child) => tester.pumpWidget(
  MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Scaffold(body: Center(child: child)),
  ),
);

/// Lets the frames be read and drawn, outside the test's own time.
Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 60)),
    );
    await tester.pump();
  }
}

void main() {
  testWidgets(
    'a group picture that moves plays in a square with round corners',
    (tester) async {
      // The group's page: a square with round corners, smaller than a large avatar.
      await _show(
        tester,
        GroupBadge(icon: 0, picture: _gif, size: 56, radius: 14, play: true),
      );
      final moving = tester.widget<MovingPicture>(find.byType(MovingPicture));
      expect(moving.round, isFalse);
      expect(moving.corner, 14 / 56);
      await _settle(tester);
      expect(tester.takeException(), isNull);
      // Asked not to, only a large round one plays, as before.
      await _show(
        tester,
        GroupBadge(icon: 0, picture: _gif, size: 56, radius: 14),
      );
      expect(find.byType(MovingPicture), findsNothing);
      await _show(
        tester,
        GroupBadge(icon: 0, picture: _gif, size: 56, radius: 28),
      );
      expect(find.byType(MovingPicture), findsOneWidget);
    },
  );

  testWidgets('a small round group picture plays, and stops when told to', (
    tester,
  ) async {
    // The call's bar.
    await _show(tester, MovingGroupPicture(bytes: _gif, size: 36, radius: 18));
    final moving = tester.widget<MovingPicture>(find.byType(MovingPicture));
    expect(moving.round, isTrue);
    expect(moving.corner, isNull);
    await _settle(tester);
    expect(tester.takeException(), isNull);
    // The rail: still until the pointer is on it.
    await _show(
      tester,
      MovingGroupPicture(bytes: _gif, size: 46, radius: 23, play: false),
    );
    expect(find.byType(MovingPicture), findsNothing);
    expect(find.byType(RoundedPicture), findsOneWidget);
  });

  testWidgets('a group picture that does not move is never played', (
    tester,
  ) async {
    await _show(tester, MovingGroupPicture(bytes: _png, size: 56, radius: 14));
    expect(find.byType(MovingPicture), findsNothing);
    expect(find.byType(RoundedPicture), findsOneWidget);
  });
}
