import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../names.dart';
import '../moving.dart';
import '../room_count.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../update.dart' show GroupUpdateNote;
import '../widgets.dart';
import 'activity.dart';
import 'group_banner.dart';
import 'identity_preview.dart';
import 'members.dart'
    show RankMark, confirmDialog, leaveGroupChecked, showTimeOut, whoLetsIn;

/// The most voice rooms a group has, as the core allows.
const _maxVoiceRooms = 10;

/// A group's settings, floating over the window like the app's own. Those who manage it
/// change the name and picture, the voice rooms, who manages, invitations and limits;
/// every member sees the members in their order, acts on those below and finds the way
/// out.
class GroupSettingsLayer extends StatelessWidget {
  const GroupSettingsLayer({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final group = state.contacts.group(state.nav.groupSettings);
    if (group == null) return const SizedBox.shrink();
    return Positioned.fill(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape):
              state.closeGroupSettings,
        },
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusLayer(
          child: TweenAnimationBuilder<double>(
            tween: Tween(begin: 0, end: 1),
            duration: const Duration(milliseconds: 320),
            curve: spring,
            builder: (context, t, child) => Stack(
              children: [
                Positioned.fill(
                  child: Backdrop(
                    onTap: state.closeGroupSettings,
                    color: c.scrim.withValues(alpha: c.scrim.a * t.clamp(0, 1)),
                  ),
                ),
                Opacity(
                  opacity: t.clamp(0, 1),
                  child: Transform.scale(scale: .96 + .04 * t, child: child),
                ),
              ],
            ),
            child: Center(
              child: Padding(
                padding: const EdgeInsets.all(32),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(
                    maxWidth: 880,
                    maxHeight: 620,
                  ),
                  child: Container(
                    clipBehavior: Clip.antiAlias,
                    // Inside the outline, as when it was drawn under what is inside.
                    padding: const EdgeInsets.all(1),
                    decoration: BoxDecoration(
                      color: c.bg,
                      borderRadius: BorderRadius.circular(22),
                    ),
                    // Over what is inside, which would cover it at the rounded corners.
                    foregroundDecoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(22),
                      border: Border.all(color: c.border),
                    ),
                    child: _Window(group: group),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Window extends StatelessWidget {
  const _Window({required this.group});
  final UiGroup group;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final manager = group.manager;
    final tabs = [
      (l10n.groupSettingsTabGeneral, Ic.sliders),
      (l10n.groupSettingsTabVoiceRooms, Ic.volume),
      (l10n.groupSettingsTabMembers, Ic.users),
      (l10n.groupSettingsTabInvite, Ic.userPlus),
      (l10n.groupSettingsTabLimits, Ic.shield),
      (l10n.groupSettingsTabActivity, Ic.history),
    ];
    // Members who do not manage the group see the members and the activity only.
    bool open(int i) => manager || i == 2 || i == 5;
    final tab = open(state.nav.groupSettingsTab)
        ? state.nav.groupSettingsTab
        : 2;
    // Tab goes through the tabs on the left, then the one shown.
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        FocusTraversalGroup(
          child: Container(
            width: 220,
            padding: const EdgeInsets.fromLTRB(12, 18, 12, 14),
            decoration: BoxDecoration(
              color: c.bg1,
              border: Border(right: BorderSide(color: c.border)),
            ),
            // The tabs scroll when large text makes them taller than the window, and
            // Leave stays at the bottom while they fit.
            child: CustomScrollView(
              slivers: [
                SliverFillRemaining(
                  hasScrollBody: false,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(6, 0, 6, 16),
                        child: Row(
                          children: [
                            GroupBadge(
                              icon: group.icon,
                              picture: group.picture,
                              size: 36,
                              radius: 18,
                              iconSize: 16,
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    group.name,
                                    overflow: TextOverflow.ellipsis,
                                    style: tx(c.fg, size: 13, weight: w600),
                                  ),
                                  Text(
                                    group.rank == 0
                                        ? l10n.groupSettingsFirstHere
                                        : l10n.groupSettingsPlaceHere(
                                            ordinal(group.rank + 1),
                                          ),
                                    style: tx(c.fg3, size: 11),
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                      for (final (i, (text, icon)) in tabs.indexed)
                        if (open(i))
                          NavItem(
                            text: text,
                            icon: icon,
                            selected: tab == i,
                            onTap: () => state.showGroupSettingsTab(i),
                          ),
                      const Spacer(),
                      NavItem(
                        text: l10n.groupSettingsLeave,
                        icon: Ic.leave,
                        danger: true,
                        onTap: () async {
                          final left = await leaveGroupChecked(context, group);
                          if (left) state.closeGroupSettings();
                        },
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
        Expanded(
          child: FocusTraversalGroup(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(28, 16, 16, 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          tab == 3
                              ? l10n.groupSettingsInviteTitle
                              : tabs[tab].$1,
                          style: hx(c.fg, size: 18),
                        ),
                      ),
                      FgButton(
                        icon: Ic.close,
                        height: 32,
                        tooltip: l10n.groupSettingsClose,
                        onTap: state.closeGroupSettings,
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(28, 8, 28, 28),
                    child: switch (tab) {
                      0 => _General(key: ValueKey(group.id), group: group),
                      1 => _VoiceRooms(key: ValueKey(group.id), group: group),
                      3 => _Invite(key: ValueKey(group.id), group: group),
                      4 => _Limits(key: ValueKey(group.id), group: group),
                      5 => GroupActivity(key: ValueKey(group.id), group: group),
                      _ => _Members(group: group),
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

/// A small heading above a part of a tab.
class _Label extends StatelessWidget {
  const _Label(this.text);
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text(text, style: tx(context.fg.fg2, size: 12, weight: w500)),
  );
}

/// A line of explanation under a part of a tab.
class _Note extends StatelessWidget {
  const _Note(this.text);
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 8),
    child: Text(text, style: tx(context.fg.fg3, size: 12, height: 1.4)),
  );
}

/// A rounded list of rows.
class _Box extends StatelessWidget {
  const _Box({required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: children,
      ),
    );
  }
}

class _General extends StatefulWidget {
  const _General({super.key, required this.group});
  final UiGroup group;

  @override
  State<_General> createState() => _GeneralState();
}

class _GeneralState extends State<_General> {
  late final _name = TextEditingController(text: widget.group.name);
  bool _picking = false;
  bool _pickingBanner = false;

  @override
  void initState() {
    super.initState();
    _name.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _pick() async {
    setState(() => _picking = true);
    final state = AppScope.read(context);
    try {
      final picture = await pickGroupPhoto();
      if (picture != null && mounted) {
        await state.setGroupPicture(widget.group.id, picture);
      }
    } on PictureProblem catch (e) {
      state.say(e.message, error: true);
    }
    if (mounted) setState(() => _picking = false);
  }

  Future<void> _pickBanner() async {
    final state = AppScope.read(context);
    setState(() => _pickingBanner = true);
    final banner = await chooseGroupBanner(state);
    if (banner != null) await state.setGroupBanner(widget.group.id, banner);
    if (mounted) setState(() => _pickingBanner = false);
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final group = widget.group;
    final name = tidyName(_name.text);
    final changed =
        name.isNotEmpty &&
        name != group.name &&
        nameProblem(_name.text) == null;
    final banner = state.contacts.groupBanner(group);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        IdentityPreview(
          banner: banner == null
              ? const EmptyBanner()
              : BannerPicture(key: ValueKey(banner), bytes: banner),
          picture: GroupBadge(
            icon: group.icon,
            picture: group.picture,
            size: IdentityPreview.pictureSize,
            radius: IdentityPreview.pictureSize / 2,
            iconSize: 32,
          ),
          title: name.isEmpty ? group.name : name,
          below: Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              l10n.homeMembers(group.members),
              style: tx(c.fg3, size: 13),
            ),
          ),
          onPicture: _pick,
          pictureTooltip: l10n.groupSettingsChooseFromComputer,
          pictureBusy: _picking,
          bannerActions: [
            OverPill(
              icon: Ic.image,
              text: banner == null
                  ? l10n.groupBannerChoose
                  : l10n.groupBannerAnother,
              busy: _pickingBanner,
              onTap: _pickBanner,
            ),
            if (banner != null)
              OverPill(
                icon: Ic.close,
                tooltip: l10n.groupBannerNone,
                onTap: () => state.setGroupBanner(group.id, null),
              ),
          ],
        ),
        const SizedBox(height: 24),
        _Label(l10n.groupSettingsName),
        Row(
          children: [
            Expanded(
              child: FgField(
                controller: _name,
                maxLength: 60,
                label: l10n.groupSettingsNameField,
                onSubmitted: (_) => _rename(),
              ),
            ),
            const SizedBox(width: 8),
            FgButton(
              text: l10n.commonSave,
              kind: Kind.primary,
              onTap: changed ? _rename : null,
            ),
          ],
        ),
        NameNote(_name.text),
        _Note(l10n.groupSettingsNameNote),
        const SizedBox(height: 22),
        _Label(l10n.newGroupIconOrPicture),
        GroupIconRow(
          // A picture from the computer shows instead of any icon.
          selected: group.picture == null ? group.icon : null,
          onPick: (i) async {
            if (group.picture != null) {
              await state.setGroupPicture(group.id, null);
            }
            await state.attempt(
              () => api.setGroupIcon(groupId: group.id, icon: i),
            );
            await state.refreshGroups();
          },
        ),
        _Note(l10n.groupSettingsPictureNote),
      ],
    );
  }

  Future<void> _rename() async {
    final name = tidyName(_name.text);
    if (name.isEmpty ||
        name == widget.group.name ||
        nameProblem(_name.text) != null) {
      return;
    }
    final state = AppScope.read(context);
    await state.attempt(
      () => api.renameGroup(groupId: widget.group.id, name: name),
    );
    await state.refreshGroups();
  }
}

class _VoiceRooms extends StatefulWidget {
  const _VoiceRooms({super.key, required this.group});
  final UiGroup group;

  @override
  State<_VoiceRooms> createState() => _VoiceRoomsState();
}

class _VoiceRoomsState extends State<_VoiceRooms> {
  final Map<String, TextEditingController> _names = {};
  final Map<String, FocusNode> _focus = {};

  /// The room whose access is open below it.
  String? _access;

  @override
  void dispose() {
    for (final c in _names.values) {
      c.dispose();
    }
    for (final f in _focus.values) {
      f.dispose();
    }
    super.dispose();
  }

  TextEditingController _name(UiVoiceRoom v) => _names.putIfAbsent(
    v.id,
    // Redrawn as it is typed in, for the note under a name that cannot be one.
    () =>
        TextEditingController(text: Contacts.voiceName(v))
          ..addListener(() => setState(() {})),
  );

  FocusNode _node(UiVoiceRoom v) {
    final id = v.id;
    return _focus.putIfAbsent(id, () {
      final node = FocusNode();
      node.addListener(() {
        if (!node.hasFocus) _rename(id);
      });
      return node;
    });
  }

  @override
  void didUpdateWidget(_VoiceRooms old) {
    super.didUpdateWidget(old);
    // A name changed elsewhere shows, unless this one is being typed.
    for (final v in widget.group.voiceRooms) {
      final was = old.group.voiceRooms.where((o) => o.id == v.id).firstOrNull;
      final name = Contacts.voiceName(v);
      final field = _names[v.id];
      if (field != null &&
          was != null &&
          Contacts.voiceName(was) != name &&
          !(_focus[v.id]?.hasFocus ?? false)) {
        field.text = name;
      }
    }
  }

  /// Renames room [id] to what its field holds, if that is another valid name: the room is
  /// looked up as the group is now, not as it was when the field was made.
  Future<void> _rename(String id) async {
    if (!mounted) return;
    final v = widget.group.voiceRooms.where((r) => r.id == id).firstOrNull;
    final typed = _names[id]?.text ?? '';
    final name = tidyName(typed);
    if (v == null ||
        name.isEmpty ||
        name == Contacts.voiceName(v) ||
        nameProblem(typed) != null) {
      return;
    }
    await AppScope.read(context).setVoiceRoom(widget.group.id, id, name);
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final group = widget.group;
    final rooms = group.voiceRooms;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.groupSettingsRoomsIntro,
          style: tx(c.fg2, size: 13, height: 1.45),
        ),
        const SizedBox(height: 14),
        _Box(
          children: [
            for (final (i, v) in rooms.indexed)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        FgIcon(Ic.volume, size: 16, color: c.fg2),
                        const SizedBox(width: 10),
                        Expanded(
                          child: FgField(
                            controller: _name(v),
                            focusNode: _node(v),
                            height: 34,
                            radius: 999,
                            maxLength: 40,
                            label: l10n.groupSettingsRoomName,
                            onSubmitted: (_) => _rename(v.id),
                          ),
                        ),
                        const SizedBox(width: 10),
                        FgButton(
                          text: v.only == null
                              ? l10n.groupSettingsAccessEveryone
                              : l10n.groupSettingsAccessSome,
                          trailing: Ic.chevronDown,
                          kind: _access == v.id ? Kind.primary : Kind.secondary,
                          height: 30,
                          fontSize: 13,
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          tooltip: l10n.groupSettingsWhoCanEnter(
                            Contacts.voiceName(v),
                          ),
                          onTap: () => setState(
                            () => _access = _access == v.id ? null : v.id,
                          ),
                        ),
                        SizedBox(
                          width: 64,
                          child: Align(
                            alignment: Alignment.centerRight,
                            child: RoomCount(
                              count: peopleInRoom(v),
                              max: group.maxInRoom,
                            ),
                          ),
                        ),
                        const SizedBox(width: 4),
                        FgButton(
                          icon: Ic.trash,
                          kind: Kind.ghost,
                          height: 30,
                          tooltip: i == 0
                              ? l10n.groupSettingsFirstRoomStays
                              : v.room != null
                              ? l10n.groupSettingsPeopleInside
                              : l10n.groupSettingsDeleteRoom,
                          onTap: i == 0 || v.room != null
                              ? null
                              : () => state.removeVoiceRoom(group.id, v.id),
                        ),
                      ],
                    ),
                    Padding(
                      padding: const EdgeInsets.only(left: 26),
                      child: NameNote(_name(v).text),
                    ),
                    AnimatedSize(
                      duration: springTime,
                      curve: spring,
                      alignment: Alignment.topCenter,
                      child: _access == v.id
                          ? _Access(group: group, room: v)
                          : const SizedBox(width: double.infinity),
                    ),
                  ],
                ),
              ),
          ],
        ),
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerLeft,
          child: FgButton(
            text: l10n.groupSettingsAddRoom,
            icon: Ic.plus,
            height: 34,
            fontSize: 13,
            onTap: rooms.length >= _maxVoiceRooms
                ? null
                : () => state.setVoiceRoom(
                    group.id,
                    null,
                    l10n.groupSettingsNewRoomName(rooms.length + 1),
                  ),
          ),
        ),
        _Note(l10n.groupSettingsRoomsNote(_maxVoiceRooms, group.maxInRoom)),
        if (group.keptOut case final kept? when kept.isNotEmpty) ...[
          const SizedBox(height: 22),
          Text(l10n.groupSettingsKeptOut, style: tx(c.fg, weight: w600)),
          const SizedBox(height: 10),
          _Box(
            children: [
              for (final k in kept) _KeptOutLine(group: group, kept: k),
            ],
          ),
        ],
      ],
    );
  }
}

/// Who can enter a voice room: everyone in the group, or only the members chosen (and this
/// device and those above it, always).
class _Access extends StatelessWidget {
  const _Access({required this.group, required this.room});
  final UiGroup group;
  final UiVoiceRoom room;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final members = [...state.contacts.members[group.id] ?? const <UiMember>[]]
      ..sort((a, b) => a.rank.compareTo(b.rank));
    // The rule set here cannot leave out this device or anyone above it.
    bool always(UiMember m) => m.rank <= group.rank;
    final only = room.only;
    final name = Contacts.voiceName(room);
    void choose(List<String>? list) =>
        state.setRoomAccess(group.id, room.id, list);
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.only(top: 8, left: 26),
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      decoration: BoxDecoration(
        color: c.hover,
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.groupSettingsWhoCanEnter(name),
            style: tx(c.fg, weight: w500),
          ),
          const SizedBox(height: 8),
          _Choice(
            text: l10n.groupSettingsAccessEveryoneLong,
            selected: only == null,
            onTap: only == null ? null : () => choose(null),
          ),
          _Choice(
            text: l10n.groupSettingsAccessChosen,
            selected: only != null,
            // At first everyone who is in now, so that nothing changes until someone
            // is left out.
            onTap: only != null
                ? null
                : () => choose([
                    for (final m in members)
                      if (!always(m)) m.person.id,
                  ]),
          ),
          if (only != null) ...[
            const SizedBox(height: 4),
            Padding(
              padding: const EdgeInsets.only(left: 26),
              child: Wrap(
                spacing: 16,
                runSpacing: 2,
                children: [
                  for (final m in members)
                    SizedBox(
                      width: 200,
                      child: _MemberCheck(
                        member: m,
                        checked: always(m) || only.contains(m.person.id),
                        onTap: always(m)
                            ? null
                            : () => choose(
                                only.contains(m.person.id)
                                    ? [
                                        for (final id in only)
                                          if (id != m.person.id) id,
                                      ]
                                    : [...only, m.person.id],
                              ),
                      ),
                    ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.only(left: 26, top: 6),
              child: Text(
                l10n.groupSettingsAccessNote,
                style: tx(c.fg3, size: 12, height: 1.4),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// One of two answers, with a round mark.
class _Choice extends StatelessWidget {
  const _Choice({required this.text, required this.selected, this.onTap});
  final String text;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Semantics(
      checked: selected,
      inMutuallyExclusiveGroup: true,
      child: Pressable(
        onTap: onTap ?? () {},
        label: text,
        builder: (context, hovered) => SizedBox(
          height: heightForText(context, 32, 14),
          child: Row(
            children: [
              AnimatedContainer(
                duration: springTime,
                curve: spring,
                width: 16,
                height: 16,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: selected ? c.fg : c.borderStrong,
                    width: selected ? 5 : 1,
                  ),
                ),
              ),
              const SizedBox(width: 10),
              Text(text, style: tx(selected || hovered ? c.fg : c.fg2)),
            ],
          ),
        ),
      ),
    );
  }
}

/// A member to let into a room or not; this device and those above it are always in.
class _MemberCheck extends StatelessWidget {
  const _MemberCheck({required this.member, required this.checked, this.onTap});
  final UiMember member;
  final bool checked;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final m = member;
    final name = m.me ? l10n.groupSettingsYou(m.person.name) : m.person.name;
    return Semantics(
      checked: checked,
      enabled: onTap != null,
      child: Pressable(
        onTap: onTap,
        label: name,
        builder: (context, hovered) => SizedBox(
          height: heightForText(context, 32, 13),
          child: Row(
            children: [
              Opacity(opacity: onTap == null ? .5 : 1, child: FgCheck(checked)),
              const SizedBox(width: 10),
              Avatar(avatar: m.person.avatar, size: 20),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  name,
                  overflow: TextOverflow.ellipsis,
                  style: tx(checked && onTap != null ? c.fg : c.fg2, size: 13),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Someone kept out of a voice room, with the way to let them back for those above them.
class _KeptOutLine extends StatelessWidget {
  const _KeptOutLine({required this.group, required this.kept});
  final UiGroup group;
  final UiKeptOut kept;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final room = Contacts.voiceName(
      state.contacts.voiceRoom(group.id, kept.voice),
    );
    final until = kept.untilMs;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
      child: Row(
        children: [
          Avatar(avatar: kept.member.avatar, size: 24),
          const SizedBox(width: 10),
          Expanded(
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(text: kept.member.name, style: tx(c.fg, size: 13)),
                  TextSpan(
                    text: l10n.groupSettingsKeptOutBy(room, kept.by.name),
                    style: tx(c.fg2, size: 13),
                  ),
                ],
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 10),
          if (until != null)
            Text(
              l10n.groupSettingsTimeLeft(
                timeLeft(until - DateTime.now().millisecondsSinceEpoch),
              ),
              style: tx(c.fg2, size: 12, monospace: true),
            )
          else
            Text(l10n.groupSettingsUntilLetBack, style: tx(c.red, size: 12)),
          if (group.outranks.contains(kept.member.id)) ...[
            const SizedBox(width: 10),
            FgButton(
              text: l10n.groupSettingsLetBack,
              height: 30,
              fontSize: 13,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              onTap: () =>
                  state.letBackIntoRoom(group.id, kept.voice, kept.member.id),
            ),
          ],
        ],
      ),
    );
  }
}

class _Members extends StatelessWidget {
  const _Members({required this.group});
  final UiGroup group;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final members = [...state.contacts.members[group.id] ?? const <UiMember>[]]
      ..sort((a, b) => a.rank.compareTo(b.rank));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (group.needsUpdate) ...[
          GroupUpdateNote(updates: state.updates, say: state.say),
          const SizedBox(height: 12),
        ],
        if (!group.manager) ...[
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            decoration: BoxDecoration(
              color: c.blueSoft,
              borderRadius: BorderRadius.circular(14),
            ),
            child: Text(
              l10n.groupSettingsManagersNote(group.managers ?? 0),
              style: tx(c.blue, size: 13),
            ),
          ),
          const SizedBox(height: 12),
        ],
        // Who holds the group back from its stronger protection: they only need to
        // update. Nothing to do here for the manager but to tell them.
        if (group.levelBehind case final behind?
            when group.manager && behind.isNotEmpty) ...[
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            decoration: BoxDecoration(
              color: c.blueSoft,
              borderRadius: BorderRadius.circular(14),
            ),
            child: Text(
              l10n.groupSettingsLevelWaiting(
                behind.map((p) => p.name).join(', '),
              ),
              style: tx(c.blue, size: 13),
            ),
          ),
          const SizedBox(height: 12),
        ],
        Text(
          l10n.groupSettingsMembersCount(members.length, group.maxMembers),
          style: tx(c.fg2, size: 13),
        ),
        const SizedBox(height: 10),
        _Box(
          children: [
            for (final m in members) _MemberLine(group: group, member: m),
          ],
        ),
        _Note(l10n.groupSettingsMembersNote),
        if (group.manager) ...[
          const SizedBox(height: 22),
          _Managers(group: group),
        ],
      ],
    );
  }
}

/// Who manages the group: everyone, or the first few on its list.
class _Managers extends StatefulWidget {
  const _Managers({required this.group});
  final UiGroup group;

  @override
  State<_Managers> createState() => _ManagersState();
}

class _ManagersState extends State<_Managers> {
  /// The number of the first ones while choosing it, before it is set.
  late double _first = (widget.group.managers ?? _least).toDouble();

  /// Nobody leaves out itself or those above it.
  int get _least => widget.group.rank + 1;

  @override
  void didUpdateWidget(_Managers old) {
    super.didUpdateWidget(old);
    final first = widget.group.managers;
    if (first != null && first != old.group.managers) {
      _first = first.toDouble();
    }
  }

  Future<void> _set(int? first) =>
      AppScope.read(context).setGroupManagers(widget.group.id, first);

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final group = widget.group;
    final first = group.managers;
    final least = _least;
    // Up to those in the group now: more would make whoever comes in next a manager too.
    final most = group.members < least ? least : group.members;
    final shown = _first.clamp(least, most).round();
    // Alone, the first one is everyone.
    final alone = group.members <= 1;
    return Container(
      padding: const EdgeInsets.fromLTRB(18, 14, 18, 14),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.groupSettingsWhoManages,
            style: tx(c.fg, size: 14, weight: w500),
          ),
          const SizedBox(height: 6),
          _Choice(
            text: l10n.groupSettingsManagersEveryone,
            selected: first == null,
            onTap: first == null ? null : () => _set(null),
          ),
          Opacity(
            opacity: alone && first == null ? .5 : 1,
            child: _Choice(
              text: l10n.groupSettingsManagersFirstN(shown),
              selected: first != null,
              onTap: first != null || alone ? null : () => _set(shown),
            ),
          ),
          if (first != null && least < most)
            SliderTheme(
              data: SliderTheme.of(context).copyWith(
                activeTrackColor: c.fg,
                inactiveTrackColor: c.border,
                thumbColor: c.fg,
                overlayColor: c.fg.withValues(alpha: .08),
                trackHeight: 3,
              ),
              child: Slider(
                value: shown.toDouble(),
                min: least.toDouble(),
                max: most.toDouble(),
                divisions: most - least,
                label: '${_first.round()}',
                onChanged: (v) => setState(() => _first = v),
                onChangeEnd: (v) => _set(v.round()),
              ),
            ),
          const SizedBox(height: 4),
          if (first != null && first > group.members) ...[
            Text(
              l10n.groupSettingsManagersMore(first, group.members),
              style: tx(c.fg3, size: 12, height: 1.4),
            ),
            const SizedBox(height: 4),
          ],
          Text(
            first == null
                ? l10n.groupSettingsManagersEveryoneNote
                : l10n.groupSettingsManagersFirstNote(shown),
            style: tx(first == null ? c.fg3 : c.amber, size: 12, height: 1.4),
          ),
        ],
      ),
    );
  }
}

class _MemberLine extends StatelessWidget {
  const _MemberLine({required this.group, required this.member});
  final UiGroup group;
  final UiMember member;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final m = member;
    final below = group.outranks.contains(m.person.id);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
      child: Row(
        children: [
          Avatar(avatar: m.person.avatar, size: 32),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Flexible(
                      child: Text(
                        m.me
                            ? l10n.groupSettingsYou(m.person.name)
                            : m.person.name,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, size: 13, weight: w500),
                      ),
                    ),
                    const SizedBox(width: 8),
                    RankMark(rank: m.rank),
                  ],
                ),
                Text(
                  m.person.short,
                  style: tx(c.fg3, size: 11, monospace: true),
                ),
              ],
            ),
          ),
          if (below) ...[
            if (m.rank > group.rank + 1)
              FgButton(
                text: l10n.groupSettingsMoveUp,
                kind: Kind.ghost,
                height: 30,
                fontSize: 13,
                tooltip: l10n.groupSettingsMoveUpTooltip(m.person.name),
                onTap: () async {
                  await state.attempt(
                    () => api.promote(groupId: group.id, memberId: m.person.id),
                  );
                  await state.refreshGroups();
                },
              ),
            if (m.rank < group.members - 1)
              FgButton(
                text: l10n.groupSettingsToEnd,
                kind: Kind.ghost,
                height: 30,
                fontSize: 13,
                tooltip: l10n.groupSettingsToEndTooltip(m.person.name),
                onTap: () async {
                  await state.attempt(
                    () => api.demote(groupId: group.id, memberId: m.person.id),
                  );
                  await state.refreshGroups();
                },
              ),
            FgButton(
              icon: Ic.timer,
              kind: Kind.ghost,
              height: 30,
              tooltip: l10n.groupSettingsTimeOut,
              onTap: () => showTimeOut(context, group, m.person),
            ),
            const SizedBox(width: 4),
            FgButton(
              icon: Ic.userMinus,
              kind: Kind.ghost,
              height: 30,
              iconColor: c.red,
              tooltip: l10n.groupSettingsRemove,
              onTap: () async {
                final ok = await confirmDialog(
                  context,
                  title: l10n.membersRemoveTitle(m.person.name),
                  text: l10n.membersRemoveText(m.person.name),
                  action: l10n.commonRemove,
                );
                if (!ok) return;
                await state.attempt(
                  () => api.removeFromGroup(
                    groupId: group.id,
                    memberId: m.person.id,
                  ),
                );
                await state.refreshGroups();
              },
            ),
          ],
        ],
      ),
    );
  }
}

