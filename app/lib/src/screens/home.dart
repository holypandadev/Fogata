import 'dart:async';

import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../l10n.dart';
import '../room_count.dart';
import '../icons.dart';
import '../rust/api/types.dart';
import '../shown.dart';
import '../theme.dart';
import '../widgets.dart';
import 'new_group.dart';
import 'room.dart';

/// The groups: rooms live now, and the rest.
class Home extends StatelessWidget {
  const Home({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final live = state.contacts.groups.where((g) => g.room != null).toList();
    final quiet = state.contacts.groups.where((g) => g.room == null).toList();
    return PageScroll(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1120),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(40, 36, 40, 56),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            l10n.homeTitle,
                            style: hx(c.fg, size: 32, height: 1.15),
                          ),
                          const SizedBox(height: 6),
                          Text(l10n.homeSubtitle, style: tx(c.fg2)),
                        ],
                      ),
                    ),
                    const SizedBox(width: 12),
                    FgButton(
                      text: l10n.homeAddFriend,
                      onTap: () => state.go(Screen.friends),
                    ),
                    const SizedBox(width: 12),
                    FgButton(
                      text: l10n.homeNewGroup,
                      icon: Ic.plus,
                      kind: Kind.primary,
                      onTap: () => showNewGroup(context),
                    ),
                  ],
                ),
                const SizedBox(height: 36),
                for (final invite in state.contacts.invitations) ...[
                  _InviteCard(invite: invite),
                  const SizedBox(height: 16),
                ],
                if (state.contacts.invitations.isNotEmpty)
                  const SizedBox(height: 20),
                if (state.contacts.groups.isEmpty)
                  const _Empty()
                else ...[
                  if (live.isNotEmpty) ...[
                    Row(
                      children: [
                        Text(
                          l10n.homeLive,
                          style: tx(
                            c.fg,
                            size: 16,
                            weight: w600,
                            spacing: -.16,
                          ),
                        ),
                        const SizedBox(width: 10),
                        Container(
                          height: heightForText(context, 20, 12),
                          padding: const EdgeInsets.symmetric(horizontal: 7),
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: c.greenSoft,
                            borderRadius: BorderRadius.circular(999),
                          ),
                          child: Text(
                            '${live.length}',
                            style: tx(
                              c.green,
                              size: 12,
                              weight: w500,
                              height: 1.2,
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 14),
                    LayoutBuilder(
                      builder: (context, box) {
                        final columns = (box.maxWidth / 400).floor().clamp(
                          1,
                          3,
                        );
                        final width =
                            (box.maxWidth - 16 * (columns - 1)) / columns;
                        // Cards side by side take the height of the tallest, their
                        // buttons on one line at the bottom.
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            for (var i = 0; i < live.length; i += columns) ...[
                              if (i > 0) const SizedBox(height: 16),
                              IntrinsicHeight(
                                child: Row(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    for (final (j, g)
                                        in live
                                            .skip(i)
                                            .take(columns)
                                            .indexed) ...[
                                      if (j > 0) const SizedBox(width: 16),
                                      SizedBox(
                                        width: width,
                                        child: _LiveCard(group: g),
                                      ),
                                    ],
                                  ],
                                ),
                              ),
                            ],
                          ],
                        );
                      },
                    ),
                    const SizedBox(height: 36),
                  ],
                  if (quiet.isNotEmpty) ...[
                    Row(
                      children: [
                        Text(
                          l10n.homeNoRoom,
                          style: tx(
                            c.fg,
                            size: 16,
                            weight: w600,
                            spacing: -.16,
                          ),
                        ),
                        const SizedBox(width: 10),
                        Text(l10n.homeNoRoomNote, style: tx(c.fg3, size: 13)),
                      ],
                    ),
                    const SizedBox(height: 14),
                    Panel(
                      child: Column(
                        children: [
                          for (var i = 0; i < quiet.length; i++)
                            _QuietRow(
                              group: quiet[i],
                              last: i == quiet.length - 1,
                            ),
                        ],
                      ),
                    ),
                  ],
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A group a friend added this device to: nothing runs until the person joins.
class _InviteCard extends StatelessWidget {
  const _InviteCard({required this.invite});
  final UiGroupInvite invite;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    return Panel(
      padding: const EdgeInsets.fromLTRB(20, 16, 16, 16),
      child: Row(
        children: [
          Container(
            width: 40,
            height: 40,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: c.border),
            ),
            child: FgIcon(Ic.users, size: 18, color: c.fg),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // The group's name on a line of its own, then who added this
                // device: in one line, a friend's long name hid the group's.
                Text(
                  invite.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, weight: w500),
                ),
                Text(
                  l10n.homeInviteBy(invite.by.name, invite.members),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg2, size: 13),
                ),
                Text(l10n.homeInviteNote, style: tx(c.fg2, size: 13)),
              ],
            ),
          ),
          const SizedBox(width: 12),
          FgButton(
            text: l10n.commonNotNow,
            height: 32,
            onTap: () => state.answerInvitation(invite.id, false),
          ),
          const SizedBox(width: 8),
          FgButton(
            text: l10n.homeInviteJoin,
            kind: Kind.primary,
            height: 32,
            onTap: () => state.answerInvitation(invite.id, true),
          ),
        ],
      ),
    );
  }
}

