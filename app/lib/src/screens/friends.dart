import 'package:flutter/material.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../popover.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';
import 'friend_card.dart';
import 'home.dart' show Ticker;
import 'room.dart' show callFriendChecked;
import 'user_panel.dart' show StatusDot;

/// Friends: the list with presence, requests, and adding someone.
class Friends extends StatefulWidget {
  const Friends({super.key});

  @override
  State<Friends> createState() => _FriendsState();
}

class _FriendsState extends State<Friends> {
  int _tab = 0;
  final _search = TextEditingController();

  @override
  void initState() {
    super.initState();
    _search.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final query = _search.text.trim().toLowerCase();
    bool match(UiPerson p) =>
        query.isEmpty || p.name.toLowerCase().contains(query);
    final online = state.contacts.friends
        .where((f) => f.presence != UiPresence.offline)
        .toList();
    final requests =
        state.contacts.requests.incoming.length +
        state.contacts.requests.outgoing.length;
    final list = (_tab == 1 ? online : state.contacts.friends)
        .where((f) => match(f.person))
        .toList();
    return PageScroll(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1160),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(40, 36, 40, 56),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  l10n.commonFriends,
                  style: hx(c.fg, size: 32, height: 1.15),
                ),
                const SizedBox(height: 6),
                Text(l10n.friendsIntro, style: tx(c.fg2)),
                const SizedBox(height: 28),
                LayoutBuilder(
                  builder: (context, box) {
                    final listPanel = Panel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Container(
                            padding: const EdgeInsets.all(12),
                            decoration: BoxDecoration(
                              border: Border(
                                bottom: BorderSide(color: c.border),
                              ),
                            ),
                            child: Wrap(
                              spacing: 10,
                              runSpacing: 10,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: [
                                Segmented(
                                  tabs: [
                                    (
                                      l10n.friendsTabAll,
                                      state.contacts.friends.length,
                                      null,
                                    ),
                                    (
                                      l10n.friendsTabOnline,
                                      online.length,
                                      null,
                                    ),
                                    (
                                      l10n.friendsTabRequests,
                                      requests,
                                      requests > 0 ? c.blue : null,
                                    ),
                                  ],
                                  selected: _tab,
                                  onSelect: (i) => setState(() => _tab = i),
                                ),
                                SizedBox(
                                  width: 260,
                                  child: FgField(
                                    controller: _search,
                                    height: 36,
                                    placeholder: l10n.friendsSearch,
                                    leading: FgIcon(
                                      Ic.search,
                                      size: 14,
                                      color: c.fg3,
                                    ),
                                    label: l10n.friendsSearch,
                                  ),
                                ),
                              ],
                            ),
                          ),
                          if (_tab == 2)
                            _RequestList(match: match)
                          else if (list.isEmpty)
                            Padding(
                              padding: const EdgeInsets.all(20),
                              child: Text(
                                state.contacts.friends.isEmpty
                                    ? l10n.friendsNone
                                    : _tab == 1
                                    ? l10n.friendsNoneOnline
                                    : l10n.friendsNoMatch,
                                style: tx(c.fg3, size: 13),
                              ),
                            )
                          else
                            for (var i = 0; i < list.length; i++)
                              _FriendRow(
                                friend: list[i],
                                last: i == list.length - 1,
                              ),
                        ],
                      ),
                    );
                    final aside = Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        const _AddFriend(),
                        for (final r in state.contacts.requests.incoming.take(
                          1,
                        )) ...[
                          const SizedBox(height: 16),
                          _RequestCard(request: r),
                        ],
                      ],
                    );
                    if (box.maxWidth < 860) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          listPanel,
                          const SizedBox(height: 24),
                          aside,
                        ],
                      );
                    }
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(child: listPanel),
                        const SizedBox(width: 24),
                        SizedBox(
                          width: box.maxWidth > 1100 ? 380 : 340,
                          child: aside,
                        ),
                      ],
                    );
                  },
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

String presenceText(UiFriend f) => switch (f.presence) {
  UiPresence.inRoom =>
    f.roomGroupName == null
        ? l10n.friendsInARoom
        : l10n.friendsInRoomOf(f.roomGroupName!),
  UiPresence.online => l10n.friendsOnline,
  UiPresence.away => l10n.friendsAway,
  UiPresence.offline =>
    f.lastSeenMs == null
        ? l10n.friendsNotSeenYet
        : l10n.friendsSeen(ago(f.lastSeenMs!)),
};

