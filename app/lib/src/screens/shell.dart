import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../devices.dart';
import '../direct_state.dart';
import '../format.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../quality.dart';
import '../reveal.dart';
import '../room_count.dart';
import '../rust/api/types.dart';
import '../theme.dart';
import '../update.dart';
import '../volume.dart';
import '../widgets.dart';
import '../window.dart';
import 'add_to_call.dart' show maxInCall;
import 'bell.dart';
import 'call_layers.dart';
import 'direct.dart';
import 'friend_card.dart';
import 'friends.dart';
import 'group_banner.dart';
import 'group_settings.dart';
import 'home.dart';
import 'members.dart';
import 'new_group.dart';
import 'pip.dart';
import 'room.dart';
import 'settings.dart';
import 'stopped.dart';
import 'switcher.dart';
import 'user_panel.dart';

/// The window once there is a profile: the header (which is the title bar), a rail with
/// the groups as icons, a column for what is selected, the page, and the toasts.
class Shell extends StatefulWidget {
  const Shell({super.key});

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  /// The quick switcher is open.
  bool _switcher = false;

  /// The places shown lately, newest first, for the quick switcher.
  final _recent = <String>[];

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_onKey);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    super.dispose();
  }

  /// What had the focus before the quick switcher opened, to give it back on Esc.
  FocusNode? _focusBefore;

  void _toggleSwitcher() {
    if (_switcher) {
      _closeSwitcher();
      return;
    }
    _focusBefore = FocusManager.instance.primaryFocus;
    setState(() => _switcher = true);
  }

  void _closeSwitcher() {
    setState(() => _switcher = false);
    final before = _focusBefore;
    _focusBefore = null;
    if (before != null && before.context != null) before.requestFocus();
  }

  Future<void> _go(Place place) async {
    setState(() => _switcher = false);
    _focusBefore = null;
    await goToPlace(context, place);
  }

  /// The window's shortcuts: heard before any focus, so they also work when nothing has
  /// it yet.
  bool _onKey(KeyEvent event) {
    if (event is! KeyDownEvent || !mounted) return false;
    final keys = HardwareKeyboard.instance;
    if (keys.isAltPressed) return false;
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.keyK &&
        (keys.isControlPressed || keys.isMetaPressed) &&
        !keys.isShiftPressed) {
      // Not while a dialog is over the window.
      if (!(ModalRoute.of(context)?.isCurrent ?? true)) return false;
      _toggleSwitcher();
      return true;
    }
    // F6 goes to what floats over the window, as it goes between parts of a window
    // elsewhere: a call that rang while a message was being written.
    if (key == LogicalKeyboardKey.f6 &&
        !keys.isControlPressed &&
        !keys.isMetaPressed &&
        !keys.isShiftPressed) {
      return FocusLayer.focusTop();
    }
    // Ctrl Shift M, ⌘⇧M on macOS.
    if (key == LogicalKeyboardKey.keyM &&
        (onMacOS ? keys.isMetaPressed : keys.isControlPressed) &&
        keys.isShiftPressed) {
      final state = AppScope.read(context);
      state.setMic(!(state.callState.call?.mic ?? state.prefs.joinWithMic));
      return true;
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    // The room shown and the chat open change only with the page.
    final state = AppScope.only(context, {Navigation, Notices, CallState});
    final c = context.fg;
    if (placeShown(state) case final here? when _recent.firstOrNull != here) {
      _recent
        ..remove(here)
        ..insert(0, here);
      if (_recent.length > 30) _recent.removeLast();
    }
    final page = switch (state.nav.page) {
      Screen.home => const Home(),
      Screen.friends => const Friends(),
      Screen.room => const RoomPage(),
      Screen.members => const MembersPage(),
      Screen.direct => const DirectPage(),
    };
    return Stack(
      key: shellLayerKey,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Tab goes through the header, the rail, the column and the page one
            // after the other, rather than back and forth between them by height.
            FocusTraversalGroup(child: _Header(onFind: _toggleSwitcher)),
            StoppedBanner(stopping: state.stopping),
            Expanded(
              child: LayoutBuilder(
                builder: (context, box) => Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (box.maxWidth >= 860)
                      FocusTraversalGroup(child: const _Side())
                    else
                      FocusTraversalGroup(child: const Rail()),
                    Expanded(
                      child: FocusTraversalGroup(
                        child: ColoredBox(color: c.bg, child: page),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
        const CornerCards(),
        if (state.nav.userPanelFrom != null) const UserPanelLayer(),
        if (state.nav.friendCard != null) const FriendCardLayer(),
        if (state.nav.settingsOpen) const _SettingsLayer(),
        if (state.nav.groupSettings != null) const GroupSettingsLayer(),
        // Drawn over the layers under it, as a window of its own, but not read with
        // them: a screen reader in one of them hears that layer only.
        PipLayer(
          key: const ValueKey('pip-layer'),
          unread:
              state.nav.userPanelFrom != null ||
              state.nav.friendCard != null ||
              state.nav.settingsOpen ||
              state.nav.groupSettings != null,
        ),
        if (state.notices.bellFrom != null) const BellLayer(),
        if (_switcher)
          QuickSwitcher(recent: _recent, onGo: _go, onClose: _closeSwitcher),
        if (state.callState.incomingCall != null) const IncomingCallLayer(),
      ],
    );
  }
}

/// The settings in a floating window over the app's: the page behind dims, and a click
/// beside it, the close button or Esc closes it.
class _SettingsLayer extends StatelessWidget {
  const _SettingsLayer();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    return Positioned.fill(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): state.closeSettings,
        },
        // A scope: Tab goes round what floats here, not the page behind.
        child: FocusLayer(
          child: TweenAnimationBuilder<double>(
            tween: Tween(begin: 0, end: 1),
            duration: const Duration(milliseconds: 320),
            curve: spring,
            builder: (context, t, child) => Stack(
              children: [
                Positioned.fill(
                  child: Backdrop(
                    onTap: state.closeSettings,
                    color: c.scrim.withValues(alpha: c.scrim.a * t.clamp(0, 1)),
                  ),
                ),
                Opacity(
                  opacity: t.clamp(0, 1),
                  child: Transform.scale(scale: .96 + .04 * t, child: child),
                ),
              ],
            ),
            child: Center(
              child: Padding(
                padding: const EdgeInsets.all(32),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(
                    maxWidth: 1000,
                    maxHeight: 760,
                  ),
                  child: Container(
                    clipBehavior: Clip.antiAlias,
                    // Inside the outline, as when it was drawn under what is inside.
                    padding: const EdgeInsets.all(1),
                    decoration: BoxDecoration(
                      color: c.bg,
                      borderRadius: BorderRadius.circular(22),
                    ),
                    // Over what is inside, which would cover it at the rounded corners.
                    foregroundDecoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(22),
                      border: Border.all(color: c.border),
                    ),
                    child: const Settings(),
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

class _Header extends StatelessWidget {
  const _Header({required this.onFind});
  final VoidCallback onFind;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final roomGroup = state.nav.page == Screen.room
        ? state.contacts.group(state.roomState.roomGroup)
        : null;
    final wide = MediaQuery.sizeOf(context).width >= 1180;
    // How the room or call is reached, in a chip on the right of a wide bar.
    final chip = wide && (state.inCallPage || roomGroup != null);
    // Long names are shortened and the chips on the right shrink, so the bell and the
    // window's buttons always fit.
    final aside = <Widget>[
      if (chip && state.inCallPage) ...[
        Flexible(
          child: IgnorePointer(
            child: _HeaderChip(
              icon: Ic.lock,
              text:
                  (state.callState
                              .callIn(state.roomState.roomGroup)
                              ?.members
                              .length ??
                          1) >
                      1
                  ? l10n.shellChipCallMany
                  : l10n.shellChipCallTwo,
            ),
          ),
        ),
      ] else if (chip) ...[
        // That the room goes with its last person, its chat says.
        Flexible(
          child: IgnorePointer(
            child: _HeaderChip(icon: Ic.lock, text: pathSummary(state)),
          ),
        ),
      ],
      if (chip) const SizedBox(width: 8),
      // Its full width first: the names shorten for it.
      UpdatePill(updates: state.updates, say: state.say),
      FindButton(onTap: onFind),
    ];
    final voiceName = roomGroup == null
        ? ''
        : Contacts.voiceName(
            state.contacts.voiceRoom(roomGroup.id, state.roomState.roomVoice),
          );
    final names = roomGroup == null
        ? (1, 1)
        : _shares(
            context,
            roomGroup.name,
            tx(c.fg, weight: w500),
            voiceName,
            tx(c.fg2, weight: w500),
          );
    return TitleBar(
      children: [
        Expanded(
          flex: 3,
          child: Row(
            children: [
              Pressable(
                onTap: state.goHome,
                label: l10n.commonHome,
                builder: (context, hovered) => Row(
                  children: [
                    Image.asset(
                      'assets/images/logo.png',
                      width: 18,
                      height: 18,
                      cacheWidth: (18 * MediaQuery.devicePixelRatioOf(context))
                          .round(),
                    ),
                    const SizedBox(width: 8),
                    Text('Fogata', style: wordmark(c.fg)),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              IgnorePointer(
                child: Text('/', style: tx(c.borderStrong, size: 16)),
              ),
              const SizedBox(width: 8),
              if (state.inCallPage) ...[
                Flexible(
                  child: IgnorePointer(child: _CallTitle(state: state)),
                ),
              ] else if (roomGroup != null) ...[
                IgnorePointer(
                  child: GroupBadge(
                    icon: roomGroup.icon,
                    picture: roomGroup.picture,
                    size: 20,
                  ),
                ),
                const SizedBox(width: 8),
                Flexible(
                  flex: names.$1,
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: _groupNameMax),
                    child: Pressable(
                      onTap: () => state.showMembers(roomGroup.id),
                      tooltip: l10n.shellGroupMembers,
                      builder: (context, hovered) => Text(
                        roomGroup.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: tx(
                          c.fg,
                          weight: w500,
                          decoration: hovered ? TextDecoration.underline : null,
                        ),
                      ),
                    ),
                  ),
                ),
                if (roomGroup.voiceRooms.length > 1) ...[
                  const SizedBox(width: 8),
                  IgnorePointer(
                    child: Text('/', style: tx(c.borderStrong, size: 16)),
                  ),
                  const SizedBox(width: 8),
                  Flexible(
                    flex: names.$2,
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(
                        maxWidth: _voiceNameMax,
                      ),
                      child: IgnorePointer(
                        child: Text(
                          voiceName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tx(c.fg2, weight: w500),
                        ),
                      ),
                    ),
                  ),
                ],
                const SizedBox(width: 8),
                const IgnorePointer(child: RoomStatusPill()),
                const SizedBox(width: 6),
                IgnorePointer(
                  child: RoomCount(
                    count: peopleInRoom(
                      state.contacts.voiceRoom(
                        roomGroup.id,
                        state.roomState.roomVoice,
                      ),
                    ),
                    max: roomGroup.maxInRoom,
                    pill: true,
                  ),
                ),
              ] else
                Flexible(
                  child: IgnorePointer(
                    child: Text(
                      switch (state.nav.page) {
                        Screen.friends => l10n.commonFriends,
                        Screen.members =>
                          state.contacts.group(state.nav.membersGroup)?.name ??
                              l10n.shellMembers,
                        Screen.direct =>
                          state.direct.page?.friend.name ??
                              l10n.shellDirectMessages,
                        _ => l10n.commonHome,
                      },
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg, weight: w500),
                    ),
                  ),
                ),
            ],
          ),
        ),
        // The chip shares the bar with the names; the round button alone takes only its
        // own room, which leaves the rest to the names.
        if (chip)
          Expanded(
            flex: 2,
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: aside,
            ),
          )
        else ...[
          const SizedBox(width: 8),
          ...aside,
        ],
        const SizedBox(width: 8),
        const BellButton(),
        const SizedBox(width: 8),
      ],
    );
  }
}

/// The widest the group's and the voice room's names get in the title bar.
const _groupNameMax = 320.0;
const _voiceNameMax = 200.0;

/// How the room left beside the title bar's other parts is shared between the group's
/// name and the voice room's: by how wide each is, so both show whole when they fit and
/// shrink alike when they do not.
(int, int) _shares(
  BuildContext context,
  String group,
  TextStyle groupStyle,
  String voice,
  TextStyle voiceStyle,
) {
  final scaler = MediaQuery.textScalerOf(context);
  double width(String text, TextStyle style, double most) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: TextDirection.ltr,
      textScaler: scaler,
      maxLines: 1,
    )..layout();
    final w = painter.width;
    painter.dispose();
    return w.clamp(1.0, most);
  }

  return (
    width(group, groupStyle, _groupNameMax).ceil(),
    width(voice, voiceStyle, _voiceNameMax).ceil(),
  );
}

/// How the room's members are reached, in two words: the member who carries the others
/// is named when they all come through the same known one.
String pathSummary(AppState state) {
  if (!throughMember(state)) return l10n.shellPathDirect;
  final group = state.roomState.roomGroup;
  final carried = (state.contacts.paths[group] ?? const <String, UiPath>{})
      .entries
      .where((p) => p.value == UiPath.throughMember)
      .map((p) => state.contacts.carrierName(group, p.key))
      .toSet();
  final only = carried.length == 1 ? carried.single : null;
  return only == null ? l10n.shellPathViaSomeone : l10n.shellPathVia(only);
}

/// Whether someone in the room is reached through another member rather than directly.
bool throughMember(AppState state) =>
    (state.contacts.paths[state.roomState.roomGroup]?.values ??
            const <UiPath>[])
        .any((p) => p == UiPath.throughMember);

/// The call in the title bar: with whom, and how long it has rung or lasted.
class _CallTitle extends StatelessWidget {
  const _CallTitle({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final call = state.callState.callIn(state.roomState.roomGroup);
    final person = call?.with_ ?? state.callState.callEnding?.person;
    if (person == null) return const SizedBox.shrink();
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Avatar(avatar: person.avatar, size: 20),
        const SizedBox(width: 8),
        Flexible(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 260),
            child: Text(
              l10n.shellCallWith(
                call == null ? person.name : state.callNames(call),
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tx(c.fg, weight: w500),
            ),
          ),
        ),
        const SizedBox(width: 8),
        Ticker(
          builder: (context) => switch (call?.phase) {
            UiCallPhase.ringing => Pill(
              text: l10n.shellRinging,
              monoTail: elapsed(call!.sinceMs),
              color: c.amber,
              background: c.amberSoft,
              leading: Dot(c.amber),
            ),
            UiCallPhase.connected => Pill(
              text: l10n.shellInCall,
              monoTail: elapsed(call!.sinceMs),
              color: c.green,
              background: c.greenSoft,
              leading: Dot(c.green),
            ),
            _ => Pill(
              text: l10n.shellCallEnded,
              color: c.fg2,
              background: c.hover,
            ),
          },
        ),
        // A call of more than two: how many are in it, of how many it takes.
        if (call != null &&
            call.members.length > 1 &&
            state.roomState.roomGroup == call.groupId &&
            state.roomState.room != null) ...[
          const SizedBox(width: 6),
          Pill(
            text: '',
            monoTail: '${state.roomState.room!.participants.length}/$maxInCall',
            color: c.fg2,
            background: c.hover,
          ),
        ],
      ],
    );
  }
}

