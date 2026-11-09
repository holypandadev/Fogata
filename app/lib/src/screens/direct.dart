import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fogata_video/fogata_video.dart';
import 'package:intl/intl.dart';

import '../app_state.dart';
import '../direct_state.dart';
import '../emoji.dart' show emojiText;
import '../format.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/direct.dart';
import '../rust/api/types.dart';
import '../moving.dart' show ChatPicture;
import '../photo.dart' show readAtMost;
import '../safe_image.dart';
import '../shown_text.dart';
import '../theme.dart';
import '../typing.dart';
import '../video_sound.dart';
import '../voice.dart';
import '../widgets.dart';
import 'emoji_picker.dart';
import 'friend_card.dart' show toggleFriendCard;
import 'user_panel.dart' show toggleUserPanel;
import 'members.dart' show confirmDialog;
import 'room.dart' show callFriendChecked;
import 'user_panel.dart' show CardBio, PanelBanner, StatusDot;
import 'voice_ui.dart';

/// The colour of files in what a chat takes: none of the theme's own.
const _violet = Color(0xFFA78BFA);

/// Messages of the same person closer than this go under one name.
const _together = Duration(minutes: 5);

/// `0.4 MB`, `84 MB`, `1.2 GB`: what a chat takes, as the storage pages say it.
String storageSize(int n) {
  String number(num v, {bool tenths = false}) =>
      NumberFormat(tenths ? '0.0' : '0', intlLocale).format(v);
  if (n <= 0) return l10n.sizeMB(number(0));
  if (n < 100000) return l10n.sizeKB(number((n / 1000).ceil()));
  if (n < 1000000) return l10n.sizeMB(number(n / 1000000, tenths: true));
  if (n < 1000000000) return l10n.sizeMB(number((n / 1000000).round()));
  return l10n.sizeGB(number(n / 1000000000, tenths: true));
}

/// `Today`, `Yesterday`, `Monday`, `Oct 21`, `Oct 21 2025`.
String dayLabel(int ms, {DateTime? now}) {
  final t = DateTime.fromMillisecondsSinceEpoch(ms);
  final today = now ?? DateTime.now();
  final days =
      (DateTime(
                today.year,
                today.month,
                today.day,
              ).difference(DateTime(t.year, t.month, t.day)).inHours /
              24)
          .round();
  if (days == 0) return l10n.directToday;
  if (days == 1) return l10n.directYesterday;
  if (days > 1 && days < 7) {
    // A day of the week stands alone here: its standalone form, with a capital letter in
    // every language.
    return capitalized(DateFormat('cccc', intlLocale).format(t));
  }
  return day(t, withYear: t.year != today.year);
}

/// The local day `ms` falls in, as the times its first and next day begin. Reading a
/// local time is slow (a few microseconds), so a long chat finds its days this way, once
/// per day rather than twice per message.
(int, int) _dayOf(int ms) {
  final t = DateTime.fromMillisecondsSinceEpoch(ms);
  return (
    DateTime(t.year, t.month, t.day).millisecondsSinceEpoch,
    DateTime(t.year, t.month, t.day + 1).millisecondsSinceEpoch,
  );
}

/// What a change of the timer says in the chat.
String timerLine(UiDirectItem item, String name, {required bool first}) {
  final ms = item.timerMs ?? 0;
  final mine = item.mine ? 'yes' : 'no';
  if (ms == 0) return l10n.directTimerOff(name, mine);
  // A new chat starts with the timer its writer chose for new chats.
  if (first) return l10n.directTimerFirst(timerWord(ms));
  return l10n.directTimerOn(name, mine, timerWord(ms));
}

/// What a pin says in the chat.
String pinLine(UiDirectItem item, String name) =>
    l10n.directPinnedLine(name, item.mine ? 'yes' : 'no');

/// How long a call lasted, in words: `a few seconds`, `1 minute`, `25 minutes`, `1 hour`,
/// `2 hours 5 minutes`.
String callLength(int ms) {
  final minutes = ms ~/ 60000;
  if (minutes <= 0) return l10n.directCallFewSeconds;
  final (hours, rest) = (minutes ~/ 60, minutes % 60);
  if (hours == 0) return l10n.directCallMinutes(rest);
  if (rest == 0) return l10n.directCallHours(hours);
  return l10n.directCallHoursMinutes(hours, rest);
}

/// What a call that ended says in the chat with `name`: who started it and how long it
/// lasted.
String callLine(UiDirectItem item, String name) {
  final call = item.call!;
  final starter = call.starter;
  return l10n.directCallLine(
    starter ?? name,
    starter == null && item.mine ? 'yes' : 'no',
    callLength(call.lastedMs),
  );
}

/// Why nothing can be written in a chat now, or null when it can: both people have to be
/// online, so what is written reaches the friend at once. What was read stays.
String? writeBlocked(AppState state, UiDirectPage page) {
  if (page.endedMs != null) return null;
  if (state.prefs.appearOffline) {
    return l10n.directBlockedOffline;
  }
  if (!page.online) {
    return l10n.directBlockedFriendOffline(page.friend.name);
  }
  return null;
}

/// `Online`, `Away`, or `Offline · seen 2 days ago`.
String chatStatus(UiDirectPage page) {
  if (page.online) return page.away ? l10n.directAway : l10n.directOnline;
  final seen = page.lastSeenMs;
  return seen == null ? l10n.directOffline : l10n.directOfflineSeen(ago(seen));
}

// ----- The list in the side column -----

/// The parts of the state the direct chats show: the chats, the friends, the calls (for
/// the Call button) and the person's settings. Rooms, toasts and the bell do not build
/// them again.
const directParts = {DirectChats, Contacts, CallState, Prefs};

/// The direct messages in the side column under Home and Friends: each chat with its
/// friend's picture and presence, the last line and how many messages are unread.
class DirectList extends StatelessWidget {
  const DirectList({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, {...directParts, Navigation});
    final c = context.fg;
    final direct = state.direct;
    final chats = direct.chats;
    final shown = state.nav.page == Screen.direct ? direct.open : null;
    // The chat shown is listed while it is new and still empty.
    final opened = shown != null && direct.chat(shown) == null
        ? direct.page
        : null;
    if (chats.isEmpty && opened == null) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(10, 14, 10, 4),
          child: Text(
            l10n.directHeading,
            style: tx(c.fg3, size: 11, weight: w500, spacing: .4),
          ),
        ),
        if (opened != null)
          _ChatRow(
            friend: opened.friend,
            online: opened.online,
            away: opened.away,
            preview: null,
            unread: 0,
            selected: true,
          ),
        for (final chat in chats)
          _ChatRow(
            friend: chat.friend,
            online: chat.online && chat.endedMs == null,
            away: chat.away,
            preview: switch ((chat.last, chat.lastVoiceMs)) {
              (null, _) => null,
              (_, final ms?) when chat.lastMine => l10n.directPreviewMine(
                l10n.directVoicePreview(voiceTime(ms)),
              ),
              (_, final ms?) => l10n.directVoicePreview(voiceTime(ms)),
              (final last?, _) when chat.lastMine => l10n.directPreviewMine(
                last,
              ),
              (final last, _) => last,
            },
            unread: chat.friend.id == shown ? 0 : chat.unread,
            selected: chat.friend.id == shown,
          ),
      ],
    );
  }
}