class _FriendRow extends StatelessWidget {
  const _FriendRow({required this.friend, required this.last});
  final UiFriend friend;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final state = AppScope.of(context);
    final on = friend.presence != UiPresence.offline;
    final missed = state.notices.missedCalls[friend.person.id];
    final inCall = state.callState.activeCall != null;
    final together = state.sharedRoom(friend.person.id);
    final name = friend.person.name;
    final open = state.nav.friendCard == friend.person.id;
    return Pressable(
      key: ValueKey('friend-row:${friend.person.id}'),
      label: l10n.friendsShowCard(name),
      onTap: () => toggleFriendCard(context, friend.person.id),
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          color: open || hovered ? c.bg2 : null,
          border: last ? null : Border(bottom: BorderSide(color: c.border)),
        ),
        child: Row(
          children: [
            // The dot says to the eye whether they are online; while a missed call
            // takes the line under the name, this says it to a screen reader.
            Semantics(
              label: missed == null
                  ? null
                  : !on
                  ? l10n.friendCardOffline
                  : friend.presence == UiPresence.away
                  ? l10n.friendCardAway
                  : l10n.friendCardOnline,
              child: SizedBox(
                width: 32,
                height: 32,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Avatar(avatar: friend.person.avatar, size: 32, dim: !on),
                    if (on)
                      Positioned(
                        right: -1,
                        bottom: -1,
                        child: StatusDot(
                          offline: false,
                          away: friend.presence == UiPresence.away,
                          ring: 2,
                          ringColor: c.bg1,
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    friend.person.name,
                    overflow: TextOverflow.ellipsis,
                    style: tx(on ? c.fg : c.fg2, weight: w500),
                  ),
                  if (missed != null)
                    Row(
                      children: [
                        FgIcon(Ic.missedCall, size: 13, color: c.amber),
                        const SizedBox(width: 5),
                        Flexible(
                          child: Text(
                            l10n.friendsMissedCall(dayClock(missed)),
                            overflow: TextOverflow.ellipsis,
                            style: tx(c.amber, size: 13),
                          ),
                        ),
                      ],
                    )
                  else
                    Text(
                      presenceText(friend),
                      overflow: TextOverflow.ellipsis,
                      style: tx(on ? c.fg2 : c.fg3, size: 13),
                    ),
                ],
              ),
            ),
            const SizedBox(width: 12),
            Tooltip(
              message: friend.fingerprint,
              child: Text(
                friend.person.short,
                style: tx(c.fg3, size: 12, monospace: true),
              ),
            ),
            const SizedBox(width: 12),
            // Both actions with their words, as on the friend's card.
            FgButton(
              key: ValueKey('message:${friend.person.id}'),
              text: l10n.friendsMessage,
              icon: Ic.chat,
              height: 32,
              fontSize: 13,
              onTap: () => state.direct.show(friend.person.id),
            ),
            const SizedBox(width: 6),
            FgButton(
              text: state.callState.callText(
                friend.person.id,
                missed: missed != null,
              ),
              icon: Ic.phone,
              height: 32,
              fontSize: 13,
              tooltip: !on
                  ? l10n.friendsOfflineNoCall(name)
                  : inCall
                  ? l10n.friendsHangUpFirst(name)
                  : together != null
                  ? l10n.friendsBothInRoom(together)
                  : null,
              onTap: on && !inCall && together == null
                  ? () => callFriendChecked(context, friend.person.id)
                  : null,
            ),
            const SizedBox(width: 4),
            _OptionsButton(friend: friend),
          ],
        ),
      ),
    );
  }
}

/// The "···" of a friend's row: its menu hangs under it (or over it, near the bottom),
/// right edges lined up, and the button looks pressed while the menu is open.
class _OptionsButton extends StatefulWidget {
  const _OptionsButton({required this.friend});
  final UiFriend friend;

  @override
  State<_OptionsButton> createState() => _OptionsButtonState();
}

class _OptionsButtonState extends State<_OptionsButton> {
  bool _open = false;