class _HeaderChip extends StatelessWidget {
  const _HeaderChip({required this.icon, required this.text});
  final Ic icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      constraints: const BoxConstraints(minHeight: 24),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 2),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          FgIcon(icon, size: 13, color: c.fg2),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tx(c.fg2, size: 12, height: 1.2),
            ),
          ),
        ],
      ),
    );
  }
}

/// The rail: home on top, then every group as an icon with a badge while its room is
/// live, a button for a new group, and the settings at the bottom when the person's pill
/// is not beside it.
class Rail extends StatelessWidget {
  const Rail({super.key, this.settings = true});

  /// The settings button at the bottom; beside the column it is in the person's pill.
  final bool settings;

  static const width = 72.0;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, {Navigation, Contacts, DirectChats});
    final c = context.fg;
    final homeSelected = state.nav.selectedGroup == null;
    return Container(
      width: width,
      padding: const EdgeInsets.symmetric(vertical: 12),
      decoration: BoxDecoration(
        color: c.bg1,
        border: Border(right: BorderSide(color: c.border)),
      ),
      child: Column(
        children: [
          _RailItem(
            selected: homeSelected,
            tooltip: l10n.commonHome,
            onTap: state.goHome,
            badge:
                state.contacts.pendingRequests +
                state.contacts.invitations.length +
                state.direct.unread,
            badgeColor: c.blueSolid,
            child: FgIcon(
              Ic.users,
              size: 20,
              color: homeSelected ? c.fg : c.fg2,
            ),
          ),
          Container(
            width: 32,
            height: 1,
            color: c.border,
            margin: const EdgeInsets.symmetric(vertical: 6),
          ),
          Expanded(
            child: ListView(
              padding: EdgeInsets.zero,
              children: [
                for (final g in state.contacts.groups)
                  _RailItem(
                    selected: state.nav.selectedGroup == g.id,
                    tooltip: g.name,
                    onTap: () => state.selectGroup(g.id),
                    badge: Contacts.peopleIn(g).length,
                    badgeColor: c.green,
                    child: g.picture != null
                        ? RoundedPicture(
                            bytes: g.picture!,
                            size: 46,
                            radius: 23,
                          )
                        : FgIcon(
                            groupIcon(g.icon),
                            size: 20,
                            color: state.nav.selectedGroup == g.id
                                ? c.fg
                                : c.fg2,
                          ),
                  ),
                _RailItem(
                  selected: false,
                  tooltip: l10n.shellNewGroup,
                  dashed: true,
                  onTap: () => showNewGroup(context),
                  child: FgIcon(Ic.plus, size: 18, color: c.fg2),
                ),
              ],
            ),
          ),
          if (settings)
            _RailItem(
              selected: state.nav.settingsOpen,
              tooltip: l10n.commonSettings,
              plain: true,
              onTap: () => state.openSettings(),
              child: FgIcon(
                Ic.settings,
                size: 20,
                color: state.nav.settingsOpen ? c.fg : c.fg2,
              ),
            ),
        ],
      ),
    );
  }
}

