import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';

import 'fake_room.dart';

/// The core's side of a video fetched to play: it ends when the test says, as the core's
/// fetch does once the room is left.
class FakeCore implements RustLibApi {
  Completer<String>? fetching;

  @override
  Future<String> crateApiAppFetchToPlay({
    required String groupId,
    required String file,
    required String dir,
  }) => (fetching = Completer<String>()).future;

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

UiFile _video() => UiFile(
  id: 'f1',
  name: 'party.mp4',
  size: 1 << 20,
  sharedBy: other(0),
  holders: [other(0)],
  mine: false,
  sent: 0,
);

void main() {
  setUpAll(() => RustLib.initMock(api: core));

  test(
    'a video that stops coming because the room was left says nothing',
    () async {
      final state = fakeRoom(people: 2);
      final asked = state.videoToPlay(_video());
      await Future<void>.delayed(Duration.zero);
      state.roomState.roomGroup = null;
      core.fetching!.completeError('you left the room');
      expect(await asked, isNull);
      expect(state.toasts.list, isEmpty);
    },
  );

  test('one that does not come while the room is open says why', () async {
    final state = fakeRoom(people: 2);
    final asked = state.videoToPlay(_video());
    await Future<void>.delayed(Duration.zero);
    core.fetching!.completeError('not enough free space for this file');
    expect(await asked, isNull);
    expect(state.toasts.list, hasLength(1));
  });
}