class _ChatRow extends StatelessWidget {
  const _ChatRow({
    required this.friend,
    required this.online,
    required this.away,
    required this.preview,
    required this.unread,
    required this.selected,
  });
  final UiPerson friend;
  final bool online;
  final bool away;
  final String? preview;
  final int unread;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Pressable(
      key: ValueKey('direct-row:${friend.id}'),
      onTap: () => state.direct.show(friend.id),
      label: l10n.directOpenChat(friend.name),
      focusRadius: 10,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        constraints: const BoxConstraints(minHeight: 48),
        margin: const EdgeInsets.only(bottom: 2),
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          color: selected
              ? c.hover
              : hovered
              ? c.hover.withValues(alpha: .6)
              : null,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: selected ? c.border : Colors.transparent),
        ),
        child: Row(
          children: [
            SizedBox(
              width: 32,
              height: 32,
              // The dot and the dimmed picture say it to the eye; this, to a screen
              // reader.
              child: Semantics(
                label: !online
                    ? l10n.directOffline
                    : away
                    ? l10n.directAway
                    : l10n.directOnline,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Avatar(avatar: friend.avatar, size: 32, dim: !online),
                    if (online)
                      Positioned(
                        right: -1,
                        bottom: -1,
                        child: StatusDot(
                          offline: false,
                          away: away,
                          inset: 2,
                          ringColor: c.bg1,
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    friend.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tx(online ? c.fg : c.fg2, weight: w500, height: 1.3),
                  ),
                  if (preview != null)
                    Text(
                      preview!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg3, size: 12, height: 1.3),
                    ),
                ],
              ),
            ),
            if (unread > 0) ...[
              const SizedBox(width: 6),
              Container(
                height: heightForText(context, 18, 11),
                constraints: BoxConstraints(
                  minWidth: heightForText(context, 18, 11),
                ),
                padding: const EdgeInsets.symmetric(horizontal: 5),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: c.pri,
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  '$unread',
                  style: tx(c.onPri, size: 11, weight: w600, height: 1.2),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

// ----- The chat's page -----

/// A direct chat: who it is with and how they are, the disappearing timer and the call
/// button on top; the messages by day and person, with their pictures, videos and files;
/// the box to write; and beside it the friend's card with what the chat takes here.
class DirectPage extends StatelessWidget {
  const DirectPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final page = state.direct.page;
    if (page == null) {
      final c = context.fg;
      return Center(child: Spinner(size: 18, color: c.fg3));
    }
    return LayoutBuilder(
      builder: (context, box) => Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: PickerArea(child: _Chat(page: page)),
          ),
          if (box.maxWidth >= 860) _Card(page: page),
        ],
      ),
    );
  }
}

class _Chat extends StatelessWidget {
  const _Chat({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Header(page: page),
        if (page.altered) const _Altered(),
        Expanded(child: _Messages(page: page)),
        if (page.endedMs != null)
          _Ended(page: page)
        else
          _Composer(key: ValueKey('composer:${page.friend.id}'), page: page),
      ],
    );
  }
}

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
                  Text.rich(emojiText(shownText(item.text), tx(c.fg))),
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

// ----- Attachments -----

/// An attachment in the chat: a picture, a video or a file, as it stands here.
class _FileCard extends StatelessWidget {
  const _FileCard({
    required this.file,
    required this.item,
    required this.page,
    required this.writer,
  });
  final UiDirectFile file;
  final UiDirectItem item;
  final UiDirectPage page;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    if (file.state == UiFileState.removed) {
      return _Gone(file: file, page: page);
    }
    if (file.voice != null) {
      final state = AppScope.read(context);
      final canFetch = _canFetch(state, page);
      return VoiceCard(
        friend: page.friend.id,
        file: file,
        mine: item.mine,
        canFetch: canFetch,
        onFetch: () => state.direct.fetch(file.id),
        notHere: file.state != UiFileState.here && !canFetch
            ? capitalized(availableWhen(state, page))
            : null,
      );
    }
    final path = file.path;
    return switch (file.kind) {
      // One larger than all the pictures kept for the screen together is not read into
      // memory at every refresh: it shows as a file.
      UiFileKind.picture when path != null && file.size <= _picturesKept =>
        _Picture(file: file, path: path, mine: item.mine, writer: writer),
      UiFileKind.video when file.video != null => _Video(
        file: file,
        page: page,
        writer: writer,
      ),
      _ => _Document(file: file, page: page),
    };
  }
}

/// Both are online and the chat connected: files come now.
bool _canFetch(AppState state, UiDirectPage page) =>
    page.connected &&
    page.online &&
    page.endedMs == null &&
    !state.prefs.appearOffline;

/// When a file that is not here yet can come: `available when Sofía is online`.
String availableWhen(AppState state, UiDirectPage page) =>
    state.prefs.appearOffline && page.online
    ? l10n.directAvailableWhenYouOnline
    : l10n.directAvailableWhenOnline(page.friend.name);

/// Pictures read from the chat's folder for the screen, by path: the newest few, so a
/// long chat does not keep every picture in memory.
final _pictures = <String, Uint8List>{};
const _picturesKept = 64 * 1024 * 1024;

/// Whether a file kept here is sealed with the data folder's key: then only the core reads
/// it.
Future<bool> _sealed(String path) async {
  final head = await File(path)
      .openRead(0, 7)
      .fold<List<int>>([], (all, part) => all..addAll(part));
  return String.fromCharCodes(head) == ' fogata';
}