class _RailItem extends StatelessWidget {
  const _RailItem({
    required this.selected,
    required this.tooltip,
    required this.onTap,
    required this.child,
    this.badge = 0,
    this.badgeColor,
    this.dashed = false,
    this.plain = false,
  });

  final bool selected;
  final String tooltip;
  final VoidCallback onTap;
  final Widget child;
  final int badge;
  final Color? badgeColor;
  final bool dashed;

  /// No tile behind the icon (the settings button).
  final bool plain;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    // As wide as the rail, so the selection mark sits at its edge, clear of the tile.
    return SizedBox(
      width: double.infinity,
      height: 56,
      child: Stack(
        alignment: Alignment.center,
        children: [
          if (selected)
            Positioned(
              left: 0,
              child: Container(
                width: 4,
                height: 32,
                decoration: BoxDecoration(
                  color: c.fg,
                  borderRadius: const BorderRadius.horizontal(
                    right: Radius.circular(4),
                  ),
                ),
              ),
            ),
          Springy(
            onTap: onTap,
            tooltip: tooltip,
            label: tooltip,
            focusRadius: 24,
            hoverScale: 1.06,
            builder: (context, hovered) => AnimatedContainer(
              duration: const Duration(milliseconds: 160),
              width: 48,
              height: 48,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: plain
                    ? (hovered ? c.hover : null)
                    : (selected || hovered ? c.hover : c.bg2),
                shape: BoxShape.circle,
                border: plain
                    ? null
                    : Border.all(
                        color: selected ? c.borderStrong : c.border,
                        style: dashed ? BorderStyle.none : BorderStyle.solid,
                      ),
              ),
              child: dashed
                  ? Dashed(radius: 24, child: Center(child: child))
                  : child,
            ),
          ),
          if (badge > 0)
            Positioned(
              right: 8,
              bottom: 2,
              child: Container(
                height: heightForText(context, 18, 10),
                constraints: BoxConstraints(
                  minWidth: heightForText(context, 18, 10),
                ),
                padding: const EdgeInsets.symmetric(horizontal: 5),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: badgeColor ?? c.green,
                  borderRadius: BorderRadius.circular(999),
                  border: Border.all(color: c.bg1, width: 2),
                ),
                child: Text(
                  '$badge',
                  style: tx(
                    badgeColor == c.blueSolid ? Colors.white : c.onPri,
                    size: 10,
                    weight: w600,
                    height: 1,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The column beside the rail: for home, the pages; for a group, its members and
/// invitations, then its voice rooms.
class SideColumn extends StatefulWidget {
  const SideColumn({super.key});

  static const width = 240.0;

  @override
  State<SideColumn> createState() => _SideColumnState();
}

class _SideColumnState extends State<SideColumn> {
  final _list = GlobalKey();
  final _active = GlobalKey();

  /// Where the active item's highlight is, inside the list; it slides there.
  Rect? _highlight;

  /// Whether [_measure] runs after the frame being built: once per frame, however many
  /// builds it has.
  bool _measurePending = false;

  void _measure() {
    if (!mounted) return;
    final list = _list.currentContext?.findRenderObject() as RenderBox?;
    final item = _active.currentContext?.findRenderObject() as RenderBox?;
    Rect? rect;
    if (list != null && item != null && list.hasSize && item.hasSize) {
      rect = item.localToGlobal(Offset.zero, ancestor: list) & item.size;
    }
    if (rect != _highlight) setState(() => _highlight = rect);
  }

  /// Measures where the active item is once the frame is built.
  void _measureSoon() {
    if (_measurePending) return;
    _measurePending = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _measurePending = false;
      _measure();
    });
  }

  @override
  Widget build(BuildContext context) {
    // Not the room's chat and files: only a direct call's items show the room, and they
    // listen to it themselves ([_RoomWatch]). The room shown changes with the page.
    final state = AppScope.only(context, {
      Navigation,
      Contacts,
      CallState,
      Prefs,
      Notices,
    });
    final c = context.fg;
    final group = state.contacts.group(state.nav.selectedGroup);
    final call = state.inCallPage
        ? state.callState.callIn(state.roomState.roomGroup)
        : null;
    _measureSoon();
    final highlight = _highlight;
    return Container(
      width: SideColumn.width,
      decoration: BoxDecoration(
        color: c.bg1,
        border: Border(right: BorderSide(color: c.border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (call != null)
            _RoomWatch(
              builder: (context) => ColumnHeader(
                title: l10n.shellCallWith(state.callNames(call)),
              ),
            )
          // Home has no header: its items start at the top.
          else if (group != null)
            ColumnHeader(
              title: group.name,
              banner: state.contacts.groupBanner(group),
              onSettings: () => state.openGroupSettings(group.id),
            ),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 12),
              child: Stack(
                key: _list,
                children: [
                  if (highlight != null)
                    AnimatedPositioned(
                      duration: springTime,
                      curve: spring,
                      left: highlight.left,
                      top: highlight.top,
                      width: highlight.width,
                      height: highlight.height,
                      child: Container(
                        decoration: BoxDecoration(
                          color: c.hover,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(color: c.border),
                        ),
                      ),
                    ),
                  if (call != null)
                    _RoomWatch(
                      builder: (context) {
                        // People coming and going move the active item.
                        _measureSoon();
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: _callItems(state, call),
                        );
                      },
                    )
                  else
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: group == null
                          ? _homeItems(state, c)
                          : _groupItems(context, state, group, c),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// The active item gets [_active], so the highlight can find it.
  Key? _key(bool active) => active ? _active : null;

  List<Widget> _homeItems(AppState state, Fg c) => [
    if (state.callState.activeCall case final call?) ...[
      _Section(l10n.shellSectionCall),
      _Item(
        icon: Ic.phone,
        text: call.phase == UiCallPhase.ringing
            ? l10n.shellCallRinging(call.with_.name)
            : call.with_.name,
        live: call.phase == UiCallPhase.connected ? 2 : 0,
        mine: true,
        onTap: state.showCall,
      ),
      const SizedBox(height: 8),
    ],
    _Item(
      key: _key(state.nav.page == Screen.home),
      icon: Ic.grid,
      text: l10n.commonHome,
      selected: state.nav.page == Screen.home,
      onTap: () => state.go(Screen.home),
    ),
    _Item(
      key: _key(state.nav.page == Screen.friends),
      icon: Ic.users,
      text: l10n.commonFriends,
      selected: state.nav.page == Screen.friends,
      badge:
          state.contacts.pendingRequests + (state.notices.missedUnseen ? 1 : 0),
      onTap: () => state.go(Screen.friends),
    ),
    const DirectList(),
    if (state.contacts.groups.any((g) => g.room != null)) ...[
      _Section(l10n.shellSectionLiveNow),
      for (final g in state.contacts.groups)
        if (g.room != null)
          _Item(
            icon: groupIcon(g.icon, small: true),
            text: g.name,
            live: Contacts.peopleIn(g).length,
            mine: g.room!.joined,
            onTap: () => state.selectGroup(g.id),
          ),
    ],
  ];

  /// Someone in the room shown, with what they do.
  Widget _participant(
    AppState state,
    UiPerson p, {
    String? group,
    String? voice,
  }) {
    // What the call tells (mute, share, camera) builds only this row again.
    return LiveWatch(
      builder: (context) =>
          _participantNow(state, p, group: group, voice: voice),
    );
  }

  Widget _participantNow(
    AppState state,
    UiPerson p, {
    String? group,
    String? voice,
  }) {
    final me = state.contacts.profile?.person.id;
    final peer = state.callState.call?.peers[p.id];
    return Talking(
      call: state.callState.call,
      builder: (context) => _Person(
        person: p,
        group: group,
        voice: voice,
        me: p.id == me,
        volume: state.prefs.volumeOf(p.id),
        silenced: state.prefs.silenced.contains(p.id),
        speaking:
            peer?.speaking ??
            (p.id == me && (state.callState.call?.speaking ?? false)),
        muted: p.id == me
            ? !(state.callState.call?.mic ?? state.prefs.joinWithMic)
            : (peer?.connected ?? false) && !(peer?.mic ?? true),
        live: p.id == me
            ? state.callState.call?.sharing ?? false
            : peer?.sharing ?? false,
        camera: p.id == me
            ? state.callState.call?.camera ?? false
            : peer?.cam ?? false,
        watching: p.id == me
            ? state.callState.call?.watching != null
            : peer?.watches != null,
      ),
    );
  }

  /// A direct call: who is in it, its chat and files, and the way home.
  List<Widget> _callItems(AppState state, UiCall call) {
    final group = call.groupId;
    return [
      _Section(l10n.shellSectionInCall),
      Padding(
        padding: const EdgeInsets.only(left: 10, top: 2, bottom: 6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final p
                in state.roomState.room?.participants ??
                    const <UiParticipant>[])
              _participant(state, p.person),
          ],
        ),
      ),
      _Item(
        key: _key(state.roomState.roomTab == 0),
        icon: Ic.chat,
        text: l10n.shellChat,
        selected: state.roomState.roomTab == 0,
        onTap: () => state.showRoomTab(group, 0),
      ),
      _Item(
        key: _key(state.roomState.roomTab == 1),
        icon: Ic.paperclip,
        text: l10n.shellFiles,
        selected: state.roomState.roomTab == 1,
        badge: state.roomState.room?.files.length ?? 0,
        onTap: () => state.showRoomTab(group, 1),
      ),
      const SizedBox(height: 12),
      _Item(
        icon: Ic.grid,
        text: l10n.commonHome,
        onTap: () => state.go(Screen.home),
      ),
      _Item(
        icon: Ic.users,
        text: l10n.commonFriends,
        badge:
            state.contacts.pendingRequests +
            (state.notices.missedUnseen ? 1 : 0),
        onTap: () => state.go(Screen.friends),
      ),
    ];
  }

  List<Widget> _groupItems(
    BuildContext context,
    AppState state,
    UiGroup group,
    Fg c,
  ) {
    final joined = state.contacts.joinedVoice(group.id);
    final shown =
        state.nav.page == Screen.room && state.roomState.roomGroup == group.id;
    // The room's chat and files are in the panel beside it, with its own tabs, and the
    // call's controls show or hide that panel: they are not listed here too.
    // The words around the name of the room this device is in, which may be cut short.
    final youAreIn = l10n.shellVoiceRoomYouAreIn('\u0000').split('\u0000');
    // The group's own items first, the voice rooms under them.
    return [
      _Section(l10n.shellSectionGroup),
      _Item(
        key: _key(state.nav.page == Screen.members),
        icon: Ic.users,
        text: l10n.shellMembers,
        selected: state.nav.page == Screen.members,
        count: group.members,
        onTap: () => state.showMembers(group.id),
      ),
      if (group.manager)
        _Item(
          icon: Ic.link,
          text: l10n.shellInvite,
          onTap: () => state.openGroupSettings(group.id, tab: 3),
        ),
      _Section(l10n.shellSectionVoiceRooms),
      for (final v in group.voiceRooms) ...[
        _Item(
          key: _key(shown && state.roomState.roomVoice == v.id),
          icon: switch (v.barrier) {
            UiBarrier(kind: UiBarrierKind.timedOut) => Ic.clock,
            _? => Ic.lock,
            null => v.room == null ? Ic.micOff : Ic.mic,
          },
          text: Contacts.voiceName(v),
          title: true,
          before: joined == v.id ? youAreIn.first : '',
          after: joined == v.id ? youAreIn.last : '',
          selected: shown && state.roomState.roomVoice == v.id,
          live: peopleInRoom(v),
          room: group.maxInRoom,
          mine: joined == v.id,
          onlySome: v.only != null,
          barred: v.barrier == null
              ? null
              : switch (v.barrier!.untilMs) {
                  final until? when v.barrier!.kind == UiBarrierKind.timedOut =>
                    timeLeft(until - DateTime.now().millisecondsSinceEpoch),
                  _ => notForYou,
                },
          onTap: () => openRoomChecked(context, group.id, voice: v.id),
        ),
        if (v.room case final room?)
          Padding(
            padding: const EdgeInsets.only(left: 26, top: 2, bottom: 6),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final p in room.participants)
                  _participant(state, p, group: group.id, voice: v.id),
              ],
            ),
          ),
      ],
    ];
  }
}

/// Builds again when the room shown changes (its people, chat and files), besides when
/// what holds it does.
class _RoomWatch extends StatelessWidget {
  const _RoomWatch({required this.builder});
  final WidgetBuilder builder;

  @override
  Widget build(BuildContext context) {
    AppScope.watch<RoomState>(context);
    return builder(context);
  }
}

class _Section extends StatelessWidget {
  const _Section(this.text);
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(10, 12, 10, 4),
    // A long voice room's name is cut short on one line.
    child: Text(
      text.toUpperCase(),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tx(context.fg.fg3, size: 11, weight: w500, spacing: .3),
    ),
  );
}

/// What a voice room this device may not enter says instead of its count.
String get notForYou => l10n.shellNotForYou;

class _Item extends StatelessWidget {
  const _Item({
    super.key,
    required this.icon,
    required this.text,
    required this.onTap,
    this.title = false,
    this.selected = false,
    this.badge = 0,
    this.count,
    this.live = 0,
    this.room,
    this.mine = false,
    this.onlySome = false,
    this.barred,
    this.before = '',
    this.after = '',
  });

  final Ic icon;
  final String text;
  final VoidCallback onTap;

  /// Words around [text] that stay whole while [text] is cut short: " · you are in".
  final String before;
  final String after;

  /// A voice room only some members may enter: a small lock after its name.
  final bool onlySome;

  /// This device may not enter the voice room now: how long until it may, or that it may
  /// not, shown instead of the count.
  final String? barred;
  final bool selected;
  final int badge;
  final int? count;

  /// People in a live room, shown with the pulsing dot.
  final int live;

  /// For a voice room, the most people it takes: [live] shows as "3/8".
  final int? room;

  /// The live room or call is this device's own: its dot keeps still.
  final bool mine;

  /// The text is a heading (a voice room's name): drawn in the display face.
  final bool title;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final style = tx(
      barred != null ? c.fg3 : (selected || live > 0 ? c.fg : c.fg2),
      weight: selected ? w500 : FontWeight.w400,
    );
    // The selected item's background is the side column's sliding highlight.
    return Pressable(
      onTap: onTap,
      focusRadius: 10,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 140),
        // Taller only when the system draws text larger.
        constraints: const BoxConstraints(minHeight: 34),
        margin: const EdgeInsets.only(bottom: 2),
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: hovered && !selected ? c.hover.withValues(alpha: .6) : null,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          children: [
            FgIcon(
              icon,
              size: 16,
              color: barred != null ? c.fg3 : (selected ? c.fg : c.fg2),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: LayoutBuilder(
                builder: (context, box) {
                  // The room's words around its name take up to most of the
                  // width, then are cut short, so the name keeps some room.
                  Widget around(String words) => ConstrainedBox(
                    constraints: BoxConstraints(maxWidth: box.maxWidth * .7),
                    child: Text(
                      words,
                      maxLines: 1,
                      softWrap: false,
                      overflow: TextOverflow.ellipsis,
                      style: style,
                    ),
                  );
                  return Row(
                    children: [
                      if (before.isNotEmpty) around(before),
                      Flexible(
                        child: Text(
                          text,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: title ? hx(style.color!, size: 14) : style,
                        ),
                      ),
                      if (after.isNotEmpty) around(after),
                      if (onlySome && barred == null) ...[
                        const SizedBox(width: 6),
                        Semantics(
                          label: l10n.shellOnlySomeMembers,
                          child: FgIcon(Ic.lock, size: 12, color: c.fg3),
                        ),
                      ],
                    ],
                  );
                },
              ),
            ),
            if (barred case final barred?)
              Text(
                barred,
                style: tx(c.fg3, size: 12, monospace: barred != notForYou),
              )
            // A dot for the room this device is in.
            else if (room case final max?) ...[
              if (live > 0 && mine) ...[Dot(c.green), const SizedBox(width: 6)],
              RoomCount(count: live, max: max, fullWord: !mine),
            ] else if (live > 0) ...[
              if (mine) ...[Dot(c.green), const SizedBox(width: 6)],
              Text('$live', style: tx(c.fg2, size: 12, monospace: true)),
            ] else if (badge > 0)
              CountBadge(badge)
            else if (count != null)
              Text('$count', style: tx(c.fg3, size: 12, monospace: true)),
          ],
        ),
      ),
    );
  }
}

