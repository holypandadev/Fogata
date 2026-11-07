import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/new_group.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// Keeps what the window asks of the core, in order.
class _Groups extends AppState {
  _Groups() : super('/nonexistent/fogata-new-group-test');

  final asked = <String>[];
  Uint8List? picture;

  @override
  Future<String?> createGroup(
    String name,
    int icon,
    List<String> friendIds,
  ) async {
    asked.add('create $name $icon');
    return 'g1';
  }

  @override
  Future<bool> setGroupPicture(String group, Uint8List? picture) async {
    asked.add('picture $group');
    this.picture = picture;
    return true;
  }
}

Future<_Groups> _open(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = _Groups();
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Builder(
            builder: (context) => TextButton(
              onPressed: () => showNewGroup(context),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pump(const Duration(milliseconds: 500));
  await tester.enterText(find.byType(TextField).first, 'Sunday ride');
  await tester.pump();
  return state;
}

GroupBadge _badge(WidgetTester tester) =>
    tester.widget<GroupBadge>(find.byType(GroupBadge));

void main() {
  setUpAll(loadFonts);

  final picked = Uint8List.fromList(List.filled(64, 7));
  setUp(() => pickGroupPicture = () async => picked);

  testWidgets('a picture from the computer is set once the group is made', (
    tester,
  ) async {
    final state = await _open(tester);
    await tester.tap(find.byTooltip('Choose from computer'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(_badge(tester).picture, picked);
    await tester.tap(find.text('Create group'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.asked, ['create Sunday ride 0', 'picture g1']);
    expect(state.picture, picked);
    // The window closes.
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('Create group'), findsNothing);
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('an icon chosen after it takes the picture’s place', (
    tester,
  ) async {
    final state = await _open(tester);
    await tester.tap(find.byTooltip('Choose from computer'));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.byTooltip('Star'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(_badge(tester).picture, isNull);
    expect(_badge(tester).icon, 1);
    await tester.tap(find.text('Create group'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.asked, ['create Sunday ride 1']);
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('no picture chosen, the window asks for none', (tester) async {
    pickGroupPicture = () async => null;
    final state = await _open(tester);
    await tester.tap(find.byTooltip('Choose from computer'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(_badge(tester).picture, isNull);
    await tester.tap(find.text('Create group'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(state.asked, ['create Sunday ride 0']);
    await tester.pump(const Duration(seconds: 10));
  });
}