/// Reads a picture kept here, through the core when it is sealed.
Future<Uint8List?> _read(String path, String friend, String file) async {
  final kept = _pictures.remove(path);
  if (kept != null) {
    _pictures[path] = kept;
    return kept;
  }
  try {
    final local = File(path);
    if (await local.length() > _picturesKept) return null;
    final bytes = await _sealed(path)
        ? await directFileBytes(
            deviceId: friend,
            file: file,
            maxBytes: _picturesKept,
          )
        : await readAtMost(local.openRead(0, _picturesKept + 1), _picturesKept);
    // A file that grew since its size was looked at is not kept.
    if (bytes == null) return null;
    _pictures[path] = bytes;
    var total = _pictures.values.fold<int>(0, (n, b) => n + b.length);
    while (total > _picturesKept && _pictures.length > 1) {
      final oldest = _pictures.keys.first;
      total -= _pictures.remove(oldest)!.length;
    }
    return bytes;
  } catch (_) {
    return null;
  }
}

class _Picture extends StatefulWidget {
  const _Picture({
    required this.file,
    required this.path,
    required this.mine,
    required this.writer,
  });
  final UiDirectFile file;
  final String path;
  final bool mine;
  final UiPerson writer;

  @override
  State<_Picture> createState() => _PictureState();
}

class _PictureState extends State<_Picture> {
  Uint8List? _bytes;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(_Picture old) {
    super.didUpdateWidget(old);
    if (old.path != widget.path) _load();
  }

