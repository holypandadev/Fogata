import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';
import 'friends.dart' show presenceText;
import 'room.dart' show confirmCalling;
import 'user_panel.dart';
import 'why_unreached.dart';

/// Opens a friend's card next to `context`'s box, or closes it if it is open.
void toggleFriendCard(BuildContext context, String friend) {
  final from = boundsInLayer(context);
  if (from != null) AppScope.read(context).toggleFriendCard(friend, from);
}

/// Opens the card of person [id] next to `context`'s box, or closes it if it is open:
/// the person's own panel when it is them, a friend's card, or else the card of a member
/// of [group] (of any group this device is in, without one), which offers a friend request.
void togglePersonCard(BuildContext context, String id, {String? group}) {
  final state = AppScope.read(context);
  if (id == state.contacts.profile?.person.id) {
    toggleUserPanel(context, beside: true);
    return;
  }
  final from = boundsInLayer(context);
  if (from == null) return;
  final friend = state.contacts.friends.any((f) => f.person.id == id);
  state.toggleFriendCard(id, from, member: !friend, group: group);
}

/// What screen readers say opening [person]'s card does, the person's own included.
String personCardLabel(AppState state, UiPerson person) =>
    person.id == state.contacts.profile?.person.id
    ? l10n.memberCardShowOwn
    : l10n.friendsShowCard(person.name);

/// The member [id] of the groups this device is in, from [group]'s list first.
UiMember? memberOf(AppState state, String id, String? group) {
  final lists = [
    ?state.contacts.members[group],
    ...state.contacts.members.entries
        .where((e) => e.key != group)
        .map((e) => e.value),
  ];
  for (final list in lists) {
    for (final m in list) {
      if (m.person.id == id && !m.me) return m;
    }
  }
  return null;
}

/// Someone in the direct call this device is in who is no friend and no member of a group
/// it is in: the call is where they were met, and the only way to know them.
UiMember? callPersonOf(AppState state, String id) {
  if (state.callState.callIn(state.roomState.roomGroup) == null) return null;
  for (final p
      in state.roomState.room?.participants ?? const <UiParticipant>[]) {
    if (p.person.id == id && id != state.contacts.profile?.person.id) {
      return UiMember(
        person: p.person,
        rank: 0,
        manager: false,
        me: false,
        away: false,
        waitingForKey: false,
        fingerprint: '',
      );
    }
  }
  return null;
}

/// A friend's card over the window, beside the row that opened it: their banner, picture,
/// name and status, the friends in common if there are any, and a button to call. A
/// group member who is not a friend gets a card of the same look that offers a friend
/// request. A click beside it or Esc closes it.
class FriendCardLayer extends StatelessWidget {
  const FriendCardLayer({super.key});

