part of '../direct.dart';

// The top of a chat: who it is with and how they are, the pinned messages, the
// disappearing timer and its menu, and the call button.

class _Header extends StatelessWidget {
  const _Header({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final friend = page.friend;
    final online = page.online && page.endedMs == null;
    final canCall = online && state.callState.activeCall == null;
    final who = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Avatar(avatar: friend.avatar, size: 28, dim: !online),
        const SizedBox(width: 10),
        Flexible(
          child: Text(
            friend.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg, weight: w600),
          ),
        ),
      ],
    );
    return Container(
      height: 56,
      padding: const EdgeInsets.fromLTRB(20, 0, 16, 0),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: c.border)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Row(
              children: [
                // The picture and name open the friend's card, as in Friends.
                Flexible(
                  child:
                      state.contacts.friends.any(
                        (f) => f.person.id == friend.id,
                      )
                      ? Builder(
                          builder: (context) => Pressable(
                            key: const ValueKey('direct-friend'),
                            onTap: () => toggleFriendCard(context, friend.id),
                            label: l10n.directShowCard(friend.name),
                            focusRadius: 10,
                            builder: (_, _) => who,
                          ),
                        )
                      : who,
                ),
                const SizedBox(width: 10),
                Flexible(
                  child: Text(
                    page.endedMs != null
                        ? l10n.directNoLongerFriend
                        : chatStatus(page),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tx(
                      !online
                          ? c.fg3
                          : page.away && page.endedMs == null
                          ? c.amber
                          : c.green,
                      size: 13,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 10),
          _PinsPill(page: page),
          const SizedBox(width: 10),
          if (page.endedMs == null) ...[
            _TimerPill(page: page),
            const SizedBox(width: 10),
          ],
          FgButton(
            key: const ValueKey('direct-call'),
            text: state.callState.callText(friend.id),
            icon: Ic.phone,
            kind: canCall ? Kind.primary : Kind.secondary,
            height: 32,
            fontSize: 13,
            padding: const EdgeInsets.symmetric(horizontal: 14),
            tooltip: !online
                ? l10n.directCallOffline(friend.name)
                : state.callState.activeCall != null
                ? l10n.directHangUpFirst(friend.name)
                : null,
            onTap: canCall ? () => callFriendChecked(context, friend.id) : null,
          ),
        ],
      ),
    );
  }
}

/// The messages pinned in the chat: a pill with how many, that opens their list.
class _PinsPill extends StatelessWidget {
  const _PinsPill({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final count = page.pinned.length;
    return Springy(
      key: const ValueKey('direct-pins'),
      onTap: () => _showUnder(context, (_) => const _PinsMenu()),
      label: l10n.directPinnedCount(count),
      tooltip: l10n.directPinnedTitle,
      hoverScale: 1.05,
      pressScale: .95,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        height: 32,
        constraints: const BoxConstraints(minWidth: 32),
        padding: EdgeInsets.symmetric(horizontal: count == 0 ? 0 : 11),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? c.hover : c.bg2,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(color: c.border),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(Ic.pin, size: 14, color: count == 0 ? c.fg2 : c.fg),
            if (count > 0) ...[
              const SizedBox(width: 6),
              Text('$count', style: tx(c.fg, size: 13, height: 1.2)),
            ],
          ],
        ),
      ),
    );
  }
}

