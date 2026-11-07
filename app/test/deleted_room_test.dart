import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';

UiGroup group(List<UiVoiceRoom> rooms) => UiGroup(
  id: 'g',
  name: 'Tests',
  localName: 'Tests',
  icon: 0,
  members: 2,
  online: 1,
  manager: false,
  rank: 1,
  outranks: const [],
  maxMembers: 20,
  maxInRoom: 8,
  voiceRooms: rooms,
  openInvites: 0,
);

void main() {
  const first = UiVoiceRoom(id: AppState.firstRoom, name: '');
  const gaming = UiVoiceRoom(id: '0707070707070707', name: 'Gaming');

  test(
    'a voice room deleted while this device is in it lets the window go',
    () async {
      final state = AppState('/nonexistent/fogata-deleted-room-test')
        ..contacts.groups = [
          group([first]),
        ]
        ..roomState.roomGroup = 'g'
        ..roomState.roomVoice = gaming.id
        ..nav.selectedGroup = 'g'
        ..nav.page = Screen.room;
      await state.leaveIfDeleted(gaming);
      expect(state.roomState.roomGroup, isNull);
      expect(state.nav.page, Screen.home);
      expect(state.toasts.list.single.text, '“Gaming” was deleted.');
    },
  );

  test(
    'a voice room still there, or another one shown, changes nothing',
    () async {
      final state = AppState('/nonexistent/fogata-deleted-room-test')
        ..contacts.groups = [
          group([first, gaming]),
        ]
        ..roomState.roomGroup = 'g'
        ..roomState.roomVoice = gaming.id
        ..nav.page = Screen.room;
      await state.leaveIfDeleted(gaming);
      expect(state.roomState.roomGroup, 'g');
      state.contacts.groups = [
        group([first]),
      ];
      state.roomState.roomVoice = first.id;
      await state.leaveIfDeleted(gaming);
      expect(state.roomState.roomGroup, 'g');
      expect(state.nav.page, Screen.room);
      expect(state.toasts.list, isEmpty);
    },
  );
}