  /// Reads the picture, and shows it as a file when it is no picture this computer can
  /// draw (damaged, or one that would take too much memory once decoded).
  Future<void> _load() async {
    final friend = AppScope.read(context).direct.open;
    if (friend == null) return;
    final bytes = await _read(widget.path, friend, widget.file.id);
    final fits = bytes != null && await fitsToDecode(bytes, maxPicturePixels);
    if (!mounted) return;
    setState(() {
      _bytes = fits ? bytes : null;
      _failed = !fits;
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final file = widget.file;
    final picture = _bytes;
    if (_failed) return _Document(file: file, page: state.direct.page!);
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final settings = state.direct.storage?.settings;
    final byItself =
        !widget.mine &&
        file.size <= autoPictureBytes &&
        (settings?.autoPictures ?? true);
    final caption = [
      file.name,
      bytes(file.size),
      if (byItself) l10n.directDownloadedByItself,
    ].join(' · ');
    return _Hovered(
      builder: (hovered) => Pressable(
        onTap: picture == null
            ? null
            : () => _showPicture(context, picture, file, widget.writer),
        label: l10n.directOpenFile(file.name),
        focusRadius: 14,
        builder: (context, _) => AnimatedScale(
          scale: hovered ? 1.02 : 1,
          duration: springTime,
          curve: spring,
          child: Container(
            width: 300,
            height: 200,
            clipBehavior: Clip.antiAlias,
            // Inside the outline, as when it was drawn under what is inside.
            padding: const EdgeInsets.all(1),
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(14),
            ),
            // Over what is inside, which would cover it at the rounded corners.
            foregroundDecoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: c.grid),
            ),
            child: Stack(
              fit: StackFit.expand,
              children: [
                if (picture != null)
                  ChatPicture(
                    bytes: picture,
                    still: Image(
                      image: safePicture(
                        picture,
                        width: (300 * dpr).round(),
                        maxPixels: maxPicturePixels,
                      ),
                      fit: BoxFit.cover,
                      gaplessPlayback: true,
                      errorBuilder: (_, _, _) => const SizedBox.shrink(),
                    ),
                  ),
                Positioned(
                  left: 8,
                  bottom: 8,
                  right: 8,
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: _OverPill(text: caption),
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

/// A video's first picture, darkened until it is here, with the way to get or play it.
class _Video extends StatelessWidget {
  const _Video({required this.file, required this.page, required this.writer});
  final UiDirectFile file;
  final UiDirectPage page;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final video = file.video!;
    final here = file.state == UiFileState.here && file.path != null;
    final coming = file.state == UiFileState.coming;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final length = videoLength(video.durationMs);
    return _Hovered(
      builder: (hovered) => Pressable(
        key: ValueKey('direct-video:${file.id}'),
        onTap: here
            ? () => _showVideo(context, file, page.friend.id, writer)
            : null,
        label: l10n.directPlayFile(file.name),
        focusRadius: 14,
        builder: (context, _) => AnimatedScale(
          scale: here && hovered ? 1.02 : 1,
          duration: springTime,
          curve: spring,
          child: Container(
            width: 300,
            height: 169,
            clipBehavior: Clip.antiAlias,
            decoration: BoxDecoration(
              color: c.bg2,
              borderRadius: BorderRadius.circular(14),
            ),
            child: Stack(
              fit: StackFit.expand,
              children: [
                ColorFiltered(
                  // Darker until it is here, so what to do with it reads over it.
                  colorFilter: ColorFilter.matrix(_brightness(here ? 1 : .55)),
                  child: Image(
                    image: safePicture(video.thumb, width: (300 * dpr).round()),
                    fit: BoxFit.cover,
                    gaplessPlayback: true,
                    errorBuilder: (_, _, _) => const SizedBox.shrink(),
                  ),
                ),
                if (here) ...[
                  Center(
                    child: AnimatedScale(
                      scale: hovered ? 1.08 : 1,
                      duration: springTime,
                      curve: spring,
                      child: Container(
                        width: 52,
                        height: 52,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          color: const Color.fromRGBO(0, 0, 0, .55),
                          shape: BoxShape.circle,
                          border: Border.all(
                            color: Colors.white.withValues(alpha: .18),
                          ),
                        ),
                        child: const FgIcon(
                          Ic.play,
                          size: 20,
                          stroke: 2.6,
                          color: Colors.white,
                        ),
                      ),
                    ),
                  ),
                  Positioned(
                    left: 8,
                    bottom: 8,
                    child: _OverPill(text: length, mono: true),
                  ),
                  Positioned(
                    right: 8,
                    bottom: 8,
                    child: _OverPill(text: bytes(file.size)),
                  ),
                ] else
                  Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (coming)
                          _ComingPill(file: file)
                        else
                          _LightButton(
                            key: ValueKey('direct-get:${file.id}'),
                            icon: Ic.download,
                            text: l10n.directDownloadSize(bytes(file.size)),
                            onTap: _canFetch(state, page)
                                ? () => state.direct.fetch(file.id)
                                : null,
                          ),
                        const SizedBox(height: 8),
                        Text(
                          _canFetch(state, page) || coming
                              ? l10n.directVideoPlaysWhenHere(file.name, length)
                              : l10n.directVideoNotHere(
                                  file.name,
                                  length,
                                  availableWhen(state, page),
                                ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tx(const Color(0xFFD4D4D4), size: 11),
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
}

/// A file (or a picture or video that cannot be drawn): its name and size, and the way
/// to get it or save it.
class _Document extends StatelessWidget {
  const _Document({required this.file, required this.page});
  final UiDirectFile file;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final here = file.state == UiFileState.here;
    final coming = file.state == UiFileState.coming;
    return Container(
      width: 320,
      padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: c.hover,
              borderRadius: BorderRadius.circular(10),
            ),
            child: FgIcon(Ic.file, size: 16, color: c.fg2),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  file.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, size: 13, weight: w500, height: 1.35),
                ),
                Text(
                  coming
                      ? l10n.directGettingIt(
                          file.size > 0 ? file.received * 100 ~/ file.size : 0,
                        )
                      : here || _canFetch(state, page)
                      ? bytes(file.size)
                      : l10n.directFileNotHere(
                          bytes(file.size),
                          availableWhen(state, page),
                        ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg3, size: 12, height: 1.35),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          if (coming)
            Spinner(size: 14, color: c.fg3)
          else if (here)
            FgButton(
              text: l10n.commonSave,
              icon: Ic.download,
              height: 28,
              fontSize: 12,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              tooltip: l10n.directSaveToDownloads,
              onTap: () => state.direct.save(file.id),
            )
          else
            FgButton(
              key: ValueKey('direct-get:${file.id}'),
              text: l10n.commonDownload,
              icon: Ic.download,
              height: 28,
              fontSize: 12,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              onTap: _canFetch(state, page)
                  ? () => state.direct.fetch(file.id)
                  : null,
            ),
        ],
      ),
    );
  }
}

/// An attachment removed here to free space or cleared: the message stays, and it can be
/// asked for again while the friend still has it.
class _Gone extends StatelessWidget {
  const _Gone({required this.file, required this.page});
  final UiDirectFile file;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Container(
      width: 320,
      padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: c.hover,
              borderRadius: BorderRadius.circular(10),
            ),
            child: FgIcon(Ic.fileX, size: 16, color: c.fg3),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${file.name} · ${bytes(file.size)}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg2, size: 13, height: 1.35),
                ),
                Text(
                  l10n.directRemovedForSpace,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg3, size: 12, height: 1.35),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          FgButton(
            key: ValueKey('direct-again:${file.id}'),
            text: l10n.directAskAgain,
            height: 28,
            fontSize: 12,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            tooltip: _canFetch(state, page)
                ? null
                : capitalized(availableWhen(state, page)),
            onTap: _canFetch(state, page)
                ? () => state.direct.fetch(file.id)
                : null,
          ),
        ],
      ),
    );
  }
}

class _ComingPill extends StatelessWidget {
  const _ComingPill({required this.file});
  final UiDirectFile file;

  @override
  Widget build(BuildContext context) {
    final percent = file.size > 0 ? file.received * 100 ~/ file.size : 0;
    return Container(
      height: 34,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: BoxDecoration(
        color: const Color.fromRGBO(16, 16, 16, .72),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Spinner(size: 13, color: Colors.white),
          const SizedBox(width: 8),
          Text(
            l10n.directGettingIt(percent),
            style: tx(Colors.white, size: 13, weight: w500, height: 1.2),
          ),
        ],
      ),
    );
  }
}

/// A colour matrix that scales how bright a picture is.
List<double> _brightness(double b) => [
  b, 0, 0, 0, 0, //
  0, b, 0, 0, 0, //
  0, 0, b, 0, 0, //
  0, 0, 0, 1, 0,
];

/// `0:42`, or `1:02:05` past an hour.
String videoLength(int ms) {
  final d = Duration(milliseconds: ms);
  final s = (d.inSeconds % 60).toString().padLeft(2, '0');
  if (d.inHours > 0) {
    final m = (d.inMinutes % 60).toString().padLeft(2, '0');
    return '${d.inHours}:$m:$s';
  }
  return '${d.inMinutes}:$s';
}

/// A little dark pill over a picture.
class _OverPill extends StatelessWidget {
  const _OverPill({required this.text, this.mono = false});
  final String text;
  final bool mono;

  @override
  Widget build(BuildContext context) => Container(
    height: 22,
    padding: const EdgeInsets.symmetric(horizontal: 8),
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: const Color.fromRGBO(16, 16, 16, .72),
      borderRadius: BorderRadius.circular(999),
    ),
    child: Text(
      text,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tx(Colors.white, size: 11, monospace: mono, height: 1.2),
    ),
  );
}

/// A light pill or round button over a dark picture.
class _LightButton extends StatelessWidget {
  const _LightButton({
    super.key,
    required this.icon,
    required this.onTap,
    this.text,
    this.tooltip,
  });
  final Ic icon;
  final String? text;
  final String? tooltip;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => Opacity(
    opacity: onTap == null ? .5 : 1,
    child: Springy(
      onTap: onTap,
      label: text ?? tooltip,
      tooltip: tooltip,
      builder: (context, hovered) => Container(
        height: 34,
        width: text == null ? 34 : null,
        padding: text == null ? null : const EdgeInsets.fromLTRB(13, 0, 15, 0),
        alignment: text == null ? Alignment.center : null,
        decoration: BoxDecoration(
          color: hovered ? Colors.white : const Color(0xFFEDEDED),
          borderRadius: BorderRadius.circular(999),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            FgIcon(icon, size: 14, color: const Color(0xFF0A0A0A)),
            if (text != null) ...[
              const SizedBox(width: 7),
              Text(
                text!,
                style: tx(const Color(0xFF0A0A0A), size: 13, weight: w600),
              ),
            ],
          ],
        ),
      ),
    ),
  );
}

/// Builds its child knowing whether the pointer is over it.
class _Hovered extends StatefulWidget {
  const _Hovered({required this.builder});
  final Widget Function(bool hovered) builder;

  @override
  State<_Hovered> createState() => _HoveredState();
}

class _HoveredState extends State<_Hovered> {
  bool _on = false;

  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => setState(() => _on = true),
    onExit: (_) => setState(() => _on = false),
    child: widget.builder(_on),
  );
}

/// What sits over a picture or video shown large: who sent it, its name, Save and close.
class _Top extends StatelessWidget {
  const _Top({required this.file, required this.writer});
  final UiDirectFile file;
  final UiPerson writer;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    return Row(
      children: [
        Avatar(avatar: writer.avatar, size: 32),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                writer.name,
                style: tx(Colors.white, size: 14, weight: w600),
              ),
              Text(
                '${file.name} · ${bytes(file.size)}',
                overflow: TextOverflow.ellipsis,
                style: tx(Colors.white60, size: 12),
              ),
            ],
          ),
        ),
        _LightButton(
          text: l10n.commonSave,
          icon: Ic.download,
          onTap: () => state.direct.save(file.id),
        ),
        const SizedBox(width: 8),
        _LightButton(
          icon: Ic.close,
          tooltip: l10n.commonClose,
          onTap: Navigator.of(context).pop,
        ),
      ],
    );
  }
}