  static const width = 300.0;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final from = state.nav.friendCardFrom;
    final id = state.nav.friendCard;
    final friend = state.contacts.friends
        .where((f) => f.person.id == id)
        .firstOrNull;
    final member = friend != null || id == null || !state.nav.memberCard
        ? null
        : memberOf(state, id, state.nav.memberCardGroup);
    final inCall =
        friend == null && member == null && id != null && state.nav.memberCard
        ? callPersonOf(state, id)
        : null;
    if (from == null || friend == null && member == null && inCall == null) {
      return const SizedBox.shrink();
    }
    return Positioned.fill(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape):
              state.closeFriendCard,
        },
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusLayer(
          child: Stack(
            children: [
              Positioned.fill(child: Backdrop(onTap: state.closeFriendCard)),
              Positioned.fill(
                child: LayoutBuilder(
                  builder: (context, _) => CustomSingleChildLayout(
                    delegate: BesideBox(followBounds(from)),
                    child: TweenAnimationBuilder<double>(
                      key: ValueKey(id),
                      tween: Tween(begin: 0, end: 1),
                      duration: springTime,
                      curve: spring,
                      builder: (context, t, child) => Opacity(
                        opacity: t.clamp(0, 1),
                        child: Transform.translate(
                          offset: Offset(-8 * (1 - t), 0),
                          child: Transform.scale(
                            scale: .95 + .05 * t,
                            alignment: Alignment.topLeft,
                            child: child,
                          ),
                        ),
                      ),
                      child: friend != null
                          ? _FriendCard(friend: friend)
                          : _MemberCard(
                              member: member ?? inCall!,
                              group: state.nav.memberCardGroup,
                              inCall: member == null,
                            ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _FriendCard extends StatelessWidget {
  const _FriendCard({required this.friend});
  final UiFriend friend;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final id = friend.person.id;
    final banner = state.contacts.friendBanners[id];
    final on = friend.presence != UiPresence.offline;
    final away = friend.presence == UiPresence.away;
    final inCall = state.callState.activeCall != null;
    final together = state.sharedRoom(id);
    final mutual = [
      for (final m in state.contacts.mutualFriends[id] ?? const <String>[])
        ...state.contacts.friends.where((f) => f.person.id == m),
    ];
    final name = friend.person.name;
    return _Frame(
      person: friend.person,
      banner: banner,
      on: on,
      away: away,
      children: [
        Text(
          name,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: tx(c.fg, size: 19, weight: w600, height: 1.25),
        ),
        // The dot by the picture says online or not; only where they are, or
        // when they were last seen, is said in words.
        if (friend.presence == UiPresence.inRoom ||
            friend.lastSeenMs != null && !on) ...[
          const SizedBox(height: 2),
          Text(
            presenceText(friend),
            overflow: TextOverflow.ellipsis,
            style: tx(
              friend.presence == UiPresence.inRoom ? c.green : c.fg3,
              size: 13,
            ),
          ),
        ],
        CardBio(friend.bio),
        if (mutual.isNotEmpty) ...[
          const SizedBox(height: 10),
          _Mutual(mutual: mutual),
        ],
        const SizedBox(height: 14),
        FgButton(
          key: const ValueKey('friend-card-call'),
          text: state.callState.callText(id),
          icon: Ic.phone,
          kind: Kind.primary,
          height: 38,
          fontSize: 13,
          expand: true,
          onTap: on && !inCall && together == null
              ? () async {
                  if (!await confirmCalling(context, id)) return;
                  state.closeFriendCard();
                  state.notices.missedCalls.remove(id);
                  await state.callFriend(id);
                }
              : null,
        ),
        if (!on || inCall || together != null) ...[
          const SizedBox(height: 8),
          Text(
            !on
                ? l10n.friendCardCallsOnlyOnline
                : inCall
                ? l10n.friendCardHangUpFirst(name)
                : l10n.friendCardBothInRoom(together ?? ''),
            textAlign: TextAlign.center,
            style: tx(c.fg3, size: 11),
          ),
        ],
        // Why the friend may not be reached: this side's network, or, with
        // nothing in the way here, that the friend is probably offline.
        if (!on)
          WhyUnreached(
            key: ValueKey('why-unreached:$id'),
            firewall: state.notices.firewallCommand != null,
            load: state.reachability,
            friend: name,
            center: true,
            size: 11,
            top: 8,
          ),
        const SizedBox(height: 8),
        FgButton(
          key: const ValueKey('friend-card-message'),
          text: l10n.friendCardMessage,
          icon: Ic.chat,
          height: 38,
          fontSize: 13,
          expand: true,
          onTap: () {
            state.closeFriendCard();
            state.direct.show(id);
          },
        ),
      ],
    );
  }
}

/// What a person's card looks like: their banner, a plain one without, their picture with
/// a dot that says whether they are online, and under it [children].
class _Frame extends StatelessWidget {
  const _Frame({
    required this.person,
    required this.banner,
    required this.on,
    required this.away,
    required this.children,
  });
  final UiPerson person;
  final UiBanner? banner;
  final bool on;
  final bool away;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final banner = this.banner;
    return Material(
      type: MaterialType.transparency,
      child: Container(
        width: FriendCardLayer.width,
        clipBehavior: Clip.antiAlias,
        // Inside the outline, as when it was drawn under what is inside.
        padding: const EdgeInsets.all(1),
        decoration: BoxDecoration(
          color: c.bg2,
          borderRadius: BorderRadius.circular(22),
        ),
        // Over what is inside, which would cover it at the rounded corners.
        foregroundDecoration: BoxDecoration(
          borderRadius: BorderRadius.circular(22),
          border: Border.all(color: c.border),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Stack(
              clipBehavior: Clip.none,
              children: [
                SizedBox(
                  height: 96,
                  child: PanelBanner(
                    // Someone that never sent a banner gets a plain one.
                    color: banner == null ? c.hover : Color(banner.color),
                    picture: banner?.picture,
                    id: person.id,
                  ),
                ),
                Positioned(
                  left: 13,
                  top: 50,
                  child: SizedBox(
                    width: 92,
                    height: 92,
                    child: Stack(
                      children: [
                        Container(
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: c.bg2,
                          ),
                          alignment: Alignment.center,
                          child: Avatar(
                            avatar: person.avatar,
                            size: 82,
                            id: person.id,
                          ),
                        ),
                        Positioned(
                          right: 5,
                          bottom: 5,
                          child: Tooltip(
                            message: !on
                                ? l10n.friendCardOffline
                                : away
                                ? l10n.friendCardAway
                                : l10n.friendCardOnline,
                            child: StatusDot(
                              offline: !on,
                              away: away,
                              size: 20,
                              ring: 4,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 52, 16, 14),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: children,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The card of a member of a group who is not a friend: their picture and name, where
/// they were met, and a friend request to send, or the one between the two: sent and
/// waiting, or theirs to accept.
class _MemberCard extends StatelessWidget {
  const _MemberCard({
    required this.member,
    required this.group,
    this.inCall = false,
  });
  final UiMember member;
  final String? group;

  /// Known only from the direct call this device is in: no group of theirs carries a
  /// friend request (the other side takes one only from a member of a group it is in).
  final bool inCall;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final person = member.person;
    final id = person.id;
    final name = person.name;
    // Someone in the call with this device is connected to it.
    final on = inCall || member.path != null;
    final asked = state.contacts.requests.outgoing.any(
      (r) => r.person.id == id,
    );
    final asks = state.contacts.requests.incoming.any((r) => r.person.id == id);
    final where = inCall ? null : state.contacts.group(group)?.name;
    return _Frame(
      person: person,
      banner: null,
      on: on,
      away: on && member.away,
      children: [
        Text(
          name,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: tx(c.fg, size: 19, weight: w600, height: 1.25),
        ),
        if (where != null) ...[
          const SizedBox(height: 2),
          Text(
            l10n.memberCardInGroup(where),
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg3, size: 13),
          ),
        ],
        if (inCall) ...[
          const SizedBox(height: 2),
          Text(
            key: const ValueKey('member-card-in-call'),
            l10n.shellSectionInCall,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg3, size: 13),
          ),
        ],
        if (!inCall) const SizedBox(height: 14),
        if (inCall)
          const SizedBox.shrink()
        else if (asks) ...[
          Text(
            l10n.memberCardAsksYou(name),
            textAlign: TextAlign.center,
            style: tx(c.fg2, size: 12),
          ),
          const SizedBox(height: 8),
          FgButton(
            key: const ValueKey('member-card-accept'),
            text: l10n.memberCardAccept,
            icon: Ic.check,
            kind: Kind.primary,
            height: 38,
            fontSize: 13,
            expand: true,
            onTap: () => state.answer(id, true),
          ),
        ] else if (asked)
          Row(
            key: const ValueKey('member-card-sent'),
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              FgIcon(Ic.check, size: 14, color: c.green),
              const SizedBox(width: 6),
              Flexible(
                child: Text(
                  l10n.memberCardRequestSent,
                  textAlign: TextAlign.center,
                  style: tx(c.fg2, size: 13),
                ),
              ),
            ],
          )
        else ...[
          FgButton(
            key: const ValueKey('member-card-ask'),
            text: l10n.memberCardSendRequest,
            icon: Ic.userPlus,
            kind: Kind.primary,
            height: 38,
            fontSize: 13,
            expand: true,
            onTap: on ? () => state.askMember(id, name) : null,
          ),
          // A request goes over the connection the two have as members.
          if (!on) ...[
            const SizedBox(height: 8),
            Text(
              l10n.memberCardOnlyOnline(name),
              textAlign: TextAlign.center,
              style: tx(c.fg3, size: 11),
            ),
          ],
        ],
      ],
    );
  }
}

/// How many friends the two have in common, with the first pictures overlapping.
class _Mutual extends StatelessWidget {
  const _Mutual({required this.mutual});
  final List<UiFriend> mutual;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final shown = mutual.take(3).toList();
    final count = mutual.length;
    return Tooltip(
      message: mutual.map((f) => f.person.name).join(', '),
      child: Row(
        children: [
          SizedBox(
            width: 18 + 13.0 * (shown.length - 1),
            height: 18,
            child: Stack(
              children: [
                for (var i = 0; i < shown.length; i++)
                  Positioned(
                    left: 13.0 * i,
                    child: Avatar(
                      avatar: shown[i].person.avatar,
                      size: 18,
                      ring: c.bg2,
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          Text(l10n.friendCardMutualFriends(count), style: tx(c.fg2, size: 12)),
        ],
      ),
    );
  }
}