  @override
  Widget build(BuildContext context) => FgButton(
    icon: Ic.dots,
    kind: Kind.ghost,
    height: 32,
    tooltip: l10n.friendsOptions(widget.friend.person.name),
    open: _open,
    onTap: _options,
  );

  Future<void> _options() async {
    // Open, a click on the button lands beside the menu and closes it.
    if (_open) return;
    final friend = widget.friend;
    final state = AppScope.read(context);
    final groups = state.contacts.groups.where((g) {
      final members = state.contacts.members[g.id] ?? const <UiMember>[];
      return g.manager && !members.any((m) => m.person.id == friend.person.id);
    }).toList();
    setState(() => _open = true);
    final choice = await showPopover<String>(
      context,
      label: l10n.friendsOptions(friend.person.name),
      alignRight: true,
      builder: (context) => FriendMenu(groups: groups),
    );
    if (mounted) setState(() => _open = false);
    if (choice == null || !mounted) return;
    if (choice.startsWith('add:')) {
      await addFriendToGroup(state, friend.person, choice.substring(4));
    } else if (choice == 'remove') {
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
                    l10n.friendsRemoveTitle(friend.person.name),
                    style: tx(
                      c.fg,
                      size: 20,
                      weight: w600,
                      height: 1.3,
                      spacing: -.4,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(l10n.friendsRemoveText, style: tx(c.fg2)),
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
                  text: l10n.friendsRemove,
                  kind: Kind.danger,
                  onTap: () => Navigator.of(dialog).pop(true),
                ),
              ],
            ),
          ],
        ),
      );
      if (ok ?? false) await state.removeFriend(friend.person.id);
    }
  }
}

/// A friend's menu: add them to a group this device manages and they are not in, then,
/// apart, remove them from friends (which asks first). Picking an item closes it with
/// `add:<group>` or `remove`.
class FriendMenu extends StatelessWidget {
  const FriendMenu({super.key, required this.groups});
  final List<UiGroup> groups;

  static const width = 260.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    Widget row(Ic icon, String text, String value, {bool danger = false}) {
      final color = danger ? c.red : null;
      return Pressable(
        key: ValueKey('friend-menu:$value'),
        onTap: () => Navigator.of(context).pop(value),
        label: text,
        builder: (context, hovered) => Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: hovered ? (danger ? c.redSoft : c.hover) : null,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Row(
            children: [
              FgIcon(icon, size: 16, color: color ?? c.fg2),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  text,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(color ?? c.fg),
                ),
              ),
            ],
          ),
        ),
      );
    }

    return Container(
      key: const ValueKey('friend-menu'),
      width: width,
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.borderStrong),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Many groups scroll here; removing stays in sight.
          if (groups.isNotEmpty) ...[
            Flexible(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final g in groups)
                      row(
                        Ic.userPlus,
                        l10n.friendsAddToGroup(g.name),
                        'add:${g.id}',
                      ),
                  ],
                ),
              ),
            ),
            Container(
              height: 1,
              margin: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              color: c.border,
            ),
          ],
          row(Ic.userMinus, l10n.friendsRemoveMenu, 'remove', danger: true),
        ],
      ),
    );
  }
}

class _RequestList extends StatelessWidget {
  const _RequestList({required this.match});
  final bool Function(UiPerson) match;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final incoming = state.contacts.requests.incoming
        .where((r) => match(r.person))
        .toList();
    final outgoing = state.contacts.requests.outgoing
        .where((r) => match(r.person))
        .toList();
    if (incoming.isEmpty && outgoing.isEmpty) {
      return Padding(
        padding: const EdgeInsets.all(20),
        child: Text(l10n.friendsNoRequests, style: tx(c.fg3, size: 13)),
      );
    }
    final rows = <Widget>[
      for (final r in incoming)
        _RequestRow(
          request: r,
          note: l10n.friendsRequestIncoming(ago(r.sinceMs)),
          actions: [
            FgButton(
              text: l10n.commonDecline,
              height: 32,
              onTap: () => state.answer(r.person.id, false),
            ),
            const SizedBox(width: 8),
            FgButton(
              text: l10n.commonAccept,
              height: 32,
              kind: Kind.primary,
              onTap: () => state.answer(r.person.id, true),
            ),
          ],
        ),
      for (final r in outgoing)
        _RequestRow(
          request: r,
          note: l10n.friendsRequestOutgoing(ago(r.sinceMs)),
          actions: [
            FgButton(
              text: l10n.commonCancel,
              height: 32,
              onTap: () => state.removeFriend(r.person.id),
            ),
          ],
        ),
    ];
    return Column(
      children: [
        for (var i = 0; i < rows.length; i++)
          Container(
            decoration: BoxDecoration(
              border: i == rows.length - 1
                  ? null
                  : Border(bottom: BorderSide(color: c.border)),
            ),
            child: rows[i],
          ),
      ],
    );
  }
}