Future<void> _over(BuildContext context, String label, Widget child) {
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: label,
    barrierColor: const Color.fromRGBO(0, 0, 0, .9),
    transitionDuration: springTime,
    pageBuilder: (context, _, _) => child,
    transitionBuilder: (context, animation, _, child) {
      final t = CurvedAnimation(parent: animation, curve: spring);
      return FadeTransition(
        opacity: animation,
        child: ScaleTransition(
          scale: Tween(begin: .94, end: 1.0).animate(t),
          child: child,
        ),
      );
    },
  );
}

/// A picture large over the window; a click beside it or Esc closes it.
Future<void> _showPicture(
  BuildContext context,
  Uint8List picture,
  UiDirectFile file,
  UiPerson writer,
) => _over(
  context,
  l10n.directClosePicture,
  Builder(
    builder: (context) {
      final close = Navigator.of(context).pop;
      var screen = View.of(context).display.size;
      if (screen.isEmpty) {
        screen =
            MediaQuery.sizeOf(context) * MediaQuery.devicePixelRatioOf(context);
      }
      return CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): close},
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusScope(
          autofocus: true,
          child: Material(
            type: MaterialType.transparency,
            child: Stack(
              children: [
                Positioned.fill(
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onTap: close,
                  ),
                ),
                Positioned.fill(
                  top: 112,
                  bottom: 40,
                  left: 40,
                  right: 40,
                  child: Center(
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(16),
                      child: ChatPicture(
                        bytes: picture,
                        still: Image(
                          image: ResizeImage(
                            SafeMemoryImage(
                              picture,
                              maxPixels: maxPicturePixels,
                            ),
                            width: screen.width.round(),
                            height: screen.height.round(),
                            policy: ResizeImagePolicy.fit,
                          ),
                          fit: BoxFit.contain,
                          gaplessPlayback: true,
                        ),
                      ),
                    ),
                  ),
                ),
                Positioned(
                  left: 24,
                  right: 24,
                  top: 56,
                  child: _Top(file: file, writer: writer),
                ),
              ],
            ),
          ),
        ),
      );
    },
  ),
);

@visibleForTesting
Future<void> showDirectVideo(
  BuildContext context,
  UiDirectFile file,
  String friend,
  UiPerson writer,
) => _showVideo(context, file, friend, writer);

/// A video kept here, playing over the window with the system's own player.
Future<void> _showVideo(
  BuildContext context,
  UiDirectFile file,
  String friend,
  UiPerson writer,
) => _over(
  context,
  l10n.directCloseVideo,
  _VideoView(file: file, friend: friend, writer: writer),
);

class _VideoView extends StatefulWidget {
  const _VideoView({
    required this.file,
    required this.friend,
    required this.writer,
  });
  final UiDirectFile file;

  /// The friend whose chat holds the video.
  final String friend;
  final UiPerson writer;

  @override
  State<_VideoView> createState() => _VideoViewState();
}

class _VideoViewState extends State<_VideoView> {
  VideoPlayer? _player;
  String? _problem;
  VideoState _at = const VideoState();
  Timer? _tick;
  late final VideoSound _sound;
  bool _closed = false;
  double? _dragging;

  /// The plain copy of a sealed video made for this player.
  String? _copy;

  @override
  void initState() {
    super.initState();
    _sound = VideoSound(AppScope.read(context), () {
      if (mounted) setState(() {});
    });
    _open();
  }

  Future<void> _open() async {
    // A sealed video plays from a plain copy, which goes when the player closes.
    String? path = widget.file.path;
    try {
      if (path != null && await _sealed(path)) {
        path = await directFileToPlay(
          deviceId: widget.friend,
          file: widget.file.id,
        );
        _copy = path;
      }
    } catch (_) {
      path = null;
    }
    if (!mounted || _closed) {
      _forgetCopy(Future<void>.value());
      return;
    }
    final player = path == null
        ? null
        : await VideoPlayer.open(
            path,
            output: AppScope.read(context).prefs.audioOutput,
          );
    if (!mounted || _closed) {
      _forgetCopy(player?.close() ?? Future<void>.value());
      return;
    }
    if (player == null) {
      _forgetCopy(Future<void>.value());
      setState(() => _problem = l10n.directCannotPlay);
      return;
    }
    await _sound.attach(player);
    if (!mounted || _closed) {
      _forgetCopy(player.close());
      return;
    }
    setState(() => _player = player);
    await player.play();
    // Closed while it started: dispose has closed the player, and no timer is left to it.
    if (!mounted || _closed) return;
    _tick = Timer.periodic(const Duration(milliseconds: 200), (_) async {
      final at = await player.state();
      if (!mounted || _closed) return;
      setState(() {
        _at = at;
        if (at.error != null) {
          _problem ??= l10n.directCannotPlay;
        }
      });
    });
  }

  @override
  void dispose() {
    _closed = true;
    _tick?.cancel();
    _sound.dispose();
    // A player still opening is closed where it opens.
    if (_player case final player?) _forgetCopy(player.close());
    super.dispose();
  }

  /// The plain copy of a sealed video goes once the player has let it go (`closing`); the
  /// copies of other players stay.
  void _forgetCopy(Future<void> closing) {
    final copy = _copy;
    if (copy == null) return;
    _copy = null;
    unawaited(
      closing
          .then((_) => forgetDirectFileCopy(path: copy))
          .catchError((Object _) {}),
    );
  }

  Future<void> _toggle() async {
    final player = _player;
    if (player == null) return;
    if (_at.playing) {
      await player.pause();
    } else {
      await player.play();
    }
    final at = await player.state();
    if (mounted) setState(() => _at = at);
  }

