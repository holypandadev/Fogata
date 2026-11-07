import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../room_count.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../widgets.dart';
import 'emoji_picker.dart' show PopIn;
import 'friends.dart' show presenceText;
import 'room.dart' show openRoomChecked;

/// A place the quick switcher can go to.
enum PlaceKind { chat, room, group }

/// A place of the window: a direct chat (with a friend or someone who was one), a voice
/// room or a group's page, with how it is shown in the list.
class Place {
  const Place({
    required this.key,
    required this.kind,
    required this.title,
    required this.detail,
    this.extra = '',
    this.unread = 0,
    this.lastMs = 0,
    this.live = false,
  });

  /// `chat:<device>`, `room:<group>:<voice room>` or `group:<group>`.
  final String key;
  final PlaceKind kind;
  final String title;
  final String detail;

  /// More text that finds it: a voice room's group name.
  final String extra;
  final int unread;
  final int lastMs;
  final bool live;

  String get id => key.substring(key.indexOf(':') + 1);
}

/// The key of the place the window shows now, if it is one the switcher lists.
String? placeShown(AppState state) => switch (state.nav.page) {
  Screen.direct when state.direct.open != null => 'chat:${state.direct.open}',
  Screen.room when state.roomState.roomGroup != null && !state.inCallPage =>
    'room:${state.roomState.roomGroup}:${state.roomState.roomVoice}',
  Screen.members when state.nav.membersGroup != null =>
    'group:${state.nav.membersGroup}',
  _ => null,
};

/// Every place: the direct chats and friends, each group's voice rooms and the groups.
List<Place> allPlaces(AppState state) {
  final places = <Place>[];
  final chats = <String>{};
  for (final chat in state.direct.chats) {
    chats.add(chat.friend.id);
    places.add(
      Place(
        key: 'chat:${chat.friend.id}',
        kind: PlaceKind.chat,
        title: chat.friend.name,
        detail: chat.endedMs != null
            ? l10n.switcherChatEnded
            : chat.online
            ? (chat.away ? l10n.switcherChatAway : l10n.switcherChatOnline)
            : l10n.switcherChat,
        unread: chat.unread,
        lastMs: chat.lastMs.toInt(),
      ),
    );
  }
  for (final f in state.contacts.friends) {
    if (chats.contains(f.person.id)) continue;
    places.add(
      Place(
        key: 'chat:${f.person.id}',
        kind: PlaceKind.chat,
        title: f.person.name,
        detail: l10n.switcherFriend(presenceText(f)),
      ),
    );
  }
  for (final g in state.contacts.groups) {
    for (final v in g.voiceRooms) {
      final people = peopleInRoom(v);
      places.add(
        Place(
          key: 'room:${g.id}:${v.id}',
          kind: PlaceKind.room,
          title: Contacts.voiceName(v),
          detail: people == 0
              ? l10n.switcherVoiceRoom(g.name)
              : l10n.switcherVoiceRoomPeople(g.name, people),
          extra: g.name,
          live: people > 0,
        ),
      );
    }
    places.add(
      Place(
        key: 'group:${g.id}',
        kind: PlaceKind.group,
        title: g.name,
        detail: l10n.switcherGroup(g.members),
      ),
    );
  }
  return places;
}

