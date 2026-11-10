import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
import '../outgoing.dart' show DropTo, DropZone;
import '../photo.dart' show readAtMost;
import '../safe_image.dart';
import '../shown_text.dart';
import '../theme.dart';
import '../typing.dart';
import '../voice.dart';
import '../widgets.dart';
import 'emoji_picker.dart';
import 'friend_card.dart' show toggleFriendCard;
import 'user_panel.dart' show toggleUserPanel;
import 'members.dart' show confirmDialog;
import 'video_view.dart';
import 'room.dart' show callFriendChecked;
import 'user_panel.dart' show CardBio, PanelBanner, StatusDot;
import 'voice_ui.dart';

part 'direct/attachments.dart';
part 'direct/card.dart';
part 'direct/composer.dart';
part 'direct/header.dart';
part 'direct/messages.dart';

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
            child: PickerArea(
              child: _DropOnChat(state: state, page: page),
            ),
          ),
          if (box.maxWidth >= 860) _Card(page: page),
        ],
      ),
    );
  }
}

/// Files dropped anywhere on the chat wait in its message box, while the + button beside
/// it is there to pick them: the chat has not ended, both people are online, and no
/// voice message is being recorded.
class _DropOnChat extends StatelessWidget {
  const _DropOnChat({required this.state, required this.page});
  final AppState state;
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final voice = state.direct.voice;
    return ListenableBuilder(
      listenable: voice,
      builder: (context, child) => DropZone(
        state: state,
        enabled:
            page.endedMs == null &&
            writeBlocked(state, page) == null &&
            !voice.recordingIn(page.friend.id),
        to: DropTo.direct(state, page.friend.name),
        child: child!,
      ),
      child: _Chat(page: page),
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