  @override
  Widget build(BuildContext context) {
    final close = Navigator.of(context).pop;
    final video = widget.file.video;
    final ratio = video != null && video.width > 0 && video.height > 0
        ? video.width / video.height
        : 16 / 9;
    final player = _player;
    final duration = _at.durationMs > 0
        ? _at.durationMs
        : (video?.durationMs ?? 0);
    final position = _dragging != null
        ? (_dragging! * duration).round()
        : _at.positionMs.clamp(0, duration == 0 ? 0 : duration);
    final fraction = duration == 0 ? 0.0 : position / duration;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): close,
        const SingleActivator(LogicalKeyboardKey.space): _toggle,
      },
      // A scope: Tab goes round what floats here, not the page behind.
      child: FocusScope(
        autofocus: true,
        child: Material(
          type: MaterialType.transparency,
          child: Stack(
            children: [
              Positioned.fill(
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: close,
                ),
              ),
              Positioned.fill(
                top: 112,
                bottom: 40,
                left: 40,
                right: 40,
                child: Center(
                  child: AspectRatio(
                    aspectRatio: ratio,
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(16),
                      child: Stack(
                        fit: StackFit.expand,
                        children: [
                          if (player != null && _problem == null)
                            GestureDetector(
                              onTap: _toggle,
                              child: ColoredBox(
                                color: Colors.black,
                                child: VideoView(player: player),
                              ),
                            )
                          else
                            Center(
                              child: _problem != null
                                  ? Text(
                                      _problem!,
                                      style: tx(Colors.white, size: 14),
                                    )
                                  : const Spinner(
                                      size: 18,
                                      color: Colors.white,
                                    ),
                            ),
                          Positioned(
                            left: 0,
                            right: 0,
                            bottom: 18,
                            child: Center(
                              child: Container(
                                width: 520,
                                height: 48,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 8,
                                ),
                                decoration: BoxDecoration(
                                  color: const Color.fromRGBO(17, 17, 17, .82),
                                  borderRadius: BorderRadius.circular(999),
                                  border: Border.all(
                                    color: Colors.white.withValues(alpha: .08),
                                  ),
                                ),
                                child: Row(
                                  children: [
                                    _LightButton(
                                      icon: _at.playing ? Ic.pause : Ic.play,
                                      tooltip: _at.playing
                                          ? l10n.directPause
                                          : l10n.directPlay,
                                      onTap: player == null ? null : _toggle,
                                    ),
                                    const SizedBox(width: 10),
                                    SizedBox(
                                      width: 96,
                                      child: Text(
                                        '${videoLength(position)} / ${videoLength(duration)}',
                                        style: tx(
                                          Colors.white70,
                                          size: 12,
                                          monospace: true,
                                        ),
                                      ),
                                    ),
                                    Expanded(
                                      child: SliderTheme(
                                        data: SliderTheme.of(context).copyWith(
                                          activeTrackColor: Colors.white,
                                          inactiveTrackColor: Colors.white24,
                                          thumbColor: Colors.white,
                                          overlayColor: Colors.white12,
                                          trackHeight: 3,
                                          thumbShape:
                                              const RoundSliderThumbShape(
                                                enabledThumbRadius: 6,
                                              ),
                                        ),
                                        child: Slider(
                                          value: fraction.clamp(0.0, 1.0),
                                          onChanged:
                                              player == null || duration == 0
                                              ? null
                                              : (v) => setState(
                                                  () => _dragging = v,
                                                ),
                                          onChangeEnd: (v) async {
                                            setState(() => _dragging = null);
                                            await player?.seek(
                                              (v * duration).round(),
                                            );
                                          },
                                        ),
                                      ),
                                    ),
                                    FgButton(
                                      icon: _sound.muted
                                          ? Ic.volumeOff
                                          : Ic.volume,
                                      kind: Kind.ghost,
                                      height: 32,
                                      iconColor: Colors.white70,
                                      tooltip: _sound.muted
                                          ? l10n.directSoundOn
                                          : l10n.directMute,
                                      onTap: player == null
                                          ? null
                                          : _sound.toggle,
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
              Positioned(
                left: 24,
                right: 24,
                top: 56,
                child: _Top(file: widget.file, writer: widget.writer),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ----- Writing -----

class _Composer extends StatefulWidget {
  const _Composer({super.key, required this.page});
  final UiDirectPage page;

  @override
  State<_Composer> createState() => _ComposerState();
}

class _ComposerState extends State<_Composer> {
  final _text = TextEditingController();
  final _focus = FocusNode();
  int _lastWaiting = 0;

  late final AppState _app;

  /// Tells the friend this device is typing while the message box changes, while both
  /// are online.
  late final _typing = TypingSender(
    (on) => _app.direct.typing(
      widget.page.friend.id,
      on && writeBlocked(_app, widget.page) == null,
    ),
  );

  @override
  void initState() {
    super.initState();
    _app = AppScope.read(context);
    // What was being written in this chat before another was opened.
    final draft = _app.direct.drafts[widget.page.friend.id];
    if (draft != null) {
      _text.value = TextEditingValue(
        text: draft,
        selection: TextSelection.collapsed(offset: draft.length),
      );
    }
    _text.addListener(() {
      _typing.changed(_text.text);
      _keepDraft();
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  @override
  void dispose() {
    _typing.stopped();
    _app.direct.voice.leftChat(widget.page.friend.id);
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _keepDraft() {
    final drafts = _app.direct.drafts;
    final friend = widget.page.friend.id;
    if (_text.text.isEmpty) {
      drafts.remove(friend);
    } else {
      drafts[friend] = _text.text;
    }
  }

  Future<void> _send() async {
    final app = AppScope.read(context);
    if (writeBlocked(app, widget.page) != null) return;
    final direct = app.direct;
    final text = _text.text;
    if (text.trim().isEmpty && direct.waiting.isEmpty) return;
    _text.clear();
    _focus.requestFocus();
    final sent = await direct.send(text);
    // It did not go: what was written is back in the box, unless more was written since.
    if (!sent && mounted && _text.text.trim().isEmpty) {
      final kept = direct.drafts[widget.page.friend.id] ?? text;
      _text.value = TextEditingValue(
        text: kept,
        selection: TextSelection.collapsed(offset: kept.length),
      );
    }
  }

  /// Esc with files waiting: asks before taking them all out of the message.
  Future<void> _removeWaiting(int count) async {
    final direct = AppScope.read(context).direct;
    final friend = direct.open;
    final ok = await confirmDialog(
      context,
      title: l10n.directRemoveFilesTitle(count),
      text: l10n.directRemoveFilesText,
      action: l10n.commonRemove,
    );
    if (ok && direct.open == friend) direct.clearOutgoing();
    if (mounted) _focus.requestFocus();
  }

  Future<void> _pick() async {
    final direct = AppScope.read(context).direct;
    final files = await openFiles();
    if (files.isEmpty) return;
    direct.addOutgoing(files.map((f) => f.path));
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final page = widget.page;
    final waiting = state.direct.waiting;
    final going = state.direct.sendingHere;
    final files = going.isNotEmpty || waiting.isNotEmpty;
    if (waiting.length > _lastWaiting) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _focus.requestFocus();
      });
    }
    _lastWaiting = waiting.length;
    // Nothing can be written while either of the two is offline: what was written stays
    // in the box, and comes back with it.
    final blocked = writeBlocked(state, page);
    final voice = state.direct.voice;
    if (blocked != null) {
      // Recording stops with the microphone off; the recording waits for both online.
      if (voice.recordingFor == page.friend.id) {
        WidgetsBinding.instance.addPostFrameCallback(
          (_) => voice.pauseRecording(),
        );
      }
      return Padding(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const SizedBox(height: 18),
            Container(
              key: const ValueKey('direct-locked'),
              height: 46,
              padding: const EdgeInsets.symmetric(horizontal: 16),
              decoration: BoxDecoration(
                color: c.bg2,
                borderRadius: BorderRadius.circular(999),
                border: Border.all(color: c.border),
              ),
              child: Row(
                children: [
                  FgIcon(Ic.lock, size: 14, color: c.fg3),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      blocked,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg2, size: 13),
                    ),
                  ),
                ],
              ),
            ),
            if (waiting.isNotEmpty || _text.text.trim().isNotEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 6, 4, 0),
                child: Text(l10n.directDraftKept, style: tx(c.fg3, size: 11)),
              ),
          ],
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 0, 24, 18),
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () {
            if (voice.recordingIn(page.friend.id)) {
              voice.escape();
            } else if (waiting.isNotEmpty) {
              _removeWaiting(waiting.length);
            }
          },
        },
        child: ListenableBuilder(
          listenable: voice,
          builder: (context, _) => _box(context, voice, waiting, going, files),
        ),
      ),
    );
  }

  /// The box to write in, or to record in, with what goes over and under it.
  Widget _box(
    BuildContext context,
    VoiceMessages voice,
    List<OutgoingFile> waiting,
    List<OutgoingFile> going,
    bool files,
  ) {
    final state = AppScope.read(context);
    final c = context.fg;
    final page = widget.page;
    final name = page.friend.name;
    final recording = voice.recordingIn(page.friend.id);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (voice.micProblem && !recording) const MicProblemNote(),
        if (recording && voice.micStopped) const MicProblemNote(stopped: true),
        ValueListenableBuilder(
          valueListenable: state.typingChanged,
          builder: (context, _, _) => TypingLine(
            names: [if (state.direct.typingHere(page.friend.id)) name],
            padding: const EdgeInsets.only(left: 4, bottom: 2),
          ),
        ),
        AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          decoration: BoxDecoration(
            color: c.bg2,
            borderRadius: BorderRadius.circular(files ? 22 : 999),
            // The box to write in is a control: its edge is seen at 3:1.
            border: Border.all(color: c.edge),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (files) ...[
                Padding(
                  padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    spacing: 6,
                    children: [
                      for (final f in going) _Waiting(file: f, sending: true),
                      for (final f in waiting) _Waiting(file: f),
                    ],
                  ),
                ),
                Container(height: 1, color: c.border),
              ],
              Padding(
                padding: const EdgeInsets.fromLTRB(6, 5, 5, 5),
                child: recording
                    ? RecordingBar(friend: page.friend.id)
                    : Row(
                        children: [
                          Springy(
                            onTap: _pick,
                            tooltip: l10n.directAddFilesTooltip,
                            label: l10n.directAddFiles,
                            builder: (context, hovered) => Container(
                              width: 34,
                              height: 34,
                              alignment: Alignment.center,
                              decoration: BoxDecoration(
                                color: hovered
                                    ? Color.lerp(c.hover, c.fg, .06)
                                    : c.hover,
                                shape: BoxShape.circle,
                              ),
                              child: FgIcon(Ic.plus, size: 15, color: c.fg2),
                            ),
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: EmojiComplete(
                              controller: _text,
                              max: maxMessage,
                              child: FgField(
                                key: const ValueKey('direct-message'),
                                controller: _text,
                                focusNode: _focus,
                                height: 34,
                                borderless: true,
                                placeholder: waiting.isEmpty
                                    ? l10n.directMessagePlaceholder(name)
                                    : l10n.directAddMessageOptional,
                                onSubmitted: (_) => _send(),
                                maxLength: maxMessage,
                                label: l10n.directMessagePlaceholder(name),
                              ),
                            ),
                          ),
                          ValueListenableBuilder(
                            valueListenable: _text,
                            builder: (context, value, _) {
                              final length = value.text.runes.length;
                              if (length < maxMessage - 200) {
                                return const SizedBox.shrink();
                              }
                              return Padding(
                                padding: const EdgeInsets.only(
                                  left: 6,
                                  right: 4,
                                ),
                                child: Text(
                                  '${maxMessage - length}',
                                  style: tx(
                                    length >= maxMessage ? c.red : c.fg3,
                                    size: 12,
                                    monospace: true,
                                  ),
                                ),
                              );
                            },
                          ),
                          const SizedBox(width: 4),
                          EmojiButton(
                            controller: _text,
                            focusNode: _focus,
                            max: maxMessage,
                          ),
                          const SizedBox(width: 4),
                          MicButton(friend: page.friend.id),
                          const SizedBox(width: 4),
                          Springy(
                            key: const ValueKey('direct-send'),
                            onTap: _send,
                            tooltip: l10n.commonSend,
                            label: l10n.commonSend,
                            builder: (context, hovered) => Container(
                              width: 34,
                              height: 34,
                              alignment: Alignment.center,
                              decoration: BoxDecoration(
                                color: hovered
                                    ? c.pri.withValues(alpha: .88)
                                    : c.pri,
                                shape: BoxShape.circle,
                              ),
                              child: FgIcon(
                                Ic.send,
                                size: 14,
                                stroke: 2.4,
                                color: c.onPri,
                              ),
                            ),
                          ),
                        ],
                      ),
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(4, 6, 4, 0),
          child: recording
              ? RecordingUnder(
                  microphone: voice.micName,
                  fellBack: voice.fellBack,
                  escDeletes: voice.escDeletes,
                )
              : Text(l10n.directAutoDownloadNote, style: tx(c.fg3, size: 11)),
        ),
      ],
    );
  }
}

/// A file waiting for Send above the message box, with a button to take it out; or, once
/// sent, saying so until the message shows in the chat.
class _Waiting extends StatelessWidget {
  const _Waiting({required this.file, this.sending = false});
  final OutgoingFile file;
  final bool sending;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Row(
      children: [
        FgIcon(Ic.file, size: 15, color: c.fg2),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            file.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tx(c.fg, size: 13),
          ),
        ),
        const SizedBox(width: 8),
        Text(bytes(file.size), style: tx(c.fg3, size: 12, monospace: true)),
        const SizedBox(width: 4),
        if (sending)
          SizedBox(
            height: 26,
            child: Center(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Text(l10n.directSending, style: tx(c.fg3, size: 12)),
              ),
            ),
          )
        else
          FgButton(
            icon: Ic.close,
            kind: Kind.ghost,
            height: 26,
            tooltip: l10n.directRemoveFile(file.name),
            onTap: () => state.direct.removeOutgoing(file),
          ),
      ],
    );
  }
}

/// Instead of the message box, once the two are no longer friends.
/// The chat's history on this computer was not as Fogata last saved it: changed, cut
/// short or put back as an older copy, by a disk fault or by someone with the files.
class _Altered extends StatelessWidget {
  const _Altered();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: const ValueKey('direct-altered'),
      margin: const EdgeInsets.fromLTRB(24, 12, 24, 0),
      padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
      decoration: BoxDecoration(
        color: c.amberSoft,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.amberLine),
      ),
      child: Row(
        children: [
          FgIcon(Ic.warning, size: 16, color: c.amber),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              l10n.directAltered,
              style: tx(c.fg2, size: 13, height: 1.4),
            ),
          ),
        ],
      ),
    );
  }
}