/// The places that match [query], best first; with no query, the places visited lately
/// ([recent], newest first, without [shown]), then the chats newest first, the voice
/// rooms with people in them, the groups and the other friends.
List<Place> rankPlaces(
  List<Place> places,
  String query, {
  List<String> recent = const [],
  String? shown,
}) {
  final q = query.trim().toLowerCase();
  int seen(Place p) {
    final i = recent.indexOf(p.key);
    return i < 0 ? recent.length : i;
  }

  int rest(Place p) => switch (p) {
    Place(kind: PlaceKind.chat, lastMs: > 0) => 0,
    Place(kind: PlaceKind.room, live: true) => 1,
    Place(kind: PlaceKind.group) => 2,
    Place(kind: PlaceKind.chat) => 3,
    _ => 4,
  };

  int score(Place p) {
    if (q.isEmpty) return 0;
    final title = p.title.toLowerCase();
    if (title.startsWith(q)) return 4;
    if (title.split(RegExp(r'[\s\-_.·]+')).any((w) => w.startsWith(q))) {
      return 3;
    }
    if (title.contains(q)) return 2;
    if (p.extra.toLowerCase().contains(q)) return 1;
    return -1;
  }

  final list = [
    for (final p in places)
      if (score(p) >= 0 && (q.isNotEmpty || p.key != shown)) p,
  ];
  list.sort((a, b) {
    var d = score(b).compareTo(score(a));
    if (d != 0) return d;
    d = seen(a).compareTo(seen(b));
    if (d != 0) return d;
    d = rest(a).compareTo(rest(b));
    if (d != 0) return d;
    d = b.lastMs.compareTo(a.lastMs);
    if (d != 0) return d;
    return a.title.toLowerCase().compareTo(b.title.toLowerCase());
  });
  return list;
}

/// Goes to [place]: a chat opens, a group shows its page (or its room when this device
/// is in it), and a voice room is entered after the usual questions.
Future<void> goToPlace(BuildContext context, Place place) async {
  final state = AppScope.read(context);
  if (state.nav.settingsOpen) {
    if (!await state.closeSettings() || !context.mounted) return;
  }
  switch (place.kind) {
    case PlaceKind.chat:
      await state.direct.show(place.id);
    case PlaceKind.group:
      state.selectGroup(place.id);
    case PlaceKind.room:
      final at = place.id.indexOf(':');
      await openRoomChecked(
        context,
        place.id.substring(0, at),
        voice: place.id.substring(at + 1),
      );
  }
}

/// The quick switcher over the window (Ctrl K, ⌘K on macOS): a field, and the groups,
/// voice rooms, friends and chats that match it, the recent ones first. Arrows choose,
/// Enter goes, Esc or a click beside it closes.
class QuickSwitcher extends StatefulWidget {
  const QuickSwitcher({
    super.key,
    required this.recent,
    required this.onGo,
    required this.onClose,
  });

  final List<String> recent;
  final ValueChanged<Place> onGo;
  final VoidCallback onClose;

  @override
  State<QuickSwitcher> createState() => _QuickSwitcherState();
}

class _QuickSwitcherState extends State<QuickSwitcher> {
  final _query = TextEditingController();
  final _scroll = ScrollController();
  int _chosen = 0;
  List<Place> _shown = const [];

  /// The switcher's height when it opened at this window size ([_Centred]).
  double _tallest = 0;
  Size? _tallestFor;

  static const _row = 46.0;

  /// Space above the first row and below the last, inside the list.
  static const _listPad = 6.0;

  @override
  void initState() {
    super.initState();
    _query.addListener(() {
      setState(() => _chosen = 0);
      if (_scroll.hasClients) _scroll.jumpTo(0);
    });
  }