/// Inviting friends straight away, and people who are not friends with a link a manager
/// then lets them in with.
class _Invite extends StatefulWidget {
  const _Invite({super.key, required this.group});
  final UiGroup group;

  @override
  State<_Invite> createState() => _InviteState();
}

class _InviteState extends State<_Invite> {
  bool _copied = false;

  Future<void> _copy() async {
    final ok = await copyInviteLink(context, widget.group);
    if (!ok || !mounted) return;
    setState(() => _copied = true);
    Future.delayed(const Duration(milliseconds: 1500), () {
      if (mounted) setState(() => _copied = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final group = widget.group;
    final asks = state.contacts.joinAsks
        .where((a) => a.groupId == group.id)
        .toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (asks.isNotEmpty) ...[
          _Label(l10n.groupSettingsAskingToJoin),
          _Box(children: [for (final a in asks) JoinAskLine(ask: a)]),
          const SizedBox(height: 22),
        ],
        _Label(l10n.groupSettingsInviteFriend),
        InviteFriends(group: group),
        _Note(l10n.groupSettingsInviteFriendNote),
        const SizedBox(height: 22),
        _Label(l10n.groupSettingsInviteLink),
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: c.bg1,
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: c.border),
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  l10n.groupSettingsInviteLinkNote(
                    whoLetsIn(group),
                    group.managers ?? 0,
                  ),
                  style: tx(c.fg2, size: 12, height: 1.45),
                ),
              ),
              const SizedBox(width: 12),
              FgButton(
                text: _copied ? l10n.commonCopied : l10n.groupSettingsCopyLink,
                icon: _copied ? Ic.check : Ic.link,
                kind: _copied ? Kind.secondary : Kind.primary,
                height: 34,
                fontSize: 13,
                onTap: _copy,
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(6, 10, 6, 0),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  l10n.groupSettingsOpenLinks(group.openInvites),
                  style: tx(c.fg3, size: 12),
                ),
              ),
              if (group.openInvites > 0)
                FgButton(
                  text: l10n.groupSettingsCancelLinks,
                  kind: Kind.ghost,
                  height: 28,
                  fontSize: 12,
                  onTap: () => state.cancelInvites(group.id),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Copies a new invite link to a group: one use, 24 hours, a manager lets its user in.
Future<bool> copyInviteLink(BuildContext context, UiGroup group) async {
  final state = AppScope.read(context);
  final text = await state.attempt(
    () => api.invite(groupId: group.id, days: 1),
  );
  if (text == null) return false;
  await Clipboard.setData(ClipboardData(text: text));
  await state.refreshGroups();
  return true;
}

/// Friends who are not in a group, each with a button that invites them. A friend invited
/// is on the group's list until they join, decline or the invitation runs out, so they
/// leave this list once invited and come back to it when they declined.
class InviteFriends extends StatelessWidget {
  const InviteFriends({super.key, required this.group});
  final UiGroup group;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final inGroup = {
      for (final m in state.contacts.members[group.id] ?? const <UiMember>[])
        m.person.id,
    };
    final friends = [
      for (final f in state.contacts.friends)
        if (!inGroup.contains(f.person.id)) f,
    ];
    final full =
        (state.contacts.members[group.id]?.length ?? 0) >= group.maxMembers;
    if (friends.isEmpty) {
      return _Box(
        children: [
          Padding(
            padding: const EdgeInsets.all(10),
            child: Text(
              state.contacts.friends.isEmpty
                  ? l10n.groupSettingsNoFriends
                  : l10n.groupSettingsAllFriendsIn,
              style: tx(c.fg3, size: 13),
            ),
          ),
        ],
      );
    }
    return _Box(
      children: [
        for (final f in friends)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
            child: Row(
              children: [
                Avatar(avatar: f.person.avatar, size: 30),
                const SizedBox(width: 11),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        f.person.name,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, size: 13, weight: w500),
                      ),
                      Text(
                        f.path != null
                            ? l10n.groupSettingsFriendOnline
                            : l10n.groupSettingsFriendOffline,
                        style: tx(f.path != null ? c.green : c.fg3, size: 11),
                      ),
                    ],
                  ),
                ),
                FgButton(
                  text: l10n.groupSettingsInviteButton,
                  kind: Kind.primary,
                  height: 30,
                  fontSize: 12,
                  tooltip: full ? l10n.groupSettingsGroupFull : null,
                  onTap: full
                      ? null
                      : () => state.inviteFriend(group.id, f.person.id),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// Someone asking to join a group with an invite, with this manager's two answers.
class JoinAskLine extends StatelessWidget {
  const JoinAskLine({super.key, required this.ask});
  final UiJoinAsk ask;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  ask.name,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 13, weight: w500),
                ),
                Text(
                  ask.fingerprint,
                  style: tx(c.fg3, size: 11, monospace: true),
                ),
              ],
            ),
          ),
          FgButton(
            text: l10n.groupSettingsSayNo,
            height: 30,
            fontSize: 12,
            onTap: () => state.answerJoin(ask, false),
          ),
          const SizedBox(width: 6),
          FgButton(
            text: l10n.groupSettingsLetIn,
            kind: Kind.primary,
            height: 30,
            fontSize: 12,
            onTap: () => state.answerJoin(ask, true),
          ),
        ],
      ),
    );
  }
}