class _Ended extends StatelessWidget {
  const _Ended({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final name = page.friend.name;
    return Container(
      margin: const EdgeInsets.fromLTRB(24, 8, 24, 18),
      padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: c.border),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(l10n.directEnded(name), style: tx(c.fg2, size: 13)),
          ),
          const SizedBox(width: 12),
          FgButton(
            key: const ValueKey('direct-delete'),
            text: l10n.directDeleteChat,
            icon: Ic.trash,
            kind: Kind.danger,
            height: 32,
            fontSize: 13,
            onTap: () async {
              final yes = await showFgDialog<bool>(
                context,
                maxWidth: 420,
                builder: (context) => _ConfirmDelete(name: name),
              );
              if (yes ?? false) await state.direct.delete(page.friend.id);
            },
          ),
        ],
      ),
    );
  }
}

class _ConfirmDelete extends StatelessWidget {
  const _ConfirmDelete({required this.name});
  final String name;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.directDeleteTitle(name),
                style: tx(c.fg, size: 16, weight: w600),
              ),
              const SizedBox(height: 6),
              Text(l10n.directDeleteText(name), style: tx(c.fg2, size: 13)),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(false),
            ),
            const SizedBox(width: 8),
            FgButton(
              text: l10n.directDelete,
              kind: Kind.danger,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(true),
            ),
          ],
        ),
      ],
    );
  }
}