/// The pinned messages under their pill, the most recently pinned first: a click brings
/// one into view, and each can be unpinned from here.
class _PinsMenu extends StatelessWidget {
  const _PinsMenu();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final page = state.direct.page;
    final pinned = page?.pinned ?? const <UiDirectItem>[];
    final me = state.contacts.profile?.person;
    return Material(
      type: MaterialType.transparency,
      child: Container(
        width: 380,
        constraints: BoxConstraints(
          maxHeight: math.min(460, MediaQuery.sizeOf(context).height - 120),
        ),
        padding: const EdgeInsets.all(6),
        decoration: BoxDecoration(
          color: c.bg2,
          borderRadius: BorderRadius.circular(18),
          border: Border.all(color: c.border),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 8, 10, 6),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(l10n.directPinnedTitle, style: tx(c.fg, weight: w600)),
                  Text(
                    l10n.directPinnedNote(maxPins),
                    style: tx(c.fg2, size: 12),
                  ),
                ],
              ),
            ),
            if (page == null || pinned.isEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(10, 14, 10, 16),
                child: Column(
                  children: [
                    FgIcon(Ic.pin, size: 22, color: c.fg3),
                    const SizedBox(height: 8),
                    Text(
                      l10n.directNothingPinned,
                      style: tx(c.fg2, size: 13, weight: w500),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      l10n.directNothingPinnedNote,
                      textAlign: TextAlign.center,
                      style: tx(c.fg3, size: 12),
                    ),
                  ],
                ),
              )
            else
              Flexible(
                child: ListView(
                  shrinkWrap: true,
                  padding: EdgeInsets.zero,
                  children: [
                    for (final item in pinned)
                      _PinnedRow(
                        key: ValueKey('pinned:${item.id}'),
                        item: item,
                        writer: item.mine && me != null ? me : page.friend,
                        canUnpin:
                            page.endedMs == null &&
                            writeBlocked(state, page) == null,
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

/// `Oct 21, 14:05`, `Today, 14:05`.
String _when(int ms) => l10n.directDayTime(dayLabel(ms), clock(ms));

/// What a message says in one place: its text, or its files.
String _gist(UiDirectItem item) {
  final text = item.text.trim();
  if (text.isNotEmpty) return shownText(text);
  final files = item.files;
  if (files.length == 1) return files.first.name;
  return l10n.directFilesCount(files.length);
}

class _PinnedRow extends StatelessWidget {
  const _PinnedRow({
    super.key,
    required this.item,
    required this.writer,
    required this.canUnpin,
  });
  final UiDirectItem item;
  final UiPerson writer;
  final bool canUnpin;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final words = item.text.trim().isNotEmpty;
    return Pressable(
      onTap: () {
        Navigator.of(context).pop();
        state.direct.showMessage(item.id);
      },
      label: l10n.directShowMessageFrom(writer.name),
      focusRadius: 12,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 140),
        padding: const EdgeInsets.fromLTRB(10, 9, 6, 9),
        decoration: BoxDecoration(
          color: hovered ? c.hover : null,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Avatar(avatar: writer.avatar, size: 28),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(
                          text: writer.name,
                          style: tx(c.fg, size: 13, weight: w600, height: 1.3),
                        ),
                        const TextSpan(text: '  '),
                        TextSpan(
                          text: _when(item.atMs),
                          style: tx(c.fg3, size: 11, height: 1.3),
                        ),
                      ],
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Row(
                    children: [
                      if (!words) ...[
                        FgIcon(Ic.paperclip, size: 12, color: c.fg2),
                        const SizedBox(width: 5),
                      ],
                      Flexible(
                        child: Text(
                          _gist(item),
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          style: tx(words ? c.fg : c.fg2, size: 13),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            if (canUnpin) ...[
              const SizedBox(width: 6),
              _Reveal(
                hovered: hovered,
                duration: const Duration(milliseconds: 140),
                tappableHidden: true,
                child: FgButton(
                  key: ValueKey('unpin:${item.id}'),
                  icon: Ic.close,
                  kind: Kind.ghost,
                  height: 26,
                  tooltip: l10n.directUnpin,
                  onTap: () => state.direct.pin(item.id, pinned: false),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// The chat's disappearing timer, as a pill that opens the choices.
class _TimerPill extends StatelessWidget {
  const _TimerPill({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final on = page.timerMs != 0;
    final color = on ? c.blue : c.fg2;
    final blocked = writeBlocked(AppScope.only(context, directParts), page);
    return Springy(
      key: const ValueKey('direct-timer'),
      onTap: blocked == null ? () => _showTimerMenu(context, page) : null,
      label: l10n.directTimerLabel(timerWord(page.timerMs)),
      tooltip: blocked ?? l10n.directTimerTitle,
      hoverScale: 1.05,
      pressScale: .95,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        height: 32,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: on ? c.blueSoft : c.bg2,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(color: c.border),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(Ic.timer, size: 14, color: color),
            const SizedBox(width: 7),
            Text(
              timerWord(page.timerMs),
              style: tx(color, size: 13, height: 1.2),
            ),
          ],
        ),
      ),
    );
  }
}

Future<void> _showTimerMenu(BuildContext context, UiDirectPage page) {
  final state = AppScope.read(context);
  return _showUnder(
    context,
    (context) => _TimerMenu(
      current: page.timerMs,
      onPick: (ms) {
        Navigator.of(context).pop();
        if (ms != page.timerMs) state.direct.setTimer(ms);
      },
    ),
  );
}

/// Opens what `builder` makes under the pill of `context`, its right edge with the pill's.
Future<void> _showUnder(
  BuildContext context,
  Widget Function(BuildContext context) builder,
) {
  final box = context.findRenderObject() as RenderBox;
  final at = box.localToGlobal(Offset.zero) & box.size;
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: l10n.directCloseMenu,
    barrierColor: Colors.transparent,
    transitionDuration: springTime,
    pageBuilder: (context, _, _) => CustomSingleChildLayout(
      delegate: _UnderPill(at),
      child: builder(context),
    ),
    transitionBuilder: (context, animation, _, child) {
      final t = CurvedAnimation(parent: animation, curve: spring);
      return FadeTransition(
        opacity: animation,
        child: AnimatedBuilder(
          animation: t,
          builder: (context, child) => Transform.translate(
            offset: Offset(0, -6 * (1 - t.value)),
            child: child,
          ),
          child: ScaleTransition(
            scale: Tween(begin: .95, end: 1.0).animate(t),
            alignment: Alignment.topRight,
            child: child,
          ),
        ),
      );
    },
  );
}

/// Puts the menu under the pill, its right edge with the pill's, inside the window.
class _UnderPill extends SingleChildLayoutDelegate {
  _UnderPill(this.pill);
  final Rect pill;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size child) {
    final left = math.min(
      pill.right - child.width,
      size.width - child.width - 8,
    );
    return Offset(math.max(8, left), pill.bottom + 8);
  }

  @override
  bool shouldRelayout(_UnderPill old) => old.pill != pill;
}

class _TimerMenu extends StatelessWidget {
  const _TimerMenu({required this.current, required this.onPick});
  final int current;
  final ValueChanged<int> onPick;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Material(
      type: MaterialType.transparency,
      child: Container(
        width: 300,
        padding: const EdgeInsets.all(6),
        decoration: BoxDecoration(
          color: c.bg2,
          borderRadius: BorderRadius.circular(18),
          border: Border.all(color: c.border),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 8, 10, 6),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(l10n.directTimerTitle, style: tx(c.fg, weight: w600)),
                  Text(l10n.directTimerNote, style: tx(c.fg2, size: 12)),
                ],
              ),
            ),
            for (final ms in timers)
              Pressable(
                key: ValueKey('timer:$ms'),
                onTap: () => onPick(ms),
                label: timerWord(ms),
                focusRadius: 10,
                builder: (context, hovered) => AnimatedContainer(
                  duration: const Duration(milliseconds: 140),
                  height: 36,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  decoration: BoxDecoration(
                    color: hovered ? c.hover : null,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Row(
                    children: [
                      Expanded(child: Text(timerWord(ms), style: tx(c.fg))),
                      if (ms == current)
                        FgIcon(Ic.check, size: 15, stroke: 2.4, color: c.green),
                    ],
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