class _RequestRow extends StatelessWidget {
  const _RequestRow({
    required this.request,
    required this.note,
    required this.actions,
  });
  final UiRequest request;
  final String note;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final fingerprint = Text(
      request.fingerprint,
      style: tx(c.fg3, size: 12, monospace: true),
    );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: LayoutBuilder(
        builder: (context, box) {
          // In a narrow list the fingerprint goes under the name, so the name and
          // the note keep a line each.
          final wide = box.maxWidth >= 640;
          return Row(
            children: [
              Avatar(avatar: request.person.avatar, size: 32),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      request.person.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg, weight: w500),
                    ),
                    Text(
                      note,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg2, size: 13),
                    ),
                    if (!wide) fingerprint,
                  ],
                ),
              ),
              const SizedBox(width: 12),
              if (wide) ...[fingerprint, const SizedBox(width: 12)],
              ...actions,
            ],
          );
        },
      ),
    );
  }
}

class _RequestCard extends StatelessWidget {
  const _RequestCard({required this.request});
  final UiRequest request;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    return Panel(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Dot(c.blue),
              const SizedBox(width: 8),
              Text(
                l10n.friendsPendingRequest,
                style: tx(c.fg2, size: 13, weight: w500),
              ),
            ],
          ),
          const SizedBox(height: 14),
          Row(
            children: [
              Avatar(avatar: request.person.avatar, size: 36),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(request.person.name, style: tx(c.fg, weight: w500)),
                    Text(
                      request.fingerprint,
                      style: tx(c.fg3, size: 12, monospace: true),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 14),
          Text(
            l10n.friendsCheckFingerprint(request.person.name),
            style: tx(c.fg2, size: 13),
          ),
          const SizedBox(height: 14),
          Row(
            children: [
              Expanded(
                child: FgButton(
                  text: l10n.commonDecline,
                  expand: true,
                  onTap: () => state.answer(request.person.id, false),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: FgButton(
                  text: l10n.commonAccept,
                  kind: Kind.primary,
                  expand: true,
                  onTap: () => state.answer(request.person.id, true),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _AddFriend extends StatefulWidget {
  const _AddFriend();

  @override
  State<_AddFriend> createState() => _AddFriendState();
}

class _AddFriendState extends State<_AddFriend> {
  /// 0 QR, 1 link, 2 code; it starts on the code.
  int _tab = 2;
  bool _making = false;
  final _input = TextEditingController();
  bool _sending = false;

  @override
  void initState() {
    super.initState();
    _input.addListener(() => setState(() {}));
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _make();
    });
  }

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  bool _fresh(UiInvitation? i) =>
      i != null && i.expiresMs > DateTime.now().millisecondsSinceEpoch;

  Future<void> _make({bool force = false}) async {
    final state = AppScope.read(context);
    final wantCode = _tab == 2;
    if (!force &&
        (wantCode
            ? _fresh(state.contacts.friendCode)
            : _fresh(state.contacts.friendLink))) {
      return;
    }
    setState(() => _making = true);
    await state.makeInvitation(code: wantCode);
    if (mounted) setState(() => _making = false);
  }

  Future<void> _send() async {
    final state = AppScope.read(context);
    setState(() => _sending = true);
    final ok = await state.useInvitation(_input.text);
    if (!mounted) return;
    setState(() => _sending = false);
    if (ok) _input.clear();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final state = AppScope.of(context);
    final invitation = _tab == 2
        ? state.contacts.friendCode
        : state.contacts.friendLink;
    final fresh = _fresh(invitation);
    return Panel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.friendsAddTitle,
                  style: tx(c.fg, size: 16, weight: w600, spacing: -.16),
                ),
                const SizedBox(height: 4),
                Text(l10n.friendsAddNote, style: tx(c.fg2, size: 13)),
              ],
            ),
          ),
          const SizedBox(height: 14),
          // Shown as Code, Link, QR; [_tab] still counts QR, Link, Code.
          LineTabs(
            tabs: [
              (l10n.friendsTabCode, null),
              (l10n.friendsTabLink, null),
              (l10n.friendsTabQr, null),
            ],
            selected: 2 - _tab,
            onSelect: (i) {
              setState(() => _tab = 2 - i);
              _make();
            },
          ),
          Padding(
            padding: const EdgeInsets.all(20),
            child: Ticker(
              builder: (context) {
                final left = invitation == null
                    ? ''
                    : countdown(invitation.expiresMs);
                final expired = invitation != null && !_fresh(invitation);
                Widget body;
                if (_making || invitation == null) {
                  body = SizedBox(
                    height: _tab == 0 ? 204 : 72,
                    child: Center(
                      child: _making
                          ? const Spinner(size: 16)
                          : FgButton(
                              text: l10n.friendsMakeInvitation,
                              onTap: () => _make(force: true),
                            ),
                    ),
                  );
                } else if (expired) {
                  body = SizedBox(
                    height: _tab == 0 ? 204 : 72,
                    child: Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            l10n.friendsInvitationExpired,
                            style: tx(c.fg2, size: 13),
                          ),
                          const SizedBox(height: 10),
                          FgButton(
                            text: l10n.friendsMakeAnother,
                            icon: Ic.retry,
                            onTap: () => _make(force: true),
                          ),
                        ],
                      ),
                    ),
                  );
                } else if (_tab == 0) {
                  body = Center(
                    child: SizedBox(
                      width: 300,
                      child: SecretBox(
                        text: invitation.text,
                        what: 'QR',
                        buttonsBelow: true,
                        child: Center(child: QrBox(text: invitation.text)),
                      ),
                    ),
                  );
                } else if (_tab == 1) {
                  body = _SecretText(
                    text: invitation.text,
                    what: 'Link',
                    monoSize: 12,
                    maxLines: 3,
                  );
                } else {
                  body = _SecretText(
                    text: invitation.text,
                    what: 'Code',
                    monoSize: 22,
                    maxLines: 1,
                  );
                }
                // The time left is drawn in the digits' font, wherever the
                // language puts it.
                const at = '\u0000';
                final hint = switch (_tab) {
                  0 => l10n.friendsHintQr(at),
                  1 => l10n.friendsHintLink(at),
                  _ => l10n.friendsHintCode(at),
                }.split(at);
                // Hidden until shown, for a window on a shared screen. A link or QR also
                // carries this device's network addresses; a code does not.
                final private = _tab == 2
                    ? l10n.friendsHiddenCode
                    : l10n.friendsHiddenLink;
                return Column(
                  children: [
                    body,
                    if (fresh && !_making) ...[
                      const SizedBox(height: 12),
                      Text.rich(
                        TextSpan(
                          children: [
                            TextSpan(text: hint.first),
                            TextSpan(
                              text: left,
                              style: const TextStyle(fontFamily: mono),
                            ),
                            if (hint.length > 1) TextSpan(text: hint.last),
                          ],
                        ),
                        textAlign: TextAlign.center,
                        style: tx(c.fg2, size: 13),
                      ),
                      ...[
                        const SizedBox(height: 4),
                        Text(
                          private,
                          textAlign: TextAlign.center,
                          style: tx(c.fg3, size: 12),
                        ),
                      ],
                    ],
                  ],
                );
              },
            ),
          ),
          Container(
            padding: const EdgeInsets.fromLTRB(20, 16, 20, 20),
            decoration: BoxDecoration(
              color: c.bg2,
              border: Border(top: BorderSide(color: c.border)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  l10n.friendsGotLink,
                  style: tx(c.fg2, size: 13, weight: w500),
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    Expanded(
                      child: FgField(
                        controller: _input,
                        height: 36,
                        strong: true,
                        monospace: true,
                        fontSize: 13,
                        placeholder: l10n.friendsPastePlaceholder,
                        onSubmitted: (_) => _send(),
                        label: l10n.friendsLinkOrCode,
                      ),
                    ),
                    const SizedBox(width: 8),
                    FgButton(
                      text: l10n.friendsAdd,
                      kind: Kind.primary,
                      busy: _sending,
                      onTap: _input.text.trim().isEmpty ? null : _send,
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// A link or a code, hidden until shown, with the copy button beside it.
class _SecretText extends StatelessWidget {
  const _SecretText({
    required this.text,
    required this.what,
    required this.monoSize,
    required this.maxLines,
  });

  final String text;
  final String what;
  final double monoSize;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final style = tx(c.fg, size: monoSize, monospace: true, height: 1.4);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: c.border),
      ),
      child: LayoutBuilder(
        builder: (context, box) {
          Widget shown = SelectableText(
            text,
            maxLines: maxLines,
            textAlign: maxLines == 1 ? TextAlign.center : TextAlign.start,
            style: style,
          );
          var below = false;
          if (maxLines == 1) {
            // A code reads best whole and large: when it and its buttons do not fit on
            // one line, the buttons go under it, and it only shrinks if it still does not fit.
            final painter = TextPainter(
              text: TextSpan(text: text, style: style),
              textDirection: TextDirection.ltr,
              maxLines: 1,
            )..layout();
            below = painter.width + _buttonsWidth > box.maxWidth;
            painter.dispose();
            shown = FittedBox(fit: BoxFit.scaleDown, child: shown);
          }
          return SizedBox(
            width: box.maxWidth,
            child: SecretBox(
              text: text,
              what: what,
              buttonsBelow: below,
              child: shown,
            ),
          );
        },
      ),
    );
  }

  /// Room the show and copy buttons take beside the text.
  static const _buttonsWidth = 140.0;
}

