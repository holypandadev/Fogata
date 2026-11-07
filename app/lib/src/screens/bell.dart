import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../theme.dart';
import '../widgets.dart';
import 'room.dart' show confirmCalling;
import 'user_panel.dart' show boundsInLayer, followBounds;

/// The bell in the title bar, with how many notices are new.
class BellButton extends StatelessWidget {
  const BellButton({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final unseen = state.notices.unseen;
    final open = state.notices.bellFrom != null;
    return Builder(
      builder: (context) => Springy(
        onTap: () => state.toggleBell(boundsInLayer(context)),
        tooltip: l10n.bellNotifications,
        label: unseen == 0
            ? l10n.bellNotifications
            : l10n.bellNewNotifications(unseen),
        focusRadius: 14,
        hoverScale: 1.08,
        builder: (context, hovered) => SizedBox(
          width: 28,
          height: 28,
          child: Stack(
            clipBehavior: Clip.none,
            children: [
              AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                width: 28,
                height: 28,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: open || hovered ? c.hover : c.bg1,
                  shape: BoxShape.circle,
                  border: Border.all(color: open ? c.borderStrong : c.border),
                ),
                child: FgIcon(
                  Ic.bell,
                  size: 14,
                  color: open || hovered ? c.fg : c.fg2,
                ),
              ),
              if (unseen > 0)
                Positioned(
                  top: -5,
                  right: -6,
                  child: Container(
                    constraints: const BoxConstraints(minWidth: 16),
                    height: 16,
                    padding: const EdgeInsets.symmetric(horizontal: 4),
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: c.blueSolid,
                      borderRadius: BorderRadius.circular(999),
                      border: Border.all(color: c.bg1, width: 2),
                    ),
                    child: Text(
                      '$unseen',
                      style: tx(Colors.white, size: 9, weight: w600, height: 1),
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

/// The bell's list, under it: the last three notices, newest first, each with what can
/// be done about it. A click beside it or Esc closes it.
class BellLayer extends StatelessWidget {
  const BellLayer({super.key});

  /// Its width, narrower in a window under [narrowWindow] wide.
  static const width = 380.0;
  static const narrowWidth = 340.0;
  static const narrowWindow = 1000.0;

  /// How far it keeps from the window's edges.
  static const margin = 8.0;

  /// Where it goes under the bell [from] in a window of [window]: its right edge on the
  /// bell's, 8 under it, and always inside the window, moved in from an edge and no
  /// taller than the room under the bell (its notices scroll).
  static Rect place(Rect from, Size window) {
    final width = math.min(
      window.width < narrowWindow ? narrowWidth : BellLayer.width,
      math.max(0.0, window.width - 2 * margin),
    );
    final left = (from.right - width)
        .clamp(margin, math.max(margin, window.width - width - margin))
        .toDouble();
    final top = math.min(from.bottom + 8, window.height - margin);
    return Rect.fromLTWH(
      left,
      top,
      width,
      math.max(0.0, window.height - top - margin),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final from = state.notices.bellFrom;
    if (from == null) return const SizedBox.shrink();
    final c = context.fg;
    final notices = state.notices.list.take(3).toList();
    void close() => state.toggleBell(null);
    return Positioned.fill(
      child: CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): close},
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusLayer(
          child: LayoutBuilder(
            builder: (context, box) {
              final place = BellLayer.place(followBounds(from), box.biggest);
              return Stack(
                children: [
                  Positioned.fill(child: Backdrop(onTap: close)),
                  Positioned(
                    left: place.left,
                    top: place.top,
                    width: place.width,
                    child: TweenAnimationBuilder<double>(
                      tween: Tween(begin: 0, end: 1),
                      duration: springTime,
                      curve: spring,
                      builder: (context, t, child) => Opacity(
                        opacity: t.clamp(0, 1),
                        child: Transform.translate(
                          offset: Offset(0, -8 * (1 - t)),
                          child: Transform.scale(
                            scale: .96 + .04 * t,
                            alignment: Alignment.topRight,
                            child: child,
                          ),
                        ),
                      ),
                      child: Container(
                        key: const ValueKey('bell-panel'),
                        constraints: BoxConstraints(maxHeight: place.height),
                        clipBehavior: Clip.antiAlias,
                        // Inside the outline, as when it was drawn under what is inside.
                        padding: const EdgeInsets.all(1),
                        decoration: BoxDecoration(
                          color: c.bg2,
                          borderRadius: BorderRadius.circular(20),
                        ),
                        // Over what is inside, which would cover it at the rounded corners.
                        foregroundDecoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(20),
                          border: Border.all(color: c.border),
                        ),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            Container(
                              // Taller when large text takes two lines.
                              constraints: const BoxConstraints(minHeight: 48),
                              padding: const EdgeInsets.fromLTRB(16, 6, 8, 6),
                              decoration: BoxDecoration(
                                border: Border(
                                  bottom: BorderSide(color: c.border),
                                ),
                              ),
                              child: Row(
                                children: [
                                  Expanded(
                                    child: Text(
                                      l10n.bellNotifications,
                                      style: tx(c.fg, weight: w600),
                                    ),
                                  ),
                                  if (notices.isNotEmpty)
                                    FgButton(
                                      text: l10n.bellClearAll,
                                      kind: Kind.ghost,
                                      height: 28,
                                      fontSize: 12,
                                      // Those older than the three shown go
                                      // too, rather than take their place.
                                      onTap: () {
                                        for (final n in state.notices.list) {
                                          state.dismissNotice(n);
                                        }
                                      },
                                    ),
                                ],
                              ),
                            ),
                            // Under the header, what scrolls when the window is short.
                            Flexible(
                              child: SingleChildScrollView(
                                child: notices.isEmpty
                                    ? Padding(
                                        padding: const EdgeInsets.symmetric(
                                          vertical: 34,
                                        ),
                                        child: Column(
                                          children: [
                                            Container(
                                              width: 40,
                                              height: 40,
                                              alignment: Alignment.center,
                                              decoration: BoxDecoration(
                                                color: c.hover,
                                                shape: BoxShape.circle,
                                              ),
                                              child: FgIcon(
                                                Ic.bell,
                                                size: 17,
                                                color: c.fg2,
                                              ),
                                            ),
                                            const SizedBox(height: 8),
                                            Text(
                                              l10n.bellNothingNew,
                                              style: tx(c.fg3, size: 13),
                                            ),
                                          ],
                                        ),
                                      )
                                    : Padding(
                                        padding: const EdgeInsets.all(6),
                                        child: Column(
                                          children: [
                                            for (final n in notices)
                                              _NoticeRow(
                                                key: ValueKey(n.key),
                                                notice: n,
                                                fresh:
                                                    n.atMs >
                                                    state.notices.newSince,
                                              ),
                                          ],
                                        ),
                                      ),
                              ),
                            ),
                            Container(
                              padding: const EdgeInsets.fromLTRB(
                                16,
                                10,
                                16,
                                12,
                              ),
                              decoration: BoxDecoration(
                                border: Border(
                                  top: BorderSide(color: c.border),
                                ),
                              ),
                              child: Text(
                                l10n.bellFooter,
                                style: tx(c.fg3, size: 11),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _NoticeRow extends StatelessWidget {
  const _NoticeRow({super.key, required this.notice, required this.fresh});
  final Notice notice;

  /// New since the bell was last opened.
  final bool fresh;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final n = notice;
    final (title, text, first, second, onFirst, onSecond) = switch (n.kind) {
      NoticeKind.missedCall => (
        state.notices.missedCallTitle(n.person!),
        switch (state.sharedRoom(n.person!.id)) {
          final room? => l10n.bellBothInRoom(room),
          null => null,
        },
        state.callState.callText(n.person!.id, missed: true),
        l10n.bellDismiss,
        state.callState.activeCall == null &&
                state.sharedRoom(n.person!.id) == null
            ? () async {
                if (!await confirmCalling(context, n.person!.id)) return;
                state.notices.missedCalls.remove(n.person!.id);
                state.notices.dismissMissedNote();
                if (state.notices.bellFrom != null) state.toggleBell(null);
                await state.callFriend(n.person!.id);
              }
            : null,
        () => state.dismissNotice(n),
      ),
      NoticeKind.friendRequest => (
        l10n.bellFriendRequest(n.person!.name),
        l10n.bellCheckFingerprint(n.request!.fingerprint),
        l10n.commonAccept,
        l10n.commonDecline,
        () => state.answer(n.person!.id, true),
        () => state.answer(n.person!.id, false),
      ),
      NoticeKind.joinAsk => (
        l10n.bellJoinAsk(n.ask!.name, n.ask!.groupName),
        l10n.bellJoinAskNote(n.ask!.fingerprint),
        l10n.bellLetIn,
        l10n.bellSayNo,
        () => state.answerJoin(n.ask!, true),
        () => state.answerJoin(n.ask!, false),
      ),
      NoticeKind.groupInvite => (
        l10n.bellGroupInvite(n.invite!.by.name, n.invite!.name),
        l10n.bellMembers(n.invite!.members),
        l10n.bellJoin,
        l10n.commonNotNow,
        () => state.answerInvitation(n.invite!.id, true),
        () => state.answerInvitation(n.invite!.id, false),
      ),
      NoticeKind.directMessage => (
        l10n.bellNewMessages(n.chat!.unread, n.person!.name),
        n.chat!.last,
        l10n.bellOpen,
        l10n.bellMarkRead,
        () {
          if (state.notices.bellFrom != null) state.toggleBell(null);
          state.direct.show(n.person!.id);
        },
        () => state.dismissNotice(n),
      ),
      NoticeKind.firewall => (
        l10n.bellFirewallTitle,
        l10n.firewallNoticeText(
          firewallSystem,
          state.notices.firewallCommand ?? '',
        ),
        l10n.bellCopyCommand,
        l10n.bellDismiss,
        () {
          Clipboard.setData(
            ClipboardData(text: state.notices.firewallCommand ?? ''),
          );
          state.say(l10n.firewallCopied(firewallSystem));
        },
        () => state.dismissNotice(n),
      ),
      NoticeKind.otherSession => (
        l10n.bellOtherSessionTitle,
        state.notices.otherSessionText,
        null,
        l10n.bellDismiss,
        null,
        () => state.dismissNotice(n),
      ),
      NoticeKind.invitationDeclined => (
        n.declined!.said,
        null,
        null,
        l10n.bellDismiss,
        null,
        () => state.dismissNotice(n),
      ),
      NoticeKind.warning => (
        n.warning!.title,
        n.warning!.text,
        null,
        l10n.bellDismiss,
        null,
        () => state.dismissNotice(n),
      ),
    };
    final (badge, badgeColor, badgeBack) = switch (n.kind) {
      NoticeKind.missedCall => (Ic.missedCall, c.red, c.redSoft),
      NoticeKind.friendRequest => (Ic.userPlus, c.blue, c.blueSoft),
      NoticeKind.joinAsk => (Ic.userPlus, c.amber, c.amberSoft),
      NoticeKind.groupInvite => (Ic.users, c.green, c.greenSoft),
      NoticeKind.firewall => (Ic.shield, c.amber, c.amberSoft),
      NoticeKind.directMessage => (Ic.chat, c.blue, c.blueSoft),
      NoticeKind.otherSession => (Ic.warning, c.amber, c.amberSoft),
      NoticeKind.invitationDeclined => (Ic.userMinus, c.amber, c.amberSoft),
      NoticeKind.warning => (Ic.warning, c.amber, c.amberSoft),
    };
    final group = n.ask == null ? null : state.contacts.group(n.ask!.groupId);
    final Widget picture = switch (n.kind) {
      NoticeKind.missedCall ||
      NoticeKind.friendRequest ||
      NoticeKind.directMessage ||
      NoticeKind.invitationDeclined => Avatar(
        avatar: n.person!.avatar,
        size: 36,
      ),
      NoticeKind.joinAsk => GroupBadge(
        icon: group?.icon ?? 0,
        picture: group?.picture,
        size: 36,
        radius: 18,
        iconSize: 16,
      ),
      NoticeKind.groupInvite => Avatar(avatar: n.invite!.by.avatar, size: 36),
      NoticeKind.firewall => Container(
        width: 36,
        height: 36,
        alignment: Alignment.center,
        decoration: BoxDecoration(color: c.amberSoft, shape: BoxShape.circle),
        child: FgIcon(Ic.shield, size: 17, color: c.amber),
      ),
      NoticeKind.otherSession => Container(
        width: 36,
        height: 36,
        alignment: Alignment.center,
        decoration: BoxDecoration(color: c.amberSoft, shape: BoxShape.circle),
        child: FgIcon(Ic.screen, size: 17, color: c.amber),
      ),
      NoticeKind.warning => Container(
        width: 36,
        height: 36,
        alignment: Alignment.center,
        decoration: BoxDecoration(color: c.amberSoft, shape: BoxShape.circle),
        child: FgIcon(Ic.warning, size: 17, color: c.amber),
      ),
    };
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 8,
            child: fresh
                ? Padding(
                    padding: const EdgeInsets.only(top: 15),
                    child: Dot(c.blueSolid, size: 6),
                  )
                : null,
          ),
          SizedBox(
            width: 40,
            height: 40,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                picture,
                Positioned(
                  right: -1,
                  bottom: 0,
                  child: Container(
                    width: 18,
                    height: 18,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: Color.alphaBlend(badgeBack, c.bg2),
                      shape: BoxShape.circle,
                      border: Border.all(color: c.bg2, width: 2),
                    ),
                    child: FgIcon(badge, size: 9, color: badgeColor),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Text(
                        title,
                        style: tx(c.fg, size: 13, weight: w500, height: 1.35),
                      ),
                    ),
                    if (n.atMs > 0) ...[
                      const SizedBox(width: 8),
                      Text(
                        dayClock(n.atMs),
                        style: tx(c.fg3, size: 11, monospace: true),
                      ),
                    ],
                  ],
                ),
                if (text != null) ...[
                  const SizedBox(height: 2),
                  Text(text, style: tx(c.fg2, size: 12, height: 1.4)),
                ],
                const SizedBox(height: 8),
                // The second button goes under the first when the two do not fit.
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: [
                    FgButton(
                      text: second,
                      height: 28,
                      fontSize: 12,
                      onTap: onSecond,
                    ),
                    if (first != null)
                      FgButton(
                        text: first,
                        kind: Kind.primary,
                        height: 28,
                        fontSize: 12,
                        onTap: onFirst,
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
