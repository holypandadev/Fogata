import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../l10n.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';

/// How many lines of a group's activity come at a time.
const activityPage = 50;

/// What [a] says, in a sentence: "Ana moved Leo up to 2nd". [me] is this device's id: it
/// reads as "You".
String activitySentence(UiActivity a, String me) {
  String isMe(UiPerson? p) => p != null && p.id == me ? 'yes' : 'no';
  final by = a.by.name;
  final byMe = isMe(a.by);
  final member = a.member?.name ?? '';
  final memberMe = isMe(a.member);
  final name = a.name ?? '';
  String room(String name) => name.isEmpty ? l10n.activityDefaultRoom : name;
  return switch (a.kind) {
    UiActivityKind.created => l10n.activityCreated(by, byMe, name),
    UiActivityKind.renamed => l10n.activityRenamed(by, byMe, name),
    UiActivityKind.icon => l10n.activityIcon(by, byMe),
    UiActivityKind.picture =>
      a.takenAway
          ? l10n.activityPictureRemoved(by, byMe)
          : l10n.activityPicture(by, byMe),
    UiActivityKind.banner =>
      a.takenAway
          ? l10n.activityBannerRemoved(by, byMe)
          : l10n.activityBanner(by, byMe),
    UiActivityKind.added => l10n.activityAdded(by, byMe, member, memberMe),
    UiActivityKind.joined => l10n.activityJoined(by, byMe, member, memberMe),
    UiActivityKind.letIn => l10n.activityLetIn(by, byMe, member, memberMe),
    UiActivityKind.removed => l10n.activityRemoved(by, byMe, member, memberMe),
    UiActivityKind.left => l10n.activityLeft(by, byMe),
    UiActivityKind.movedUp => l10n.activityMovedUp(
      by,
      byMe,
      member,
      memberMe,
      ordinal((a.number ?? 0) + 1),
    ),
    UiActivityKind.movedDown => l10n.activityMovedDown(
      by,
      byMe,
      member,
      memberMe,
    ),
    UiActivityKind.managers =>
      a.number == null
          ? l10n.activityManagersAll(by, byMe)
          : l10n.activityManagersFirst(by, byMe, a.number!),
    UiActivityKind.limits => l10n.activityLimits(
      by,
      byMe,
      a.number ?? 0,
      a.number2 ?? 0,
    ),
    UiActivityKind.roomOpened => l10n.activityRoomOpened(by, byMe, room(name)),
    UiActivityKind.roomRenamed => l10n.activityRoomRenamed(
      by,
      byMe,
      room(a.from ?? ''),
      room(name),
    ),
    UiActivityKind.roomClosed => l10n.activityRoomClosed(by, byMe, room(name)),
    UiActivityKind.invited => l10n.activityInvited(by, byMe),
    UiActivityKind.revoked => l10n.activityRevoked(by, byMe),
    UiActivityKind.roomAccess => switch (a.members) {
      null => l10n.activityRoomOpenToAll(by, byMe, room(name)),
      final List<UiPerson> some when some.isEmpty => l10n.activityRoomForAbove(
        by,
        byMe,
        room(name),
      ),
      final some => l10n.activityRoomOnly(
        by,
        byMe,
        [for (final p in some) p.id == me ? l10n.activityYouInList : p.name]
            .join(l10n.listSeparator),
        room(name),
      ),
    },
    UiActivityKind.keptOut =>
      a.untilMs == null
          ? l10n.activityKeptOut(by, byMe, member, memberMe, room(name))
          : l10n.activityTimedOut(
              by,
              byMe,
              member,
              memberMe,
              room(name),
              dayClock(a.untilMs!),
              dayClockKind(a.untilMs!),
            ),
    UiActivityKind.letBack => l10n.activityLetBack(
      by,
      byMe,
      member,
      memberMe,
      room(name),
    ),
  };
}