class _Empty extends StatelessWidget {
  const _Empty();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Panel(
      padding: const EdgeInsets.all(28),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.homeNoGroups, style: tx(c.fg, size: 16, weight: w600)),
          const SizedBox(height: 6),
          Text(l10n.homeNoGroupsNote, style: tx(c.fg2)),
        ],
      ),
    );
  }
}

/// Ticks every second while shown, for clocks.
class Ticker extends StatefulWidget {
  const Ticker({
    super.key,
    required this.builder,
    this.every = const Duration(seconds: 1),
  });
  final WidgetBuilder builder;
  final Duration every;

  @override
  State<Ticker> createState() => _TickerState();
}

/// Stops while the window is hidden, and catches up when it is shown again.
class _TickerState extends State<Ticker> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    windowShown.addListener(_shownChanged);
    if (windowShown.value) _start();
  }

  void _start() =>
      _timer = Timer.periodic(widget.every, (_) => setState(() {}));

  void _shownChanged() {
    _timer?.cancel();
    _timer = null;
    if (!windowShown.value) return;
    _start();
    setState(() {});
  }

  @override
  void dispose() {
    windowShown.removeListener(_shownChanged);
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.builder(context);
}

class _LiveCard extends StatelessWidget {
  const _LiveCard({required this.group});
  final UiGroup group;

