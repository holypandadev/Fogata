import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/screens/group_banner.dart';
import 'package:fogata/src/theme.dart';

/// Three frames of 8 by 8 pixels.
final _gif = base64Decode(
  'R0lGODlhCAAIAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQABQAAACwAAAAACAAIAAAIDwAB'
  'CBxIsKDBgwgTKkwYEAAh+QQBBQABACwAAAAACAAIAIEA/wAAAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAAh+QQB'
  'BQABACwAAAAACAAIAIEAAP8AAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAA7',
);

Future<void> _show(WidgetTester tester, Widget child) => tester.pumpWidget(
  MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Scaffold(
      body: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(width: 240, child: child),
      ),
    ),
  ),
);

void main() {
  testWidgets('without a banner the header is the bar it always was', (
    tester,
  ) async {
    var opened = 0;
    await _show(
      tester,
      ColumnHeader(title: 'Tests', onSettings: () => opened++),
    );
    expect(tester.getSize(find.byType(ColumnHeader)).height, 48);
    expect(find.byKey(const ValueKey('group-banner')), findsNothing);
    await tester.tap(find.byTooltip('Group settings'));
    expect(opened, 1);
  });

  testWidgets('with a banner the name and the button sit over its top', (
    tester,
  ) async {
    var opened = 0;
    await _show(
      tester,
      ColumnHeader(title: 'Tests', banner: _gif, onSettings: () => opened++),
    );
    await tester.pump();
    expect(tester.getSize(find.byType(ColumnHeader)).height, groupBannerHeight);
    expect(find.byKey(const ValueKey('group-banner')), findsOneWidget);
    final title = tester.getRect(find.text('Tests'));
    expect(title.top, lessThan(48), reason: 'in the top band');
    await tester.tap(find.byTooltip('Group settings'));
    expect(opened, 1);
    await tester.pumpWidget(const SizedBox());
  });
}
