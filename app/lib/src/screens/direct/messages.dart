part of '../direct.dart';

// The chat's messages: its rows by day and writer, the lines about the timer and pins,
// and each message with its actions and how far it got.

/// One row of the chat: a day, a line about the timer or a pin, or a message with or
/// without its writer's name above.
sealed class _Row {}

class _DayRow extends _Row {
  _DayRow(this.label);
  final String label;
}

class _TimerRow extends _Row {
  _TimerRow(this.text);
  final String text;
}

class _CallRow extends _Row {
  _CallRow(this.id, this.text);

  /// The line's own item, to tell one call from another.
  final String id;
  final String text;
}

class _PinRow extends _Row {
  _PinRow(this.id, this.target, this.text);

  /// The line's own item: the same message can be pinned again later.
  final String id;

  /// The message pinned.
  final String target;
  final String text;
}

class _MessageRow extends _Row {
  _MessageRow(this.item, {required this.head, required this.last});
  final UiDirectItem item;

  /// The writer's picture and name go above it.
  final bool head;

  /// This device's last message: whether it reached the friend shows under it.
  final bool last;
}

List<_Row> _rows(UiDirectPage page) {
  final out = <_Row>[];
  UiDirectItem? before;
  String? lastMine;
  for (final item in page.items) {
    if (item.mine &&
        item.timerMs == null &&
        item.pin == null &&
        item.call == null) {
      lastMine = item.id;
    }
  }
  var day = (0, 0);
  for (final (index, item) in page.items.indexed) {
    final pin = item.pin;
    // An unpin says nothing in the chat.
    if (pin != null && !pin.pinned) continue;
    final newDay = before == null || item.atMs < day.$1 || item.atMs >= day.$2;
    if (newDay) {
      day = _dayOf(item.atMs);
      out.add(_DayRow(dayLabel(item.atMs)));
    }
    if (item.timerMs != null) {
      out.add(
        _TimerRow(
          timerLine(item, page.friend.name, first: index == 0 && !page.more),
        ),
      );
    } else if (pin != null) {
      out.add(_PinRow(item.id, pin.target, pinLine(item, page.friend.name)));
    } else if (item.call != null) {
      out.add(_CallRow(item.id, callLine(item, page.friend.name)));
    } else {
      final prev = before;
      final head =
          prev == null ||
          newDay ||
          prev.timerMs != null ||
          prev.pin != null ||
          prev.call != null ||
          prev.mine != item.mine ||
          item.atMs - prev.atMs > _together.inMilliseconds;
      out.add(_MessageRow(item, head: head, last: item.id == lastMine));
    }
    before = item;
  }
  return out;
}

class _Messages extends StatefulWidget {
  const _Messages({required this.page});
  final UiDirectPage page;

  @override
  State<_Messages> createState() => _MessagesState();
}

class _MessagesState extends State<_Messages> {
  final _scroll = ScrollController();

  /// The messages' keys, to know which are built and bring one into view.
  final _keys = <String, GlobalKey>{};
  ValueNotifier<(String, int)?>? _jump;

  /// The message just brought into view, shown lit for a moment.
  String? _lit;
  Timer? _unlight;

  /// The rows of the page last shown: made again only when the page is read again or
  /// the day changes (Today, Yesterday), not on each build.
  UiDirectPage? _rowsOf;
  int _rowsDay = 0;
  List<_Row> _rowsMade = const [];