class _Limits extends StatefulWidget {
  const _Limits({super.key, required this.group});
  final UiGroup group;

  @override
  State<_Limits> createState() => _LimitsState();
}

class _LimitsState extends State<_Limits> {
  late double _members = widget.group.maxMembers.toDouble();
  late double _room = widget.group.maxInRoom.toDouble();

  /// A slider being dragged keeps its value; otherwise each follows the group's limits
  /// when they change, as another manager may do while this page is open.
  bool _dragging = false;

  @override
  void didUpdateWidget(_Limits old) {
    super.didUpdateWidget(old);
    if (_dragging) return;
    final group = widget.group;
    if (group.maxMembers != old.group.maxMembers) {
      _members = group.maxMembers.toDouble();
    }
    if (group.maxInRoom != old.group.maxInRoom) {
      _room = group.maxInRoom.toDouble();
    }
  }

  Future<void> _save() =>
      AppScope.read(context)
          .setGroupLimits(widget.group.id, _members.round(), _room.round());

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final group = widget.group;
    final least = group.members < 2 ? 2 : group.members;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _slider(
          c,
          title: l10n.groupSettingsLimitGroup,
          value: _members,
          min: 2,
          max: 20,
          onChanged: (v) =>
              setState(() => _members = v < least ? least.toDouble() : v),
          note: l10n.groupSettingsLimitGroupNote(group.members),
        ),
        const SizedBox(height: 14),
        _slider(
          c,
          title: l10n.groupSettingsLimitRoom,
          value: _room,
          min: 2,
          max: 8,
          onChanged: (v) => setState(() => _room = v),
          note: l10n.groupSettingsLimitRoomNote,
        ),
      ],
    );
  }

  Widget _slider(
    Fg c, {
    required String title,
    required double value,
    required double min,
    required double max,
    required ValueChanged<double> onChanged,
    required String note,
  }) => Container(
    padding: const EdgeInsets.fromLTRB(18, 14, 18, 14),
    decoration: BoxDecoration(
      color: c.bg1,
      borderRadius: BorderRadius.circular(16),
      border: Border.all(color: c.border),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(title, style: tx(c.fg, size: 14, weight: w500)),
            ),
            Text(
              '${value.round()}',
              style: tx(c.fg, size: 13, monospace: true),
            ),
          ],
        ),
        SliderTheme(
          data: SliderTheme.of(context).copyWith(
            activeTrackColor: c.fg,
            inactiveTrackColor: c.border,
            thumbColor: c.fg,
            overlayColor: c.fg.withValues(alpha: .08),
            trackHeight: 3,
          ),
          child: Slider(
            value: value,
            min: min,
            max: max,
            divisions: (max - min).round(),
            label: '${value.round()}',
            onChangeStart: (_) => _dragging = true,
            onChanged: onChanged,
            onChangeEnd: (_) {
              _dragging = false;
              _save();
            },
          ),
        ),
        Text(note, style: tx(c.fg3, size: 12, height: 1.4)),
      ],
    ),
  );
}