  Widget _person(AppState state, Fg c, UiPerson p) => Padding(
    padding: const EdgeInsets.only(bottom: 10),
    child: Row(
      children: [
        Avatar(avatar: p.avatar, size: 24),
        const SizedBox(width: 10),
        Expanded(
          child: Text(
            p.id == state.contacts.profile?.person.id
                ? l10n.homePersonYou(p.name)
                : p.name,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg),
          ),
        ),
        const SizedBox(width: 12),
        Text(p.short, style: tx(c.fg3, size: 12, monospace: true)),
      ],
    ),
  );

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final room = group.room!;
    // One call at a time: in a call the buttons are its own, otherwise how to enter.
    final mic = state.callState.call?.mic ?? state.prefs.joinWithMic;
    final camera =
        state.callState.call?.camera ?? state.callState.joinWithCamera;
    return Panel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 20, 20, 16),
            child: Row(
              children: [
                GroupBadge(
                  icon: group.icon,
                  picture: group.picture,
                  size: 40,
                  radius: 10,
                  iconSize: 18,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        group.name,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, size: 15, weight: w600, height: 1.3),
                      ),
                      Text(
                        l10n.homeMembers(group.members),
                        style: tx(c.fg2, size: 13),
                      ),
                    ],
                  ),
                ),
                Pill(
                  text: room.joined ? l10n.homeYouAreIn : l10n.homeLive,
                  color: c.green,
                  background: c.greenSoft,
                  height: 24,
                  // A dot for the room this device is in.
                  leading: room.joined ? Dot(c.green) : null,
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 18),
            child: Column(
              children: [
                // Everyone live in the group, under the voice room they are in.
                for (final v in group.voiceRooms)
                  if (v.room case final live?) ...[
                    Row(
                      children: [
                        FgIcon(Ic.mic, size: 13, color: c.fg3),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            Contacts.voiceName(v),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tx(c.fg2, size: 12, weight: w500),
                          ),
                        ),
                        const SizedBox(width: 8),
                        RoomCount(
                          count: live.participants.length,
                          max: group.maxInRoom,
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    for (final p in live.participants) _person(state, c, p),
                  ],
              ],
            ),
          ),
          const Spacer(),
          Container(
            padding: const EdgeInsets.fromLTRB(20, 12, 16, 12),
            decoration: BoxDecoration(
              color: c.bg2,
              border: Border(top: BorderSide(color: c.border)),
            ),
            child: Row(
              children: [
                FgIcon(Ic.clock, size: 13, color: c.fg3),
                const SizedBox(width: 6),
                Expanded(
                  child: Ticker(
                    every: const Duration(seconds: 15),
                    builder: (context) => Text(
                      minutes(room.startedMs),
                      style: tx(c.fg3, size: 12, monospace: true),
                    ),
                  ),
                ),
                FgButton(
                  icon: mic ? Ic.mic : Ic.micOff,
                  height: 32,
                  tooltip: mic ? l10n.homeMuteMic : l10n.homeUnmuteMic,
                  iconColor: mic ? c.fg : c.red,
                  onTap: () => state.setMic(!mic),
                ),
                const SizedBox(width: 8),
                FgButton(
                  icon: camera ? Ic.camera : Ic.cameraOff,
                  height: 32,
                  tooltip: camera ? l10n.homeCameraOff : l10n.homeCameraOn,
                  iconColor: camera ? c.fg : c.fg3,
                  onTap: () => state.setCamera(!camera),
                ),
                const SizedBox(width: 8),
                FgButton(
                  icon: Ic.users,
                  height: 32,
                  tooltip: l10n.homeMembersTooltip,
                  onTap: () => state.showMembers(group.id),
                ),
                const SizedBox(width: 8),
                FgButton(
                  text: room.joined ? l10n.homeReturnToRoom : l10n.homeJoinRoom,
                  kind: Kind.primary,
                  height: 32,
                  tooltip: room.joined ? null : _barred(state, group),
                  // Off while a manager keeps this device out of the room it opens.
                  onTap: !room.joined && _barred(state, group) != null
                      ? null
                      : () => openRoomChecked(context, group.id),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _QuietRow extends StatelessWidget {
  const _QuietRow({required this.group, required this.last});
  final UiGroup group;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final others = (state.contacts.members[group.id] ?? const <UiMember>[])
        .where((m) => !m.me)
        .toList();
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: BoxDecoration(
        border: last ? null : Border(bottom: BorderSide(color: c.border)),
      ),
      child: LayoutBuilder(
        builder: (context, box) {
          final wide = box.maxWidth > 640;
          return Row(
            children: [
              GroupBadge(
                icon: group.icon,
                picture: group.picture,
                size: 32,
                radius: 8,
                iconSize: 15,
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      group.name,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg, weight: w500),
                    ),
                    Text(
                      l10n.homeMembersOnline(group.members, group.online),
                      style: tx(c.fg2, size: 13),
                    ),
                  ],
                ),
              ),
              if (wide) ...[
                const SizedBox(width: 14),
                _Stack(people: others.map((m) => m.person).toList()),
                const SizedBox(width: 14),
                SizedBox(
                  width: 190,
                  child: Text(
                    group.lastRoomMs == null
                        ? l10n.homeNoRoomsYet
                        : l10n.homeLastRoom(ago(group.lastRoomMs!)),
                    textAlign: TextAlign.right,
                    style: tx(c.fg3, size: 13),
                  ),
                ),
              ],
              const SizedBox(width: 14),
              FgButton(
                icon: Ic.users,
                kind: Kind.ghost,
                height: 32,
                tooltip: l10n.homeMembersTooltip,
                onTap: () => state.showMembers(group.id),
              ),
              const SizedBox(width: 6),
              FgButton(
                text: l10n.homeOpenRoom,
                height: 32,
                padding: const EdgeInsets.symmetric(horizontal: 12),
                tooltip: _barred(state, group),
                onTap: _barred(state, group) != null
                    ? null
                    : () => openRoomChecked(context, group.id),
              ),
            ],
          );
        },
      ),
    );
  }
}

/// Why this device may not enter the voice room [group] opens in, if it may not.
String? _barred(AppState state, UiGroup group) {
  final room = state.contacts.voiceRoom(
    group.id,
    state.contacts.voiceToOpen(group.id),
  );
  return room == null ? null : AppState.barrierText(room);
}

/// Overlapping avatars, at most three and a count.
class _Stack extends StatelessWidget {
  const _Stack({required this.people});
  final List<UiPerson> people;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final shown = people.length > 3 ? people.sublist(0, 2) : people;
    final rest = people.length - shown.length;
    final items = <Widget>[
      for (final p in shown)
        Container(
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            boxShadow: [BoxShadow(color: c.bg1, spreadRadius: 2)],
          ),
          child: Avatar(avatar: p.avatar, size: 22),
        ),
      if (rest > 0)
        Container(
          width: 22,
          height: 22,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: c.hover,
            boxShadow: [BoxShadow(color: c.bg1, spreadRadius: 2)],
          ),
          child: Text(
            '+$rest',
            style: tx(c.fg2, size: 10, weight: w600, height: 1),
          ),
        ),
    ];
    return SizedBox(
      width: items.isEmpty ? 0 : 22 + 16.0 * (items.length - 1),
      height: 22,
      child: Stack(
        children: [
          for (var i = 0; i < items.length; i++)
            Positioned(left: 16.0 * i, child: items[i]),
        ],
      ),
    );
  }
}
