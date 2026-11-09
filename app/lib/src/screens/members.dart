import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../volume.dart' show timeOuts;
import '../widgets.dart';
import 'friend_card.dart' show personCardLabel, togglePersonCard;
import 'group_settings.dart' show InviteFriends, copyInviteLink;
import 'room.dart' show openRoomChecked, pathWords;

/// A group's members in their order, the first one first: each acts on those below it. Who
/// is in, how each is reached; and the group's own actions (invite with a link, settings,
/// leave).
class MembersPage extends StatelessWidget {
  const MembersPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final group = state.contacts.group(state.nav.membersGroup);
    if (group == null) {
      return Center(child: Text(l10n.membersGroupGone, style: tx(c.fg2)));
    }
    final members = [...state.contacts.members[group.id] ?? const <UiMember>[]]
      ..sort((a, b) => a.rank.compareTo(b.rank));
    final manager = group.manager;
    // The footer's parts.
    final settings = FgButton(
      text: l10n.membersGroupSettings,
      icon: Ic.settings,
      height: 32,
      fontSize: 13,
      onTap: () => state.openGroupSettings(group.id),
    );
    final note = l10n.membersManagersNote(group.managers ?? 0);
    final leave = FgButton(
      text: l10n.membersLeaveGroup,
      kind: Kind.danger,
      height: 32,
      fontSize: 13,
      onTap: () => leaveGroupChecked(context, group),
    );
    // Why this device may not enter the room the button opens, if it may not.
    final opens = state.contacts.voiceRoom(
      group.id,
      state.contacts.voiceToOpen(group.id),
    );
    final barred = (group.room?.joined ?? false) || opens == null
        ? null
        : AppState.barrierText(opens);
    final online = members.where((m) => !m.me && m.path != null).length;
    return PageScroll(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 920),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(40, 36, 40, 56),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                LayoutBuilder(
                  builder: (context, box) {
                    final title = Row(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        GroupBadge(
                          icon: group.icon,
                          picture: group.picture,
                          size: 56,
                          radius: 14,
                          iconSize: 24,
                        ),
                        const SizedBox(width: 16),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                group.name,
                                overflow: TextOverflow.ellipsis,
                                style: hx(c.fg, size: 32, height: 1.15),
                              ),
                              const SizedBox(height: 4),
                              Text(
                                group.lastRoomMs == null
                                    ? l10n.membersSummary(group.members, online)
                                    : l10n.membersSummaryLastRoom(
                                        group.members,
                                        online,
                                        ago(group.lastRoomMs!),
                                      ),
                                style: tx(c.fg2),
                              ),
                            ],
                          ),
                        ),
                      ],
                    );
                    final actions = <Widget>[
                      if (manager) ...[
                        FgButton(
                          text: l10n.membersCopyInviteLink,
                          icon: Ic.link,
                          tooltip: l10n.membersInviteLinkTooltip(
                            whoLetsIn(group),
                            group.managers ?? 0,
                          ),
                          onTap: () async {
                            if (await copyInviteLink(context, group)) {
                              state.say(
                                l10n.membersInviteLinkCopied(
                                  whoLetsIn(group),
                                  group.managers ?? 0,
                                ),
                              );
                            }
                          },
                        ),
                        FgButton(
                          text: l10n.membersInviteFriends,
                          icon: Ic.userPlus,
                          onTap: () => _inviteFriends(context, group),
                        ),
                      ],
                      FgButton(
                        text: group.room?.joined ?? false
                            ? l10n.membersBackToRoom
                            : (group.room == null
                                  ? l10n.membersOpenRoom
                                  : l10n.membersEnterRoom),
                        kind: Kind.primary,
                        tooltip: barred,
                        // Off while someone keeps this device out of that room.
                        onTap: barred != null
                            ? null
                            : () => openRoomChecked(context, group.id),
                      ),
                    ];
                    // The buttons go under the name when beside it the name would
                    // get less than 360 points (a manager has three buttons).
                    final room = box.maxWidth - 72 - (manager ? 500 : 180);
                    if (room < 360) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          title,
                          const SizedBox(height: 20),
                          Wrap(spacing: 8, runSpacing: 8, children: actions),
                        ],
                      );
                    }
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        Expanded(child: title),
                        for (final (i, action) in actions.indexed) ...[
                          SizedBox(width: i == 0 ? 12 : 8),
                          action,
                        ],
                      ],
                    );
                  },
                ),
                const SizedBox(height: 28),
                Panel(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Container(
                        padding: const EdgeInsets.fromLTRB(20, 14, 20, 14),
                        decoration: BoxDecoration(
                          border: Border(bottom: BorderSide(color: c.border)),
                        ),
                        child: Row(
                          children: [
                            Text(
                              l10n.membersTitle,
                              style: tx(c.fg, weight: w600),
                            ),
                            const SizedBox(width: 10),
                            Flexible(
                              child: Text(
                                l10n.membersOrderNote,
                                overflow: TextOverflow.ellipsis,
                                style: tx(c.fg3, size: 13),
                              ),
                            ),
                          ],
                        ),
                      ),
                      for (var i = 0; i < members.length; i++)
                        _MemberRow(
                          group: group,
                          member: members[i],
                          last: i == members.length - 1,
                        ),
                      Container(
                        padding: const EdgeInsets.fromLTRB(20, 12, 20, 12),
                        decoration: BoxDecoration(
                          color: c.bg2,
                          border: Border(top: BorderSide(color: c.border)),
                        ),
                        // At the two ends of one line; with large text, one under
                        // the other when there is no room for both.
                        child: largeText(context)
                            ? Wrap(
                                alignment: WrapAlignment.spaceBetween,
                                crossAxisAlignment: WrapCrossAlignment.center,
                                spacing: 12,
                                runSpacing: 8,
                                children: [
                                  if (manager)
                                    settings
                                  else
                                    Text(note, style: tx(c.fg3, size: 13)),
                                  leave,
                                ],
                              )
                            : Row(
                                children: [
                                  if (manager)
                                    settings
                                  else
                                    Flexible(
                                      child: Text(
                                        note,
                                        overflow: TextOverflow.ellipsis,
                                        style: tx(c.fg3, size: 13),
                                      ),
                                    ),
                                  const Spacer(),
                                  leave,
                                ],
                              ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _inviteFriends(BuildContext context, UiGroup group) async {
    final c = context.fg;
    await showFgDialog<void>(
      context,
      maxWidth: 440,
      builder: (dialog) => Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 22, 24, 18),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  l10n.membersInviteTitle(group.name),
                  style: tx(c.fg, size: 18, weight: w600, height: 1.3),
                ),
                const SizedBox(height: 4),
                Text(l10n.membersInviteNote, style: tx(c.fg2, size: 13)),
                const SizedBox(height: 14),
                InviteFriends(group: group),
              ],
            ),
          ),
          Footer(
            children: [
              const Spacer(),
              FgButton(
                text: l10n.commonDone,
                kind: Kind.primary,
                onTap: () => Navigator.of(dialog).pop(),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _MemberRow extends StatelessWidget {
  const _MemberRow({
    required this.group,
    required this.member,
    required this.last,
  });

  final UiGroup group;
  final UiMember member;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final m = member;
    final path = m.path;
    final String presence;
    final Color presenceColor;
    if (m.me) {
      presence = l10n.membersPresenceYou;
      presenceColor = c.fg3;
    } else if (path != null) {
      final through = m.through;
      final carrier = through == null
          ? null
          : (state.contacts.members[group.id] ?? const <UiMember>[])
                .where((o) => o.person.id == through)
                .firstOrNull
                ?.person
                .name;
      final how = pathWords(path, through: carrier);
      presence = m.away
          ? l10n.membersPresenceAway(how)
          : l10n.membersPresenceOnline(how);
      presenceColor = m.away ? c.amber : c.green;
    } else {
      presence = l10n.membersPresenceOffline;
      presenceColor = c.fg3;
    }
    // Everyone acts on those below it, and only on them.
    final canAct = group.outranks.contains(m.person.id);
    // A row opens the member's card: a friend's as in the list of friends, the person's
    // own panel, or for anyone else a card that offers a friend request.
    final open = m.me
        ? state.nav.userPanelFrom != null && state.nav.userPanelBeside
        : state.nav.friendCard == m.person.id;
    return Pressable(
      key: ValueKey('member-row:${m.person.id}'),
      label: personCardLabel(state, m.person),
      onTap: () => togglePersonCard(context, m.person.id, group: group.id),
      builder: (context, hovered) => _row(
        context,
        state,
        presence,
        presenceColor,
        canAct,
        lit: open || hovered,
      ),
    );
  }

  Widget _row(
    BuildContext context,
    AppState state,
    String presence,
    Color presenceColor,
    bool canAct, {
    bool lit = false,
  }) {
    final c = context.fg;
    final m = member;
    final path = m.path;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: BoxDecoration(
        color: lit ? c.bg2 : null,
        border: last ? null : Border(bottom: BorderSide(color: c.border)),
      ),
      // The way a member comes can be long ("through" a member's name): it takes at most
      // three tenths of the row, shortened, and shows whole on hover.
      child: LayoutBuilder(
        builder: (context, box) => Row(
          children: [
            Avatar(
              avatar: m.person.avatar,
              size: 28,
              dim: !m.me && path == null,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Row(
                children: [
                  Flexible(
                    child: Text(
                      m.me ? l10n.membersYou(m.person.name) : m.person.name,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg, weight: w500),
                    ),
                  ),
                  const SizedBox(width: 8),
                  RankMark(rank: m.rank),
                  if (m.waitingForKey) ...[
                    const SizedBox(width: 8),
                    Flexible(
                      child: Text(
                        l10n.membersWaitingForKey,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.amber, size: 12),
                      ),
                    ),
                  ],
                ],
              ),
            ),
            const SizedBox(width: 12),
            ConstrainedBox(
              constraints: BoxConstraints(maxWidth: box.maxWidth * 0.3),
              child: Tooltip(
                message: presence,
                child: Text(
                  presence,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(presenceColor, size: 13),
                ),
              ),
            ),
            const SizedBox(width: 12),
            Tooltip(
              message: l10n.membersFingerprint(m.fingerprint),
              child: Text(
                m.person.short,
                style: tx(c.fg3, size: 12, monospace: true),
              ),
            ),
            const SizedBox(width: 8),
            SizedBox(
              width: 32,
              child: canAct
                  ? Builder(
                      builder: (anchor) => FgButton(
                        icon: Ic.dots,
                        kind: Kind.ghost,
                        height: 32,
                        tooltip: l10n.membersOptionsFor(m.person.name),
                        onTap: () => _options(anchor, state),
                      ),
                    )
                  : null,
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _options(BuildContext context, AppState state) async {
    final c = context.fg;
    final box = context.findRenderObject() as RenderBox;
    final origin = box.localToGlobal(Offset(0, box.size.height + 4));
    final choice = await showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(
        origin.dx - 200,
        origin.dy,
        origin.dx + box.size.width,
        0,
      ),
      color: c.bg1,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: BorderSide(color: c.border),
      ),
      items: [
        // Up to just below this device, unless it is there already.
        if (member.rank > group.rank + 1)
          PopupMenuItem(
            value: 'up',
            height: 36,
            child: Text(l10n.membersMoveUp, style: tx(c.fg, size: 13)),
          ),
        if (member.rank < group.members - 1)
          PopupMenuItem(
            value: 'down',
            height: 36,
            child: Text(l10n.membersMoveToEnd, style: tx(c.fg, size: 13)),
          ),
        PopupMenuItem(
          value: 'timeout',
          height: 36,
          child: Text(l10n.membersTimeOut, style: tx(c.fg, size: 13)),
        ),
        PopupMenuItem(
          value: 'remove',
          height: 36,
          child: Text(l10n.membersRemoveFromGroup, style: tx(c.red, size: 13)),
        ),
      ],
    );
    if (choice == null || !context.mounted) return;
    if (choice == 'up') {
      await state.attempt(
        () => api.promote(groupId: group.id, memberId: member.person.id),
      );
    } else if (choice == 'down') {
      await state.attempt(
        () => api.demote(groupId: group.id, memberId: member.person.id),
      );
    } else if (choice == 'timeout') {
      await showTimeOut(context, group, member.person);
      return;
    } else if (choice == 'remove') {
      final ok = await confirmDialog(
        context,
        title: l10n.membersRemoveTitle(member.person.name),
        text: l10n.membersRemoveText(member.person.name),
        action: l10n.commonRemove,
      );
      if (!ok) return;
      await state.attempt(
        () =>
            api.removeFromGroup(groupId: group.id, memberId: member.person.id),
      );
    }
    await state.refreshGroups();
  }
}

/// Who lets someone with an invite in, for messages that choose their words by it:
/// `anyone` in the group, the `first`, or (`other`) one of the first few
/// ([UiGroup.managers]).
String whoLetsIn(UiGroup group) => switch (group.managers) {
  null => 'anyone',
  1 => 'first',
  _ => 'other',
};

/// Where a member is in its group: "First" for the first one, a small "2nd" and so on for
/// the others.
class RankMark extends StatelessWidget {
  const RankMark({super.key, required this.rank});

  /// Counted from 0.
  final int rank;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    if (rank == 0) {
      return Pill(
        text: l10n.membersFirst,
        color: c.blue,
        background: c.blueSoft,
        height: 20,
        leading: FgIcon(Ic.crown, size: 11, color: c.blue),
      );
    }
    return Text(ordinal(rank + 1), style: tx(c.fg3, size: 12));
  }
}

/// Asks how long to keep [person] out of which voice room of [group], then does it.
Future<void> showTimeOut(
  BuildContext context,
  UiGroup group,
  UiPerson person,
) async {
  final state = AppScope.read(context);
  final picked = await showFgDialog<(String, int?)>(
    context,
    maxWidth: 420,
    builder: (dialog) => _TimeOutDialog(group: group, person: person),
  );
  if (picked == null) return;
  await state.keepOutOfRoom(group.id, picked.$1, person, picked.$2);
}

class _TimeOutDialog extends StatefulWidget {
  const _TimeOutDialog({required this.group, required this.person});
  final UiGroup group;
  final UiPerson person;

  @override
  State<_TimeOutDialog> createState() => _TimeOutDialogState();
}

class _TimeOutDialogState extends State<_TimeOutDialog> {
  late String _room = widget.group.voiceRooms.first.id;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final rooms = widget.group.voiceRooms;
    final name = widget.person.name;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 22, 24, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                l10n.membersTimeOutTitle(name),
                style: tx(c.fg, size: 18, weight: w600, height: 1.3),
              ),
              const SizedBox(height: 4),
              Text(l10n.membersTimeOutText(name), style: tx(c.fg2, size: 13)),
              if (rooms.length > 1) ...[
                const SizedBox(height: 14),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: [
                    for (final room in rooms)
                      FgButton(
                        text: Contacts.voiceName(room),
                        height: 30,
                        fontSize: 13,
                        kind: room.id == _room ? Kind.primary : Kind.ghost,
                        onTap: () => setState(() => _room = room.id),
                      ),
                  ],
                ),
              ],
              const SizedBox(height: 14),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: [
                  for (final (text, minutes) in timeOuts)
                    FgButton(
                      text: text,
                      height: 32,
                      fontSize: 13,
                      onTap: () => Navigator.of(context).pop((_room, minutes)),
                    ),
                  FgButton(
                    text: l10n.membersUntilLetBack,
                    kind: Kind.danger,
                    height: 32,
                    fontSize: 13,
                    onTap: () => Navigator.of(context).pop((_room, null)),
                  ),
                ],
              ),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              onTap: () => Navigator.of(context).pop(),
            ),
          ],
        ),
      ],
    );
  }
}

