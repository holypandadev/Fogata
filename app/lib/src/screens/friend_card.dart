import 'dart:math' as math;

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

/// A friend's card over the window, beside the row that opened it: their banner, picture,
/// name and status, the friends in common if there are any, and a button to call. A click
/// beside it or Esc closes it.
class FriendCardLayer extends StatelessWidget {
  const FriendCardLayer({super.key});

  static const width = 300.0;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final from = state.nav.friendCardFrom;
    final friend = state.contacts.friends
        .where((f) => f.person.id == state.nav.friendCard)
        .firstOrNull;
    if (from == null || friend == null) return const SizedBox.shrink();
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
                    delegate: _BesideRow(followBounds(from)),
                    child: TweenAnimationBuilder<double>(
                      key: ValueKey(friend.person.id),
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
                      child: _FriendCard(friend: friend),
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

/// Puts the card to the right of the row's picture and name, its top near the row's, and
/// keeps it inside the window.
class _BesideRow extends SingleChildLayoutDelegate {
  _BesideRow(this.from);
  final Rect from;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size child) {
    final left = math.min(from.left + 160, size.width - child.width - 8);
    final top = math.min(from.top - 10, size.height - child.height - 8);
    return Offset(math.max(8, left), math.max(8, top));
  }

  @override
  bool shouldRelayout(_BesideRow old) => old.from != from;
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
                    // A friend that never sent a banner gets a plain one.
                    color: banner == null ? c.hover : Color(banner.color),
                    picture: banner?.picture,
                    id: friend.person.id,
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
                            avatar: friend.person.avatar,
                            size: 82,
                            id: friend.person.id,
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
              ),
            ),
          ],
        ),
      ),
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
