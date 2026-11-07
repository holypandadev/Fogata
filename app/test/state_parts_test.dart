// The state is told apart in parts: a widget that watches some parts builds again when one
// of those changes, and not when another one does; one that depends on the whole state
// builds again on any change.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/direct_state.dart';

/// Counts its builds; [watch] picks what it depends on.
class _Counted extends StatelessWidget {
  const _Counted(this.watch, this.builds);
  final void Function(BuildContext) watch;
  final List<int> builds;

  @override
  Widget build(BuildContext context) {
    watch(context);
    builds[0]++;
    return const SizedBox();
  }
}

void main() {
  testWidgets('a part tells only what watches it; the whole state tells all', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-parts-test');
    final whole = [0];
    final chats = [0];
    final core = [0];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: Column(
          children: [
            _Counted(AppScope.of, whole),
            _Counted(AppScope.watch<DirectChats>, chats),
            _Counted(AppScope.watch<AppState>, core),
          ],
        ),
      ),
    );
    expect([whole[0], chats[0], core[0]], [1, 1, 1]);

    state.direct.changed();
    await tester.pump();
    expect([whole[0], chats[0], core[0]], [2, 2, 1]);

    state.notifyListeners();
    await tester.pump();
    expect([whole[0], chats[0], core[0]], [3, 3, 2]);

    // A setting changed by the person tells what shows the settings, not the chats.
    await tester.runAsync(() => state.setTray(false));
    await tester.pump();
    expect([whole[0], chats[0], core[0]], [4, 3, 2]);

    // A toast tells only the toasts.
    final toasts = [0];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: Column(
          children: [
            _Counted(AppScope.of, whole),
            _Counted(AppScope.watch<ToastBoard>, toasts),
          ],
        ),
      ),
    );
    final before = whole[0];
    state.say('Saved');
    await tester.pump();
    expect([whole[0] - before, toasts[0]], [0, 2]);
    expect(state.toasts.list.single.text, 'Saved');

    // What the room's call tells as it goes (someone connects, mutes, shares) reaches
    // what watches it, not what watches the calls, the chats or the whole state.
    final live = [0];
    final calls = [0];
    chats[0] = 0;
    core[0] = 0;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: Column(
          children: [
            _Counted(AppScope.watch<LiveCall>, live),
            _Counted(AppScope.watch<CallState>, calls),
            _Counted(AppScope.watch<DirectChats>, chats),
            _Counted(AppScope.watch<AppState>, core),
          ],
        ),
      ),
    );
    final call = Call(group: 'g', me: 'me', mic: true);
    state.callState.call = call;
    await tester.pump();
    expect([live[0], calls[0], chats[0], core[0]], [2, 2, 1, 1]);
    call.notifyListeners();
    await tester.pump();
    expect([live[0], calls[0], chats[0], core[0]], [3, 2, 1, 1]);
    state.callState.call = null;
    await tester.pump();
    call.notifyListeners();
    await tester.pump();
    expect([live[0], calls[0], chats[0], core[0]], [4, 3, 1, 1]);

    await tester.pumpWidget(const SizedBox());
    state.dispose();
  });
}