  @override
  void dispose() {
    _query.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _move(int by) {
    if (_shown.isEmpty) return;
    setState(() => _chosen = (_chosen + by) % _shown.length);
    if (!_scroll.hasClients) return;
    final top = _listPad + _chosen * _row;
    // The first and the last row bring the list's padding into view with them.
    final from = _chosen == 0 ? 0.0 : top;
    final to = top + _row + (_chosen == _shown.length - 1 ? _listPad : 0);
    final view = _scroll.position.viewportDimension;
    if (from < _scroll.offset) {
      _scroll.jumpTo(from);
    } else if (to > _scroll.offset + view) {
      _scroll.jumpTo(to - view);
    }
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.escape) {
      widget.onClose();
    } else if (key == LogicalKeyboardKey.arrowDown) {
      _move(1);
    } else if (key == LogicalKeyboardKey.arrowUp) {
      _move(-1);
    } else if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      if (_shown.isNotEmpty) widget.onGo(_shown[_chosen]);
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final size = MediaQuery.sizeOf(context);
    _shown = rankPlaces(
      allPlaces(state),
      _query.text,
      recent: widget.recent,
      shown: placeShown(state),
    ).take(60).toList();
    if (_chosen >= _shown.length) _chosen = math.max(0, _shown.length - 1);
    final width = math.min(560.0, size.width - 32);
    // Room for the list once the field, the hints and a margin above and below are in.
    final listHeight = math.max(
      _row,
      math.min(_row * 8.5, size.height - 2 * _margin - 110),
    );
    return Positioned.fill(
      child: Focus(
        canRequestFocus: false,
        skipTraversal: true,
        onKeyEvent: _key,
        // A scope: Tab stays here, not on the page behind.
        child: FocusScope(
          child: TweenAnimationBuilder<double>(
            tween: Tween(begin: 0, end: 1),
            duration: springTime,
            curve: spring,
            builder: (context, t, child) => GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: widget.onClose,
              child: ColoredBox(
                color: c.scrim.withValues(alpha: c.scrim.a * t.clamp(0, 1)),
                child: child,
              ),
            ),
            child: CustomSingleChildLayout(
              delegate: _Centred(this),
              child: PopIn(
                alignment: Alignment.topCenter,
                child: GestureDetector(
                  // A click inside does not close it.
                  onTap: () {},
                  child: Container(
                    key: const ValueKey('switcher'),
                    width: width,
                    clipBehavior: Clip.antiAlias,
                    // Inside the outline, as when it was drawn under what is inside.
                    padding: const EdgeInsets.all(1),
                    decoration: BoxDecoration(
                      color: c.bg1,
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
                        Padding(
                          padding: const EdgeInsets.all(10),
                          child: FgField(
                            key: const ValueKey('switcher-field'),
                            controller: _query,
                            autofocus: true,
                            height: 42,
                            radius: 999,
                            fontSize: 15,
                            placeholder: l10n.switcherPlaceholder,
                            leading: FgIcon(Ic.search, size: 16, color: c.fg3),
                            label: l10n.switcherGoTo,
                          ),
                        ),
                        Container(height: 1, color: c.border),
                        if (_shown.isEmpty)
                          Padding(
                            padding: const EdgeInsets.all(20),
                            child: Text(
                              _query.text.trim().isEmpty
                                  ? l10n.switcherEmpty
                                  : l10n.switcherNoMatch(_query.text.trim()),
                              style: tx(c.fg3, size: 13),
                            ),
                          )
                        else
                          ConstrainedBox(
                            constraints: BoxConstraints(maxHeight: listHeight),
                            child: ListView.builder(
                              controller: _scroll,
                              shrinkWrap: true,
                              padding: const EdgeInsets.symmetric(
                                vertical: _listPad,
                              ),
                              itemExtent: _row,
                              itemCount: _shown.length,
                              itemBuilder: (context, i) => _Row(
                                place: _shown[i],
                                chosen: i == _chosen,
                                onHover: () {
                                  if (_chosen != i) {
                                    setState(() => _chosen = i);
                                  }
                                },
                                onTap: () => widget.onGo(_shown[i]),
                              ),
                            ),
                          ),
                        Container(height: 1, color: c.border),
                        Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 9,
                          ),
                          child: Wrap(
                            spacing: 16,
                            runSpacing: 4,
                            children: [
                              _Hint(keys: '↑ ↓', text: l10n.switcherHintChoose),
                              _Hint(keys: 'Enter', text: l10n.switcherHintGo),
                              _Hint(keys: 'Esc', text: l10n.switcherHintClose),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Puts the switcher in the middle of the window, as tall as it opened: typing makes its
/// list shorter from the bottom, and the field stays where it was.
class _Centred extends SingleChildLayoutDelegate {
  _Centred(this.state);
  final _QuickSwitcherState state;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size childSize) {
    if (state._tallestFor != size || childSize.height > state._tallest) {
      state._tallestFor = size;
      state._tallest = childSize.height;
    }
    final top = math.max(_margin, (size.height - state._tallest) / 2);
    return Offset((size.width - childSize.width) / 2, top);
  }

  @override
  bool shouldRelayout(_Centred old) => true;
}

/// The least room above and below the switcher: the title bar stays clear of it.
const _margin = 56.0;

class _Hint extends StatelessWidget {
  const _Hint({required this.keys, required this.text});
  final String keys;
  final String text;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(text: keys, style: tx(c.fg2, size: 12, monospace: true)),
          TextSpan(text: ' $text'),
        ],
      ),
      style: tx(c.fg3, size: 12),
    );
  }
}

/// One place of the list: its picture or icon, its name, what it is, and the unread
/// messages of a chat. The chosen one sits on a soft pill.
class _Row extends StatelessWidget {
  const _Row({
    required this.place,
    required this.chosen,
    required this.onHover,
    required this.onTap,
  });

  final Place place;
  final bool chosen;
  final VoidCallback onHover;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final Widget leading;
    switch (place.kind) {
      case PlaceKind.chat:
        final person = _person(state, place.id);
        leading = person == null
            ? FgIcon(Ic.user, size: 16, color: c.fg2)
            : Avatar(avatar: person.avatar, size: 26);
      case PlaceKind.group:
        final g = state.contacts.group(place.id);
        leading = GroupBadge(
          icon: g?.icon ?? 0,
          picture: g?.picture,
          size: 26,
          radius: 8,
          iconSize: 14,
        );
      case PlaceKind.room:
        leading = Container(
          width: 26,
          height: 26,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: place.live ? c.greenSoft : c.hover,
            shape: BoxShape.circle,
          ),
          child: FgIcon(
            place.live ? Ic.mic : Ic.micOff,
            size: 13,
            color: place.live ? c.green : c.fg2,
          ),
        );
    }
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => onHover(),
      child: GestureDetector(
        onTap: onTap,
        child: Semantics(
          button: true,
          selected: chosen,
          label: '${place.title}, ${place.detail}',
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            margin: const EdgeInsets.symmetric(horizontal: 6),
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              color: chosen ? c.hover : null,
              borderRadius: BorderRadius.circular(999),
            ),
            child: Row(
              children: [
                SizedBox(width: 28, child: Center(child: leading)),
                const SizedBox(width: 12),
                Expanded(
                  child: Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(
                          text: place.title,
                          style: tx(c.fg, weight: w500),
                        ),
                        TextSpan(
                          text: '   ${place.detail}',
                          style: tx(c.fg3, size: 12),
                        ),
                      ],
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (place.unread > 0) ...[
                  const SizedBox(width: 8),
                  CountBadge(place.unread),
                ],
                AnimatedOpacity(
                  duration: const Duration(milliseconds: 120),
                  opacity: chosen ? 1 : 0,
                  child: Padding(
                    padding: const EdgeInsets.only(left: 8),
                    child: FgIcon(Ic.arrowRight, size: 14, color: c.fg2),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  static UiPerson? _person(AppState state, String id) {
    for (final chat in state.direct.chats) {
      if (chat.friend.id == id) return chat.friend;
    }
    for (final f in state.contacts.friends) {
      if (f.person.id == id) return f.person;
    }
    return null;
  }
}

/// Opens the quick switcher: a round button, its shortcut in its tooltip.
class FindButton extends StatelessWidget {
  const FindButton({super.key, required this.onTap});
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final shortcut = onMacOS ? '⌘K' : 'Ctrl K';
    return Springy(
      key: const ValueKey('find'),
      onTap: onTap,
      tooltip: l10n.switcherTooltip(shortcut),
      label: l10n.switcherGoTo,
      builder: (context, hovered) => Container(
        width: 28,
        height: 28,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? c.hover : null,
          shape: BoxShape.circle,
          border: Border.all(color: c.border),
        ),
        child: FgIcon(Ic.search, size: 14, color: hovered ? c.fg : c.fg2),
      ),
    );
  }
}
