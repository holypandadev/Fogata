import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/friends.dart';

const leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB3C·D4EF',
  avatar: UiAvatar(preset: 3),
);

void main() {
  testWidgets('adding a friend to a group that fails says only that', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-add-to-group');
    // There is no core here: the add fails.
    await addFriendToGroup(state, leo, 'g');
    expect(state.toasts.list, hasLength(1));
    expect(state.toasts.list.single.error, isTrue);
    expect(
      state.toasts.list.where((t) => t.text.contains('gets the group')),
      isEmpty,
    );
    state.dispose();
  });
}