/// A participant of the group's room, under the room item.
class _Person extends StatelessWidget {
  const _Person({
    required this.person,
    this.group,
    this.voice,
    required this.me,
    required this.speaking,
    required this.muted,
    this.live = false,
    this.camera = false,
    this.watching = false,
    this.volume = 100,
    this.silenced = false,
  });

  final UiPerson person;
  final bool me;
  final bool speaking;
  final bool muted;

  /// The group and voice room it is in, for a manager's menu.
  final String? group;
  final String? voice;

  /// Shares the screen, has the camera on, watches someone's screen.
  final bool live;
  final bool camera;
  final bool watching;

  /// How loud this person plays on this device, in percent, and whether not at all.
  final int volume;
  final bool silenced;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final row = ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 30),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6),
        child: Row(
          children: [
            QualityDim(
              id: person.id,
              child: Avatar(
                avatar: person.avatar,
                size: 18,
                ring: speaking ? c.green : null,
                ringGap: c.bg1,
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                me ? l10n.shellPersonYou(person.name) : person.name,
                overflow: TextOverflow.ellipsis,
                style: tx(me ? c.fg : c.fg2, size: 13),
              ),
            ),
            if (silenced) ...[
              FgIcon(Ic.volumeOff, size: 12, color: c.red),
              const SizedBox(width: 6),
            ] else if (volume != 100) ...[
              Text(
                l10n.shellVolumePercent(volume),
                style: tx(c.fg3, size: 11, monospace: true),
              ),
              const SizedBox(width: 6),
            ],
            if (speaking)
              const SpeakingBars(height: 9)
            else if (muted)
              FgIcon(Ic.micOff, size: 12, color: c.fg3),
            if (watching) ...[
              const SizedBox(width: 6),
              FgIcon(Ic.eye, size: 12, color: c.fg3),
            ],
            if (camera) ...[
              const SizedBox(width: 6),
              FgIcon(Ic.camera, size: 12, color: c.blue),
            ],
            if (live) ...[
              const SizedBox(width: 6),
              Container(
                height: heightForText(context, 16, 9),
                padding: const EdgeInsets.symmetric(horizontal: 5),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: c.redSolid,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  l10n.shellLive,
                  style: tx(
                    Colors.white,
                    size: 9,
                    weight: w600,
                    height: 1,
                    spacing: .4,
                  ),
                ),
              ),
            ],
            QualityMark(id: person.id),
          ],
        ),
      ),
    );
    return me
        ? row
        : VolumeTarget(person: person, group: group, voice: voice, child: row);
  }
}

