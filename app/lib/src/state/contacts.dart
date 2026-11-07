import 'dart:typed_data';

import '../l10n.dart';
import '../moving.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import 'notices.dart';
import 'piece.dart';
import 'room_state.dart';

/// Whom this device is in contact with: its own profile, its groups with their members
/// and banners, its friends, the requests and invitations waiting, and how each member is
/// reached.
class Contacts extends Piece implements NoticeSources {
  UiProfile? profile;

  /// The bio friends see on this device's card: none until the person writes one, and the
  /// cards then show `defaultBio`.
  String? bio;
  List<UiGroup> groups = [];

  /// Groups friends added this device to, waiting for an answer.
  @override
  List<UiGroupInvite> invitations = [];

  /// People waiting for this device's manager to let them into a group with an invite.
  @override
  List<UiJoinAsk> joinAsks = [];
  final Map<String, List<UiMember>> members = {};
  @override
  List<UiFriend> friends = [];
  @override
  UiRequests requests = UiRequests(incoming: [], outgoing: []);

  /// Friends' banners, and the ids of the friends in common with each, as fetched when
  /// their cards open.
  final friendBanners = <String, UiBanner?>{};
  final mutualFriends = <String, List<String>>{};

  /// When the friends in common with each friend were last asked for.
  final mutualAskedMs = <String, int>{};

  /// How each member of each group is reached now, by group and member.
  final Map<String, Map<String, UiPath>> paths = {};

  /// By group, the member whose relay slot carries each member that comes through one,
  /// when the core names it (always a member of that same group).
  final Map<String, Map<String, String>> carriers = {};

  /// Friends and members see this device as away: nobody used the computer for a while.
  bool away = false;

  /// The newest friend link and code made here, shared by the panel and Friends.
  UiInvitation? friendLink;
  UiInvitation? friendCode;

  UiGroup? group(String? id) {
    for (final g in groups) {
      if (g.id == id) return g;
    }
    return null;
  }

  UiGroup? get joinedGroup {
    for (final g in groups) {
      if (g.room?.joined ?? false) return g;
    }
    return null;
  }

  int get pendingRequests => requests.incoming.length;

  /// The voice room of `group` this device is in, if any.
  String? joinedVoice(String group) {
    for (final v in this.group(group)?.voiceRooms ?? const <UiVoiceRoom>[]) {
      if (v.room?.joined ?? false) return v.id;
    }
    return null;
  }

  UiVoiceRoom? voiceRoom(String? group, String voice) {
    for (final v in this.group(group)?.voiceRooms ?? const <UiVoiceRoom>[]) {
      if (v.id == voice) return v;
    }
    return null;
  }

  /// The voice room a group opens in: the one this device is in, or else the first one
  /// with people in it, or else the first one.
  String voiceToOpen(String group) {
    final joined = joinedVoice(group);
    if (joined != null) return joined;
    final rooms = this.group(group)?.voiceRooms ?? const <UiVoiceRoom>[];
    // One this device may enter, when there is one.
    for (final v in rooms) {
      if (v.barrier == null && (v.room?.participants.isNotEmpty ?? false)) {
        return v.id;
      }
    }
    if (voiceRoom(group, RoomState.firstRoom)?.barrier != null) {
      for (final v in rooms) {
        if (v.barrier == null) return v.id;
      }
    }
    return RoomState.firstRoom;
  }

  /// Everyone in any of a group's voice rooms, room after room.
  static List<UiPerson> peopleIn(UiGroup g) => [
    for (final v in g.voiceRooms) ...?v.room?.participants,
  ];

  /// A voice room's name as shown: the first one is General until someone names it.
  static String voiceName(UiVoiceRoom? v) =>
      v == null || v.name.isEmpty ? l10n.contactsGeneral : v.name;

  /// The name, as [group]'s member list has it now, of the member that carries
  /// [device]'s connection; null when it comes through none, or through one not known.
  String? carrierName(String? group, String device) {
    final carrier = carriers[group]?[device];
    if (carrier == null) return null;
    for (final m in members[group] ?? const <UiMember>[]) {
      if (m.person.id == carrier) return m.person.name;
    }
    return null;
  }

  String friendName(String? device) {
    for (final f in friends) {
      if (f.person.id == device) return f.person.name;
    }
    for (final r in requests.incoming) {
      if (r.person.id == device) return r.person.name;
    }
    return l10n.appSomeone;
  }

  UiPerson? friendPerson(String id) {
    for (final f in friends) {
      if (f.person.id == id) return f.person;
    }
    return null;
  }

  /// Friends and members of this device's groups, as last seen.
  var _knownPeople = <String>{};

  /// Forgets the animations of people who were friends or members and are no longer.
  void forgetGonePeople() {
    final known = {
      for (final f in friends) f.person.id,
      for (final list in members.values)
        for (final m in list) m.person.id,
    };
    for (final id in _knownPeople.difference(known)) {
      movingBook.forget(id);
    }
    _knownPeople = known;
  }

  /// Groups' banners as fetched, with the short name each was fetched under.
  final groupBanners = <String, (String, Uint8List?)>{};

  /// The group's banner, when it has one and it is here; asked for once it changed.
  Uint8List? groupBanner(UiGroup? group) {
    final id = group?.banner;
    if (group == null || id == null) return null;
    final held = groupBanners[group.id];
    if (held == null || held.$1 != id) {
      groupBanners[group.id] = (id, held?.$2);
      _fetchGroupBanner(group.id, id);
    }
    return groupBanners[group.id]?.$2;
  }

  Future<void> _fetchGroupBanner(String group, String id) async {
    await null;
    try {
      final bytes = await api.groupBanner(groupId: group);
      if (groupBanners[group]?.$1 != id) return;
      groupBanners[group] = (id, bytes);
      changed();
    } catch (_) {}
  }
}