  List<_Row> _rowsFor(UiDirectPage page) {
    final now = DateTime.now();
    final day = now.year * 10000 + now.month * 100 + now.day;
    if (!identical(page, _rowsOf) || day != _rowsDay) {
      _rowsOf = page;
      _rowsDay = day;
      _rowsMade = _rows(page);
    }
    return _rowsMade;
  }

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_onScroll);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final jump = AppScope.read(context).direct.jump;
    if (!identical(jump, _jump)) {
      _jump?.removeListener(_onJump);
      _jump = jump..addListener(_onJump);
    }
  }

  @override
  void dispose() {
    _jump?.removeListener(_onJump);
    _unlight?.cancel();
    _scroll.dispose();
    super.dispose();
  }

  void _onJump() {
    final asked = _jump?.value;
    if (asked != null) _bringIntoView(asked.$1);
  }

  /// Scrolls until message `id` is built, then brings it to the middle and lights it.
  Future<void> _bringIntoView(String id) async {
    // The page may have just grown with older items.
    await WidgetsBinding.instance.endOfFrame;
    for (var tries = 0; tries < 200 && mounted; tries++) {
      final built = _keys[id]?.currentContext;
      if (built != null && built.mounted) {
        setState(() => _lit = id);
        _unlight?.cancel();
        _unlight = Timer(const Duration(milliseconds: 1600), () {
          if (mounted) setState(() => _lit = null);
        });
        await Scrollable.ensureVisible(
          built,
          alignment: .5,
          duration: const Duration(milliseconds: 380),
          curve: Curves.easeOutCubic,
        );
        return;
      }
      if (!_scroll.hasClients) return;
      // Rows count from the bottom, where the list starts.
      final rows = _rowsFor(widget.page);
      int place(int row) => rows.length - 1 - row;
      final row = rows.indexWhere((r) => r is _MessageRow && r.item.id == id);
      if (row < 0) return;
      final shown = [
        for (final (i, r) in rows.indexed)
          if (r is _MessageRow && _keys[r.item.id]?.currentContext != null)
            place(i),
      ];
      final position = _scroll.position;
      final step = position.viewportDimension * .8;
      final next = shown.isEmpty
          ? position.maxScrollExtent * place(row) / math.max(1, rows.length - 1)
          : shown.reduce(math.max) < place(row)
          ? position.pixels + step
          : position.pixels - step;
      final to = next.clamp(position.minScrollExtent, position.maxScrollExtent);
      if (to == position.pixels && shown.isNotEmpty) return;
      _scroll.jumpTo(to);
      await WidgetsBinding.instance.endOfFrame;
    }
  }

  /// Near the oldest message shown, older ones are read.
  void _onScroll() {
    if (!_scroll.hasClients) return;
    if (_scroll.position.extentAfter < 200 && widget.page.more) {
      AppScope.read(context).direct.more();
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final page = widget.page;
    final me = state.contacts.profile?.person;
    final rows = _rowsFor(page);
    final ids = {for (final i in page.items) i.id};
    _keys.removeWhere((id, _) => !ids.contains(id));
    if (rows.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Avatar(avatar: page.friend.avatar, size: 56, id: page.friend.id),
              const SizedBox(height: 14),
              Text(page.friend.name, style: tx(c.fg, size: 18, weight: w600)),
              const SizedBox(height: 4),
              Text(
                l10n.directEmptyNote,
                textAlign: TextAlign.center,
                style: tx(c.fg2, size: 13),
              ),
            ],
          ),
        ),
      );
    }
    // Newest at the bottom, where the list starts; what is older is above.
    return SelectionArea(
      child: Scrollbar(
        controller: _scroll,
        child: ListView.builder(
          controller: _scroll,
          reverse: true,
          padding: const EdgeInsets.fromLTRB(24, 18, 24, 8),
          itemCount: rows.length,
          itemBuilder: (context, i) {
            final row = rows[rows.length - 1 - i];
            return switch (row) {
              _DayRow(:final label) => _Day(label: label),
              _TimerRow(:final text) => _TimerLine(text: text),
              _CallRow(:final id, :final text) => _TimerLine(
                key: ValueKey('direct-call-line:$id'),
                text: text,
                icon: Ic.phone,
              ),
              _PinRow(:final id, :final target, :final text) => _PinLine(
                key: ValueKey('direct-pin-line:$id'),
                text: text,
                onTap: () => state.direct.showMessage(target),
              ),
              _MessageRow() => KeyedSubtree(
                key: _keys.putIfAbsent(row.item.id, GlobalKey.new),
                child: _Message(
                  key: ValueKey('direct-item:${row.item.id}'),
                  row: row,
                  page: page,
                  writer: row.item.mine && me != null ? me : page.friend,
                  lit: _lit == row.item.id,
                ),
              ),
            };
          },
        ),
      ),
    );
  }
}