/// The rail and the column, with the person's own pill under both. Tab goes through
/// the rail, the column, then the pill.
class _Side extends StatelessWidget {
  const _Side();

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return SizedBox(
      width: Rail.width + SideColumn.width,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                FocusTraversalGroup(child: const Rail(settings: false)),
                FocusTraversalGroup(child: const SideColumn()),
              ],
            ),
          ),
          FocusTraversalGroup(
            child: Container(
              padding: const EdgeInsets.fromLTRB(8, 4, 8, 8),
              decoration: BoxDecoration(
                color: c.bg1,
                border: Border(right: BorderSide(color: c.border)),
              ),
              child: const _OwnCard(),
            ),
          ),
        ],
      ),
    );
  }
}

/// The person's own pill at the bottom left: name, state, microphone and deafen (each with
/// the arrow of its device's panel, which opens over the pill), settings.
class _OwnCard extends StatelessWidget {
  const _OwnCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final profile = state.contacts.profile;
    if (profile == null) return const SizedBox.shrink();
    final mic = state.callState.call?.mic ?? state.prefs.joinWithMic;
    // While the core is stopped for a move of the account, nobody sees this device.
    final moving = state.offlineForMove;
    final inRoom = moving ? null : state.contacts.joinedGroup;
    final direct = moving ? null : state.callState.activeCall;
    final deafened = state.callState.deafened;
    final offline = state.prefs.appearOffline || moving;
    // The core never says away in a room or a call; the window may know of one first.
    final away =
        state.contacts.away && !offline && inRoom == null && direct == null;
    final open = state.nav.userPanelFrom != null;
    return Builder(
      // The person's panel and the device panels open above the whole card.
      builder: (card) => AnimatedContainer(
        key: const ValueKey('own-pill'),
        duration: const Duration(milliseconds: 200),
        padding: const EdgeInsets.fromLTRB(4, 4, 6, 4),
        decoration: BoxDecoration(
          color: open ? c.hover : c.bg2,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(color: c.border),
        ),
        child: Row(
          children: [
            Expanded(
              child: Pressable(
                onTap: () => toggleUserPanel(card),
                label: l10n.shellYourProfile,
                focusRadius: 999,
                builder: (context, hovered) => AnimatedContainer(
                  duration: const Duration(milliseconds: 160),
                  padding: const EdgeInsets.fromLTRB(2, 2, 6, 2),
                  decoration: BoxDecoration(
                    color: hovered && !open ? c.hover : Colors.transparent,
                    borderRadius: BorderRadius.circular(999),
                  ),
                  child: Row(
                    children: [
                      SizedBox(
                        width: 32,
                        height: 32,
                        child: Stack(
                          clipBehavior: Clip.none,
                          children: [
                            Avatar(avatar: profile.person.avatar, size: 32),
                            Positioned(
                              right: -1,
                              bottom: -1,
                              child: StatusDot(
                                offline: offline,
                                away: away,
                                size: 10,
                                ring: 2.5,
                                ringColor: open ? c.hover : c.bg2,
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              profile.person.name,
                              overflow: TextOverflow.ellipsis,
                              style: tx(c.fg, weight: w500, height: 1.2),
                            ),
                            _OwnState(
                              state: state,
                              direct: direct,
                              inRoom: inRoom != null,
                              offline: offline,
                              away: away,
                              card: card,
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            FgButton(
              icon: mic ? Ic.mic : Ic.micOff,
              kind: Kind.ghost,
              height: 32,
              iconColor: mic ? c.fg : c.red,
              tooltip: mic
                  ? l10n.shellMuteMic(muteKeys)
                  : l10n.shellUnmuteMic(muteKeys),
              onTap: () => state.setMic(!mic),
            ),
            DeviceArrow(
              kind: DeviceKind.input,
              anchor: card,
              width: 16,
              height: 32,
              size: 11,
            ),
            FgButton(
              icon: deafened ? Ic.headphonesOff : Ic.headphones,
              kind: Kind.ghost,
              height: 32,
              iconColor: deafened ? c.red : c.fg,
              tooltip: deafened ? l10n.shellUndeafen : l10n.shellDeafen,
              onTap: () => state.setDeafened(!deafened),
            ),
            DeviceArrow(
              kind: DeviceKind.output,
              anchor: card,
              width: 16,
              height: 32,
              size: 11,
            ),
            FgButton(
              icon: Ic.settings,
              kind: Kind.ghost,
              height: 32,
              iconColor: c.fg,
              tooltip: l10n.commonSettings,
              onTap: () => state.openSettings(),
            ),
          ],
        ),
      ),
    );
  }
}

/// What the person is doing, under their name: the words always whole, a call's other
/// person shortened if need be, and how the room is reached as an icon whose tooltip
/// says it.
class _OwnState extends StatelessWidget {
  const _OwnState({
    required this.state,
    required this.direct,
    required this.inRoom,
    required this.offline,
    required this.away,
    required this.card,
  });
  final AppState state;
  final UiCall? direct;
  final bool inRoom;
  final bool offline;
  final bool away;

  /// The card, which the details of the connection open above.
  final BuildContext card;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final direct = this.direct;
    final ringing = direct?.phase == UiCallPhase.ringing;
    final words = direct != null
        ? (ringing ? l10n.shellStateCalling : l10n.shellStateInCall)
        : inRoom
        ? l10n.shellStateInRoom
        : offline
        ? l10n.shellStateOffline
        : away
        ? l10n.shellStateAway
        : l10n.shellStateOnline;
    final color = direct != null || inRoom
        ? c.green
        : offline
        ? c.fg3
        : away
        ? c.amber
        : c.green;
    final style = tx(color, size: 11, height: 1.3);
    final connected = inRoom || (direct != null && !ringing);
    final row = Row(
      children: [
        Flexible(
          flex: 2,
          child: Text(
            words,
            maxLines: 1,
            softWrap: false,
            overflow: TextOverflow.ellipsis,
            style: style,
          ),
        ),
        if (direct != null)
          Flexible(
            child: Text(
              ' · ${direct.with_.name}',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
        if (connected) ...[
          const SizedBox(width: 5),
          Tooltip(
            message: pathSummary(state),
            child: FgIcon(
              throughMember(state) ? Ic.nodes : Ic.lock,
              size: 11,
              color: color,
            ),
          ),
        ],
      ],
    );
    // In a call, the bars of this device's own connection take the path's place.
    return !connected
        ? row
        : OwnQuality(
            words: words,
            suffix: direct != null ? ' · ${direct.with_.name}' : null,
            color: color,
            card: card,
            fallback: row,
          );
  }
}

/// The app's pages and dialogs with the toasts over them all: a note said from inside the
/// settings, a dialog or any other window over the app shows above it. For
/// [MaterialApp.builder].
Widget withToasts(BuildContext context, Widget? child) =>
    Stack(fit: StackFit.expand, children: [?child, const Toasts()]);

/// The keys that mute and unmute the microphone, as this computer writes them.
String get muteKeys => onMacOS ? '⌘⇧M' : l10n.shellMuteKeys;

/// Text drawn larger than this, as the system may ask, would not fit the window at its
/// smallest: larger settings stop here. Twice the usual size is what WCAG asks text to
/// reach without losing anything.
const maxTextScale = 2.0;

/// What the app puts around every page: the system's text size up to [maxTextScale], and
/// the toasts.
Widget appFrame(BuildContext context, Widget? child) => _RoundWindow(
  child: MediaQuery.withClampedTextScaling(
    maxScaleFactor: maxTextScale,
    child: withToasts(context, child),
  ),
);

/// The window's round corners where the application draws them ([windowCorner]): what it
/// draws is clipped to them, and a hairline in the theme's border colour marks the edge,
/// as the splash's does.
class _RoundWindow extends StatelessWidget {
  const _RoundWindow({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder(
    valueListenable: windowCorner,
    child: child,
    // The same widgets whether round or square, so the window's contents keep their state
    // as it is maximized and brought back.
    builder: (context, corner, child) {
      final round = corner > 0;
      final radius = BorderRadius.circular(corner);
      return Container(
        clipBehavior: round ? Clip.antiAlias : Clip.none,
        decoration: BoxDecoration(borderRadius: radius),
        foregroundDecoration: BoxDecoration(
          borderRadius: radius,
          border: round ? Border.all(color: context.fg.border) : null,
        ),
        child: child,
      );
    },
  );
}

/// The toasts ([AppState.say]), in a column near the bottom of the window.
class Toasts extends StatelessWidget {
  const Toasts({super.key});

  @override
  Widget build(BuildContext context) {
    // Not the whole window: only the toasts, and the page for where they go.
    final nav = AppScope.watch<Navigation>(context);
    final toasts = AppScope.watch<ToastBoard>(context);
    return ListenableBuilder(
      listenable: Listenable.merge([cornerCards, pipInCorner]),
      builder: (context, column) {
        // In a room, above its controls; clear of the cards and the floating picture in
        // the corner.
        final place = toastsPlace(
          MediaQuery.sizeOf(context),
          nav.page == Screen.room ? 116 : 24,
        );
        return Positioned(
          left: place.left,
          right: place.right,
          bottom: place.bottom,
          child: column!,
        );
      },
      // Over the navigator, where no page's Material gives the text its style. Only a
      // toast with something to do takes clicks; the others let them through.
      child: Material(
        type: MaterialType.transparency,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final t in toasts.list)
              IgnorePointer(
                key: ValueKey(t.id),
                ignoring: t.path == null,
                child: _ToastPill(toast: t),
              ),
          ],
        ),
      ),
    );
  }
}

/// A toast in a pill: it rises with a little bounce and fades as it goes. One about a file
/// shows it in its folder or copies its path.
class _ToastPill extends StatefulWidget {
  const _ToastPill({required this.toast});
  final Toast toast;

  @override
  State<_ToastPill> createState() => _ToastPillState();
}

class _ToastPillState extends State<_ToastPill> {
  bool _copied = false;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final toast = widget.toast;
    final error = toast.error;
    final path = toast.path;
    final text = Text(toast.text, style: tx(c.fg, size: 13));
    final pill = Container(
      constraints: const BoxConstraints(maxWidth: 520),
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.fromLTRB(6, 6, 16, 6),
      decoration: BoxDecoration(
        color: c.dark ? c.hover : c.bg1,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: error ? c.redLine : c.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: path == null
            ? CrossAxisAlignment.center
            : CrossAxisAlignment.start,
        children: [
          Container(
            width: 26,
            height: 26,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: error ? c.redSoft : c.blueSoft,
            ),
            child: FgIcon(
              error ? Ic.warning : Ic.info,
              size: 14,
              color: error ? c.red : c.blue,
            ),
          ),
          const SizedBox(width: 10),
          Flexible(
            child: path == null
                ? text
                // The buttons go under the words, which keep the width a long path
                // needs. No tooltips: the toasts are over every page, outside the
                // overlay tooltips are drawn in, so the buttons say what they do.
                : Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        text,
                        const SizedBox(height: 6),
                        Wrap(
                          spacing: 4,
                          runSpacing: 4,
                          children: [
                            FgButton(
                              text: l10n.shellShowInFolder,
                              icon: Ic.folder,
                              height: 26,
                              fontSize: 12,
                              onTap: () => showInFolder(path),
                            ),
                            FgButton(
                              text: _copied
                                  ? l10n.commonCopied
                                  : l10n.shellCopyPath,
                              icon: _copied ? Ic.check : Ic.copy,
                              height: 26,
                              fontSize: 12,
                              onTap: () async {
                                await Clipboard.setData(
                                  ClipboardData(
                                    text: pastablePath(
                                      path,
                                      windows: onWindows,
                                    ),
                                  ),
                                );
                                if (mounted) setState(() => _copied = true);
                              },
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
          ),
        ],
      ),
    );
    // Read out by screen readers as it comes.
    final announced = Semantics(liveRegion: true, child: pill);
    return AnimatedOpacity(
      opacity: toast.leaving ? 0 : 1,
      duration: const Duration(milliseconds: 240),
      child: AnimatedSlide(
        offset: Offset(0, toast.leaving ? .3 : 0),
        duration: const Duration(milliseconds: 240),
        child: TweenAnimationBuilder<double>(
          tween: Tween(begin: 0, end: 1),
          duration: const Duration(milliseconds: 420),
          curve: spring,
          builder: (context, t, child) => Opacity(
            opacity: t.clamp(0, 1),
            child: Transform.translate(
              offset: Offset(0, (1 - t) * 18),
              child: Transform.scale(scale: .92 + .08 * t, child: child),
            ),
          ),
          child: announced,
        ),
      ),
    );
  }
}
