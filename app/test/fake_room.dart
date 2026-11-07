// A room with made-up participants, as the room page draws it, for the layout tests.

import 'package:flutter/services.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';

const _names = [
  'Leo',
  'Sofía',
  'Ana',
  'Bruno',
  'Clara',
  'Diego',
  'Elena',
  'Fabio',
  'Gala',
  'Hugo',
  'Irene',
  'Julia',
  'Karim',
  'Lucía',
  'Mateo',
  'Nora',
];

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

/// The other participants of a fake room: ids `p0`, `p1`…, their names followed by
/// [longer].
UiPerson other(int i, {Uint8List? photo, String longer = ''}) => UiPerson(
  id: 'p$i',
  name: '${_names[i % _names.length]}$longer',
  short: 'AB${i}C·D${i}EF',
  avatar: UiAvatar(preset: i % 5, photo: photo),
);

/// A state showing the room of group `g` with `people` in it, this device included.
/// `sharing` of the others share a screen, `cameras` of them have the camera on,
/// `watching` watches the first one sharing, `own` shares this device's screen and
/// `ownShown` fills the room with it. `photo`, when given, is every other one's picture.
/// [longer] follows every name, this device's and the group's too, to try long ones.
AppState fakeRoom({
  required int people,
  int sharing = 0,
  int cameras = 0,
  bool watching = false,
  bool own = false,
  bool ownShown = false,
  Uint8List? photo,
  String dataDir = '/nonexistent/fogata-layout-test',
  String longer = '',
}) {
  final others = [
    for (var i = 0; i < people - 1; i++) other(i, photo: photo, longer: longer),
  ];
  final self = longer.isEmpty
      ? me
      : UiPerson(
          id: me.id,
          name: '${me.name}$longer',
          short: me.short,
          avatar: me.avatar,
        );
  final everyone = [self, ...others];
  // Open for a quarter of an hour.
  final since = DateTime.now().millisecondsSinceEpoch - 15 * 60 * 1000;
  final summary = UiRoomSummary(
    startedMs: since,
    participants: everyone,
    joined: true,
  );
  final call = Call(group: 'g', me: self.id, mic: true);
  for (final (i, p) in others.indexed) {
    call.peers[p.id] = CallPeer(p.id)
      ..connected = true
      ..mic = i.isEven
      ..cam = i < cameras
      ..sharing = i < sharing;
  }
  if (watching && sharing > 0) call.watching = others.first.id;
  if (own) {
    call.sharing = true;
    call.ownScreen = ownShown;
  }
  return AppState(dataDir)
    ..contacts.profile = UiProfile(
      person: self,
      fingerprint: '7Q4M K2XR 9PLD 3VHC',
    )
    ..contacts.groups = [
      UiGroup(
        id: 'g',
        name: 'Friday games$longer',
        localName: '',
        icon: 0,
        members: 20,
        online: people - 1,
        manager: true,
        rank: 0,
        outranks: const [],
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: [
          UiVoiceRoom(id: AppState.firstRoom, name: '', room: summary),
        ],
        room: summary,
        openInvites: 0,
      ),
    ]
    ..nav.page = Screen.room
    ..roomState.roomGroup = 'g'
    ..nav.selectedGroup = 'g'
    ..roomState.room = UiRoom(
      startedMs: since,
      joined: true,
      participants: [
        for (final p in everyone)
          UiParticipant(person: p, away: false, me: p.id == self.id),
      ],
      chat: const [],
      files: const [],
    )
    ..callState.call = call;
}

/// The app's own fonts, so text takes the room it takes in the app.
Future<void> loadFonts() async {
  for (final (family, files) in [
    (
      'Inter',
      [
        'Inter-Regular.ttf',
        'Inter-Medium.ttf',
        'Inter-SemiBold.ttf',
        'Inter-Bold.ttf',
      ],
    ),
    ('GeistMono', ['GeistMono-Regular.ttf', 'GeistMono-Medium.ttf']),
    ('PhosphorRegular', ['Phosphor-Regular.ttf']),
    ('PhosphorBold', ['Phosphor-Bold.ttf']),
  ]) {
    final loader = FontLoader(family);
    for (final file in files) {
      loader.addFont(rootBundle.load('assets/fonts/$file'));
    }
    await loader.load();
  }
}