/// Asks, then leaves a group. Anyone may leave at any time; when this device is first, the
/// question says who comes first after it. Returns whether it left.
Future<bool> leaveGroupChecked(BuildContext context, UiGroup group) async {
  final state = AppScope.read(context);
  final next = (state.contacts.members[group.id] ?? const <UiMember>[])
      .where((m) => m.rank == 1)
      .firstOrNull;
  final ok = await confirmDialog(
    context,
    title: l10n.membersLeaveTitle(group.name),
    text: group.rank == 0 && next != null
        ? l10n.membersLeaveTextFirst(next.person.name)
        : l10n.membersLeaveText,
    action: l10n.membersLeaveGroup,
  );
  if (!ok) return false;
  if (state.roomState.roomGroup == group.id) await state.leaveRoom();
  if (!await state.succeeds(() => api.leaveGroup(groupId: group.id))) {
    return false;
  }
  state.forgetGroup(group.id);
  state.goHome();
  await state.refreshGroups();
  return true;
}

/// A yes-or-no question with a red action.
Future<bool> confirmDialog(
  BuildContext context, {
  required String title,
  required String text,
  required String action,
}) async {
  final c = context.fg;
  final ok = await showFgDialog<bool>(
    context,
    builder: (dialog) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                style: tx(
                  c.fg,
                  size: 20,
                  weight: w600,
                  height: 1.3,
                  spacing: -.4,
                ),
              ),
              const SizedBox(height: 6),
              Text(text, style: tx(c.fg2)),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              onTap: () => Navigator.of(dialog).pop(false),
            ),
            const SizedBox(width: 10),
            FgButton(
              text: action,
              kind: Kind.danger,
              onTap: () => Navigator.of(dialog).pop(true),
            ),
          ],
        ),
      ],
    ),
  );
  return ok ?? false;
}
