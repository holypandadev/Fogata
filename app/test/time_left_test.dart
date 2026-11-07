// While someone is kept out of a room for a while, the time left that the window shows is
// worked out again each minute, and when the nearest time out ends: it goes down by itself
// and a room opens again without anything else happening.

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';

class _Core implements RustLibApi {
  /// When the room's time out ends, or none when null.
  int? until;
  int asked = 0;

  @override
  Future<List<UiGroup>> crateApiAppGroups() async {
    asked++;
    final until = this.until;
    return [
      UiGroup(
        id: 'g',
        name: 'Call',
        localName: '',
        icon: 0,
        members: 2,
        online: 2,
        manager: false,
        rank: 1,
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: [
          UiVoiceRoom(
            id: AppState.firstRoom,
            name: 'General',
            barrier: until == null
                ? null
                : UiBarrier(
                    kind: UiBarrierKind.timedOut,
                    by: 'ana',
                    sinceMs: until - 3 * 60 * 60 * 1000,
                    untilMs: until,
                  ),
          ),
        ],
        openInvites: 0,
        outranks: const [],
      ),
    ];
  }

  @override
  Future<List<UiGroupInvite>> crateApiAppGroupInvitations() async => const [];

  @override
  Future<List<UiMember>> crateApiAppMembers({required String groupId}) async =>
      const [];

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));

  testWidgets('the time left is worked out again each minute', (tester) async {
    core
      ..until = DateTime.now().millisecondsSinceEpoch + 2 * 60 * 60 * 1000
      ..asked = 0;
    final state = AppState('/nonexistent/fogata-time-left-test');
    var told = 0;
    state.addListener(() => told++);
    await state.refreshGroups();
    expect(core.asked, 1);
    expect(state.watchingTimeOuts, isTrue);
    told = 0;
    // A minute on, with nothing else happening: read again, and the window told.
    await tester.pump(const Duration(seconds: 61));
    await tester.idle();
    expect(core.asked, 2);
    expect(told, greaterThan(0));
    await tester.pump(const Duration(seconds: 61));
    await tester.idle();
    expect(core.asked, 3);
    state.dispose();
  });

  testWidgets('the room opens again as soon as its time out ends', (
    tester,
  ) async {
    core
      ..until = DateTime.now().millisecondsSinceEpoch + 5000
      ..asked = 0;
    final state = AppState('/nonexistent/fogata-time-left-test');
    await state.refreshGroups();
    // Ended, the room comes back without its time out: nothing more to watch.
    core.until = null;
    await tester.pump(const Duration(seconds: 6));
    await tester.idle();
    expect(core.asked, 2);
    expect(state.watchingTimeOuts, isFalse);
    expect(state.contacts.groups.single.voiceRooms.single.barrier, isNull);
    state.dispose();
  });
}