/// A QR code on white with the logo in the middle.
class QrBox extends StatefulWidget {
  const QrBox({super.key, required this.text});
  final String text;

  @override
  State<QrBox> createState() => _QrBoxState();
}

class _QrBoxState extends State<QrBox> {
  /// Made once per text: the page around it is built again every second.
  UiQr? _qr;

  @override
  void initState() {
    super.initState();
    _encode();
  }

  @override
  void didUpdateWidget(QrBox old) {
    super.didUpdateWidget(old);
    if (old.text != widget.text) _encode();
  }

  void _encode() {
    try {
      _qr = api.qr(text: widget.text);
    } catch (_) {
      _qr = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final qr = _qr;
    return Container(
      width: 204,
      height: 204,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: Colors.white,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: Stack(
        alignment: Alignment.center,
        children: [
          if (qr != null)
            Semantics(
              label: l10n.friendsQrLabel,
              child: CustomPaint(
                size: const Size.square(174),
                painter: _QrPainter(qr),
              ),
            ),
          Container(
            width: 44,
            height: 44,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Image.asset(
              'assets/images/logo.png',
              width: 30,
              height: 30,
              // Decoded at the size it shows, not the file's 256 pixels.
              cacheWidth: (30 * MediaQuery.devicePixelRatioOf(context)).round(),
            ),
          ),
        ],
      ),
    );
  }
}

class _QrPainter extends CustomPainter {
  _QrPainter(this.qr);
  final UiQr qr;

  @override
  void paint(Canvas canvas, Size size) {
    final n = qr.size;
    final cell = size.width / n;
    final paint = Paint()
      ..color = const Color(0xFF0A0A0A)
      ..isAntiAlias = false;
    for (var y = 0; y < n; y++) {
      for (var x = 0; x < n; x++) {
        if (qr.dark[y * n + x]) {
          canvas.drawRect(
            Rect.fromLTWH(x * cell, y * cell, cell + .2, cell + .2),
            paint,
          );
        }
      }
    }
  }

  @override
  bool shouldRepaint(_QrPainter old) => old.qr != qr;
}

/// Adds [friend] to [group] and says when they get it; only an error when that failed.
Future<void> addFriendToGroup(
  AppState state,
  UiPerson friend,
  String group,
) async {
  final added = await state.succeeds(
    () => api.addToGroup(groupId: group, friendId: friend.id),
  );
  if (added) {
    state.say(l10n.friendsAddedToGroup(friend.name));
  }
  await state.refreshGroups();
}