// ----- The friend's card -----

class _Card extends StatelessWidget {
  const _Card({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final friend = page.friend;
    final banner = state.direct.banners[friend.id];
    final online = page.online && page.endedMs == null;
    final usage = page.usage;
    final rows = [
      (l10n.directUsageMessages, usage.messages, c.fg),
      (l10n.directUsagePictures, usage.pictures, c.blue),
      (l10n.directUsageVideos, usage.videos, c.amber),
      (l10n.directUsageFiles, usage.files, _violet),
    ];
    final total = rows.fold<int>(0, (n, r) => n + r.$2);
    return Container(
      width: 300,
      decoration: BoxDecoration(
        color: c.bg1,
        border: Border(left: BorderSide(color: c.border)),
      ),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Stack(
              clipBehavior: Clip.none,
              children: [
                SizedBox(
                  height: 84,
                  child: PanelBanner(
                    color: banner == null ? c.hover : Color(banner.color),
                    picture: banner?.picture,
                    id: friend.id,
                  ),
                ),
                Positioned(
                  left: 16,
                  top: 48,
                  child: Container(
                    width: 76,
                    height: 76,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: c.bg1,
                      shape: BoxShape.circle,
                    ),
                    child: Avatar(
                      avatar: friend.avatar,
                      size: 66,
                      dim: !online,
                      id: friend.id,
                    ),
                  ),
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 50, 16, 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    friend.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: tx(c.fg, size: 18, weight: w600, height: 1.25),
                  ),
                  Text(
                    friend.short,
                    style: tx(c.fg3, size: 12, monospace: true),
                  ),
                  // A former friend's bio is not known any longer.
                  if (state.contacts.friends
                          .where((f) => f.person.id == friend.id)
                          .firstOrNull
                      case final f?)
                    CardBio(f.bio),
                  const SizedBox(height: 14),
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: c.bg2,
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: c.border),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Text(
                          l10n.directUsageHeading,
                          style: tx(c.fg3, size: 11, weight: w500, spacing: .4),
                        ),
                        const SizedBox(height: 10),
                        _UsageBar(
                          parts: [for (final r in rows) (r.$2, r.$3)],
                          total: total,
                          height: 6,
                        ),
                        const SizedBox(height: 10),
                        for (final (label, size, color) in rows) ...[
                          Row(
                            children: [
                              Dot(color, size: 8),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(label, style: tx(c.fg2, size: 13)),
                              ),
                              Text(
                                storageSize(size),
                                style: tx(c.fg, size: 12, monospace: true),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                        ],
                        const SizedBox(height: 2),
                        FgButton(
                          key: const ValueKey('direct-free'),
                          text: l10n.directFreeUpSpace,
                          height: 32,
                          fontSize: 13,
                          expand: true,
                          onTap: () {
                            state.direct.storageOnOpen = true;
                            state.openSettings();
                          },
                        ),
                      ],
                    ),
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

/// A bar split by what takes the space, each part in its colour.
class _UsageBar extends StatelessWidget {
  const _UsageBar({
    required this.parts,
    required this.total,
    required this.height,
  });
  final List<(int, Color)> parts;

  /// What the whole bar stands for.
  final int total;
  final double height;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return ClipRRect(
      borderRadius: BorderRadius.circular(999),
      child: Container(
        height: height,
        color: c.border,
        child: LayoutBuilder(
          builder: (context, box) => Row(
            children: [
              for (final (size, color) in parts)
                if (size > 0 && total > 0)
                  AnimatedContainer(
                    duration: const Duration(milliseconds: 450),
                    curve: spring,
                    width: box.maxWidth * math.min(1, size / total),
                    color: color,
                  ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The bar of what direct messages take, for the settings: the same parts and colours.
class UsageBar extends StatelessWidget {
  const UsageBar({
    super.key,
    required this.parts,
    required this.total,
    this.height = 10,
  });
  final List<(int, Color)> parts;
  final int total;
  final double height;

  @override
  Widget build(BuildContext context) =>
      _UsageBar(parts: parts, total: total, height: height);
}

/// The colour of files in what direct messages take.
const filesColor = _violet;