class _Day extends StatelessWidget {
  const _Day({required this.label});
  final String label;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Padding(
      padding: const EdgeInsets.fromLTRB(0, 10, 0, 6),
      child: Row(
        children: [
          Expanded(child: Container(height: 1, color: c.border)),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Text(label, style: tx(c.fg3, size: 12)),
          ),
          Expanded(child: Container(height: 1, color: c.border)),
        ],
      ),
    );
  }
}

/// A line of the chat about the chat itself: a change of the timer, or a call that ended.
class _TimerLine extends StatelessWidget {
  const _TimerLine({super.key, required this.text, this.icon = Ic.timer});
  final String text;
  final Ic icon;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Center(
        child: TweenAnimationBuilder<double>(
          tween: Tween(begin: 0, end: 1),
          duration: const Duration(milliseconds: 250),
          curve: Curves.easeOut,
          builder: (context, t, child) => Opacity(
            opacity: t,
            child: Transform.translate(
              offset: Offset(0, 4 * (1 - t)),
              child: child,
            ),
          ),
          child: Container(
            height: 28,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(999),
              border: Border.all(color: c.border),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                FgIcon(icon, size: 13, color: c.fg2),
                const SizedBox(width: 7),
                Flexible(
                  child: Text(
                    text,
                    overflow: TextOverflow.ellipsis,
                    style: tx(c.fg2, size: 12, height: 1.2),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A pin, as a line in the chat: a click brings the message into view.
class _PinLine extends StatelessWidget {
  const _PinLine({super.key, required this.text, required this.onTap});
  final String text;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Center(
        child: TweenAnimationBuilder<double>(
          tween: Tween(begin: 0, end: 1),
          duration: const Duration(milliseconds: 250),
          curve: Curves.easeOut,
          builder: (context, t, child) => Opacity(
            opacity: t,
            child: Transform.translate(
              offset: Offset(0, 4 * (1 - t)),
              child: child,
            ),
          ),
          child: Springy(
            onTap: onTap,
            label: l10n.directPinLineLabel(text),
            tooltip: l10n.directShowMessage,
            hoverScale: 1.03,
            pressScale: .97,
            builder: (context, hovered) => AnimatedContainer(
              duration: const Duration(milliseconds: 160),
              height: 28,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              decoration: BoxDecoration(
                color: hovered ? c.hover : c.bg2,
                borderRadius: BorderRadius.circular(999),
                border: Border.all(color: c.border),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  FgIcon(Ic.pin, size: 13, color: c.fg2),
                  const SizedBox(width: 7),
                  Flexible(
                    child: Text(
                      text,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg2, size: 12, height: 1.2),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Message extends StatelessWidget {
  const _Message({
    super.key,
    required this.row,
    required this.page,
    required this.writer,
    this.lit = false,
  });
  final _MessageRow row;
  final UiDirectPage page;
  final UiPerson writer;

  /// Just brought into view from the pinned messages.
  final bool lit;

  /// Asks, then deletes the message: from this computer, the friend keeping its copy, or
  /// for both, one of this device's own in a chat with a friend.
  Future<void> _delete(
    BuildContext context,
    AppState state,
    UiDirectPage page,
  ) async {
    final id = row.item.id;
    if (!row.item.mine || page.endedMs != null) {
      final yes = await confirmDialog(
        context,
        title: l10n.directDeleteMessageTitle,
        text: l10n.directDeleteMessageNote(page.friend.name),
        action: l10n.directDeleteMessageAction,
      );
      if (yes) await state.direct.deleteMessage(id);
      return;
    }
    final c = context.fg;
    final both = await showFgDialog<bool>(
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
                  l10n.directDeleteMessageTitle,
                  style: tx(
                    c.fg,
                    size: 20,
                    weight: w600,
                    height: 1.3,
                    spacing: -.4,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  l10n.directDeleteOwnNote(page.friend.name),
                  style: tx(c.fg2),
                ),
              ],
            ),
          ),
          Footer(
            children: [
              // Three buttons may not fit in one row in every language: they wrap.
              Expanded(
                child: Wrap(
                  alignment: WrapAlignment.end,
                  spacing: 10,
                  runSpacing: 10,
                  children: [
                    FgButton(
                      text: l10n.commonCancel,
                      onTap: () => Navigator.of(dialog).pop(),
                    ),
                    FgButton(
                      key: const ValueKey('direct-delete-for-me'),
                      text: l10n.directDeleteMessage,
                      onTap: () => Navigator.of(dialog).pop(false),
                    ),
                    FgButton(
                      key: const ValueKey('direct-delete-for-both'),
                      text: l10n.directDeleteForBoth,
                      kind: Kind.danger,
                      onTap: () => Navigator.of(dialog).pop(true),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
    );
    if (both == null) return;
    await (both
        ? state.direct.unsendMessage(id)
        : state.direct.deleteMessage(id));
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final item = row.item;
    final friend = !item.mine;
    // The friend's picture and name open the friend's card, as in Friends; this
    // device's own open its own panel beside them.
    Widget toCard(Widget child, String label) => !friend
        ? Builder(
            builder: (context) => Pressable(
              onTap: () => toggleUserPanel(context, beside: true),
              label: l10n.memberCardShowOwn,
              builder: (_, _) => child,
            ),
          )
        : state.contacts.friends.any((f) => f.person.id == writer.id)
        ? Builder(
            builder: (context) => Pressable(
              onTap: () => toggleFriendCard(context, writer.id),
              label: label,
              builder: (_, _) => child,
            ),
          )
        : child;
    final message = Padding(
      padding: EdgeInsets.only(top: row.head ? 14 : 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 32,
            child: row.head
                ? toCard(
                    Avatar(avatar: writer.avatar, size: 32),
                    l10n.directShowCard(writer.name),
                  )
                : null,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (row.head)
                  Row(
                    children: [
                      Flexible(
                        child: toCard(
                          Text(
                            writer.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tx(
                              c.fg,
                              size: 13,
                              weight: w600,
                              height: 1.3,
                            ),
                          ),
                          l10n.directShowCard(writer.name),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        clock(item.atMs),
                        style: tx(c.fg3, size: 11, height: 1.3),
                      ),
                      if (item.pinned) ...[
                        const SizedBox(width: 8),
                        const _Pinned(),
                      ],
                    ],
                  )
                else if (item.pinned)
                  const _Pinned(),
                if (item.text.isNotEmpty) ...[
                  if (row.head) const SizedBox(height: 4),
                  LongText(
                    emojiText(shownText(item.text), tx(c.fg)),
                    key: ValueKey(('text', item.id)),
                  ),
                ],
                for (final f in item.files) ...[
                  const SizedBox(height: 4),
                  _FileCard(file: f, item: item, page: page, writer: writer),
                ],
                if (row.last) ...[
                  const SizedBox(height: 4),
                  _Delivery(item: item, page: page),
                ],
              ],
            ),
          ),
        ],
      ),
    );
    final canPin = page.endedMs == null && writeBlocked(state, page) == null;
    final full = !item.pinned && page.pinned.length >= maxPins;
    return _Hovered(
      builder: (hovered) => Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned(
            left: -10,
            right: -10,
            top: row.head ? 8 : 0,
            bottom: -2,
            child: AnimatedContainer(
              duration: Duration(milliseconds: lit ? 120 : 600),
              decoration: BoxDecoration(
                color: lit
                    ? c.hover
                    : hovered
                    ? c.hover.withValues(alpha: .45)
                    : c.hover.withValues(alpha: 0),
                borderRadius: BorderRadius.circular(12),
              ),
            ),
          ),
          // With a timer, hovering tells when it goes from here (the time left is read
          // again each time the pointer comes over it), or that it stays while pinned.
          if (item.expiresMs == 0)
            message
          else
            Tooltip(
              message: item.pinned
                  ? l10n.directPinnedStays
                  : disappearsIn(item.expiresMs),
              child: message,
            ),
          // In the middle of the message's right edge, however many lines it has.
          Positioned(
            top: row.head ? 8 : 0,
            bottom: -2,
            right: 4,
            child: Center(
              child: _Reveal(
                hovered: hovered,
                duration: const Duration(milliseconds: 160),
                child: _MessageActions(
                  pinned: item.pinned,
                  full: full,
                  onPin: canPin
                      ? () => state.direct.pin(item.id, pinned: !item.pinned)
                      : null,
                  onDelete: () => _delete(context, state, page),
                  id: item.id,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Beside the time of a pinned message.
class _Pinned extends StatelessWidget {
  const _Pinned();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        FgIcon(Ic.pin, size: 11, color: c.fg3),
        const SizedBox(width: 4),
        Text(l10n.directPinned, style: tx(c.fg3, size: 11, height: 1.3)),
      ],
    );
  }
}

/// What can be done with a message, over its top right corner while hovered.
/// Controls that show while what they belong to is hovered, and while one of them has
/// the keyboard's focus: Tab never lands on something that cannot be seen.
class _Reveal extends StatefulWidget {
  const _Reveal({
    required this.hovered,
    required this.duration,
    required this.child,
    this.tappableHidden = false,
  });
  final bool hovered;
  final Duration duration;
  final Widget child;

  /// A press reaches it even while it does not show, as on a touch screen, where
  /// nothing is hovered.
  final bool tappableHidden;

  @override
  State<_Reveal> createState() => _RevealState();
}

class _RevealState extends State<_Reveal> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final shown = widget.hovered || _focused;
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: IgnorePointer(
        ignoring: !shown && !widget.tappableHidden,
        child: AnimatedOpacity(
          opacity: shown ? 1 : 0,
          duration: widget.duration,
          child: widget.child,
        ),
      ),
    );
  }
}

class _MessageActions extends StatelessWidget {
  const _MessageActions({
    required this.id,
    required this.pinned,
    required this.full,
    required this.onPin,
    required this.onDelete,
  });
  final String id;
  final bool pinned;

  /// As many messages as a chat keeps pinned are pinned already.
  final bool full;

  /// Null where nothing may be pinned (a chat that can only be read).
  final VoidCallback? onPin;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    Widget button({
      required String key,
      required VoidCallback? onTap,
      required String label,
      required String tooltip,
      required Ic icon,
      bool danger = false,
    }) => Springy(
      key: ValueKey(key),
      onTap: onTap,
      label: label,
      tooltip: tooltip,
      hoverScale: 1.1,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 140),
        width: 24,
        height: 24,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: hovered && onTap != null ? c.hover : null,
        ),
        child: FgIcon(
          icon,
          size: 14,
          color: onTap == null
              ? c.fg3
              : hovered
              ? (danger ? c.red : c.fg)
              : c.fg2,
        ),
      ),
    );
    final onPin = this.onPin;
    return Container(
      height: 30,
      padding: const EdgeInsets.all(2),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (onPin != null)
            button(
              key: 'direct-pin:$id',
              onTap: full ? null : onPin,
              label: pinned ? l10n.directUnpin : l10n.directPin,
              tooltip: full
                  ? l10n.directPinsFull(maxPins)
                  : pinned
                  ? l10n.directUnpin
                  : l10n.directPin,
              icon: pinned ? Ic.unpin : Ic.pin,
            ),
          button(
            key: 'direct-delete:$id',
            onTap: onDelete,
            label: l10n.directDeleteMessageAction,
            tooltip: l10n.directDeleteMessageAction,
            icon: Ic.trash,
            danger: true,
          ),
        ],
      ),
    );
  }
}

/// Under this device's last message: whether it reached the friend.
class _Delivery extends StatelessWidget {
  const _Delivery({required this.item, required this.page});
  final UiDirectItem item;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final (text, color) = item.delivered
        ? (l10n.directDelivered, c.fg3)
        : page.endedMs != null
        ? (l10n.directNotDelivered, c.fg3)
        : page.connected && page.online
        ? (l10n.directSending, c.fg3)
        : (l10n.directWaiting(page.friend.name), c.amber);
    return Row(
      children: [
        FgIcon(Ic.clock, size: 12, color: color),
        const SizedBox(width: 6),
        Flexible(child: Text(text, style: tx(color, size: 11, height: 1.3))),
      ],
    );
  }
}
