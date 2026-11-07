// The panel to add a friend makes a code once its first frame is drawn. Gone before that
// frame ends, it makes none and asks nothing of what it was in.

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/friends.dart';
import 'package:fogata/src/theme.dart';

class _Core implements RustLibApi {
  int codes = 0;

  @override
  Future<UiInvitation> crateApiAppFriendCode() async {
    codes++;
    return UiInvitation(
      text: '1234-5678',
      expiresMs: DateTime.now().millisecondsSinceEpoch + 600000,
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

/// Lays its child out twice in one frame: first as wide as it may be, then 10 pixels
/// wide. A [LayoutBuilder] under it builds for each, so what it builds for the first can be
/// gone by the end of the same frame.
class _TwiceLaidOut extends SingleChildRenderObjectWidget {
  const _TwiceLaidOut({required super.child});

  @override
  RenderObject createRenderObject(BuildContext context) => _RenderTwice();
}

class _RenderTwice extends RenderProxyBox {
  @override
  void performLayout() {
    child!.layout(constraints, parentUsesSize: true);
    child!.layout(constraints.copyWith(minWidth: 0, maxWidth: 10));
    size = constraints.biggest;
  }
}

void main() {
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));

  testWidgets('the panel gone in the frame it came makes no code', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-add-friend-gone-test');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Scaffold(
            body: _TwiceLaidOut(
              child: LayoutBuilder(
                builder: (context, box) =>
                    box.maxWidth > 100 ? const Friends() : const SizedBox(),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(tester.takeException(), isNull);
    expect(find.byType(Friends), findsNothing);
    expect(core.codes, 0);
  });

  testWidgets('the panel that stays makes its code', (tester) async {
    tester.view.physicalSize = const Size(1440, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-add-friend-gone-test');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Scaffold(body: Friends()),
        ),
      ),
    );
    await tester.pump();
    expect(core.codes, 1);
    await tester.pumpWidget(const SizedBox());
  });
}