/// Why [a] was undone, in words, or null when it stands.
String? undoneWords(UiActivity a, String me) {
  final kind = a.undone;
  if (kind == null) return null;
  final author = a.by.name;
  final authorMe = a.by.id == me ? 'yes' : 'no';
  final undoer = a.undoneBy;
  final by = undoer?.name ?? '';
  final byWho = undoer == null
      ? 'none'
      : undoer.id == me
      ? 'me'
      : 'other';
  return switch (kind) {
    UiUndoneKind.removed => l10n.activityUndoneRemoved(
      author,
      authorMe,
      by,
      byWho,
    ),
    UiUndoneKind.movedDown => l10n.activityUndoneMovedDown(
      author,
      authorMe,
      by,
      byWho,
    ),
    UiUndoneKind.left => l10n.activityUndoneLeft(author, authorMe),
  };
}

/// Who did what in [group], the newest first, older lines a page at a time.
class GroupActivity extends StatefulWidget {
  const GroupActivity({super.key, required this.group, this.load});
  final UiGroup group;

  /// Where lines come from (the core, unless a test says otherwise).
  final Future<UiActivityPage> Function(String group, int skip, int take)? load;

  @override
  State<GroupActivity> createState() => _GroupActivityState();
}

class _GroupActivityState extends State<GroupActivity> {
  List<UiActivity> _lines = const [];
  bool _more = false;
  bool _loading = true;

  Future<UiActivityPage> _page(int skip, int take) =>
      (widget.load ??
      (g, s, t) => api.groupActivity(groupId: g, skip: s, take: t))(
        widget.group.id,
        skip,
        take,
      );

  @override
  void initState() {
    super.initState();
    _reload();
  }

  @override
  void didUpdateWidget(GroupActivity old) {
    super.didUpdateWidget(old);
    // The group changed (an entry arrived, say): the lines shown so far again.
    if (!identical(old.group, widget.group)) _reload();
  }

  Future<void> _reload() async {
    final shown = _lines.length < activityPage ? activityPage : _lines.length;
    try {
      final page = await _page(0, shown);
      if (!mounted) return;
      setState(() {
        _lines = page.lines;
        _more = page.more;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _older() async {
    setState(() => _loading = true);
    try {
      final page = await _page(_lines.length, activityPage);
      if (!mounted) return;
      setState(() {
        _lines = [..._lines, ...page.lines];
        _more = page.more;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final me = state.contacts.profile?.person.id ?? '';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.activityAboutList, style: tx(c.fg3, size: 12, height: 1.4)),
        // The log's order is the real one, the same for everyone; clocks can be set to
        // anything, so the times are shown but never decide the order.
        Text(l10n.activityAboutOrder, style: tx(c.fg3, size: 12, height: 1.4)),
        const SizedBox(height: 12),
        if (_lines.isEmpty && !_loading)
          Text(l10n.activityNothingYet, style: tx(c.fg2, size: 13))
        else
          Container(
            padding: const EdgeInsets.all(6),
            decoration: BoxDecoration(
              color: c.bg1,
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: c.border),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final line in _lines) _ActivityLine(line: line, me: me),
              ],
            ),
          ),
        if (_more) ...[
          const SizedBox(height: 12),
          Center(
            child: FgButton(
              text: _loading ? l10n.activityLoading : l10n.activityShowOlder,
              height: 32,
              fontSize: 13,
              onTap: _loading ? null : _older,
            ),
          ),
        ],
      ],
    );
  }
}

class _ActivityLine extends StatelessWidget {
  const _ActivityLine({required this.line, required this.me});
  final UiActivity line;
  final String me;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final undone = undoneWords(line, me);
    final when = dayClock(line.atMs);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 7),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Avatar(avatar: line.by.avatar, size: 28, dim: undone != null),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  activitySentence(line, me),
                  style:
                      tx(
                        undone == null ? c.fg : c.fg3,
                        size: 13,
                        weight: w500,
                      ).copyWith(
                        decoration: undone == null
                            ? null
                            : TextDecoration.lineThrough,
                      ),
                ),
                const SizedBox(height: 2),
                Text(
                  line.seenHere ? l10n.activitySeenHere(when) : when,
                  style: tx(c.fg3, size: 11),
                ),
                if (undone != null) ...[
                  const SizedBox(height: 2),
                  Text(undone, style: tx(c.amber, size: 11)),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
