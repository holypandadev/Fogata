import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';

import 'app_state.dart';
import 'format.dart';
import 'icons.dart';
import 'l10n.dart';
import 'rust/api/types.dart';
import 'screens/members.dart' show confirmDialog;
import 'theme.dart';
import 'widgets.dart';

/// Opens [person]'s voice menu with a right click or a long press on [child]. In voice
/// room [voice] of [group], the menu of someone above them can also take them out of the room.
class VolumeTarget extends StatelessWidget {
  const VolumeTarget({
    super.key,
    required this.person,
    required this.child,
    this.group,
    this.voice,
  });
  final UiPerson person;
  final Widget child;
  final String? group;
  final String? voice;

  @override
  Widget build(BuildContext context) => Semantics(
    customSemanticsActions: {
      CustomSemanticsAction(label: l10n.volumeVoice): () {
        final box = context.findRenderObject() as RenderBox?;
        if (box == null) return;
        showVolumeMenu(
          context,
          box.localToGlobal(box.size.center(Offset.zero)),
          person,
          group: group,
          voice: voice,
        );
      },
    },
    // Screen readers open the menu with the action above: the right click would show
    // them as a tap that does nothing.
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      excludeFromSemantics: true,
      onSecondaryTapUp: (d) => showVolumeMenu(
        context,
        d.globalPosition,
        person,
        group: group,
        voice: voice,
      ),
      onLongPressStart: (d) => showVolumeMenu(
        context,
        d.globalPosition,
        person,
        group: group,
        voice: voice,
      ),
      child: child,
    ),
  );
}

/// How loud [person]'s voice plays on this device, in a menu opened at [at]: a volume
/// from 0 to 200 %, a mute for this device only and a way back to 100 %. For someone above
/// them in the group, in voice room [voice] of [group]: taking them out of the room, now
/// or for a while.
Future<void> showVolumeMenu(
  BuildContext context,
  Offset at,
  UiPerson person, {
  String? group,
  String? voice,
}) => showGeneralDialog<void>(
  context: context,
  barrierDismissible: true,
  barrierLabel: l10n.commonClose,
  barrierColor: Colors.transparent,
  transitionDuration: const Duration(milliseconds: 90),
  pageBuilder: (context, _, _) => CustomSingleChildLayout(
    delegate: _At(at),
    child: _VolumeMenu(person: person, group: group, voice: voice),
  ),
  transitionBuilder: (context, animation, _, child) =>
      FadeTransition(opacity: animation, child: child),
);

/// How long a time out lasts, in minutes, by its words; none until someone above them lets
/// them back.
List<(String, int?)> get timeOuts => [
  for (final minutes in const [5, 60, 24 * 60, 7 * 24 * 60])
    (spanWords(minutes * 60000), minutes),
];

/// Puts the menu at a point, kept inside the window.
class _At extends SingleChildLayoutDelegate {
  _At(this.at);
  final Offset at;

  static const _margin = 8.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints.loose(constraints.biggest)
          .deflate(const EdgeInsets.all(_margin));

  @override
  Offset getPositionForChild(Size size, Size childSize) => Offset(
    at.dx.clamp(
      _margin,
      math.max(_margin, size.width - childSize.width - _margin),
    ),
    at.dy.clamp(
      _margin,
      math.max(_margin, size.height - childSize.height - _margin),
    ),
  );

  @override
  bool shouldRelayout(_At oldDelegate) => oldDelegate.at != at;
}

class _VolumeMenu extends StatefulWidget {
  const _VolumeMenu({required this.person, this.group, this.voice});
  final UiPerson person;
  final String? group;
  final String? voice;

  @override
  State<_VolumeMenu> createState() => _VolumeMenuState();
}

class _VolumeMenuState extends State<_VolumeMenu> {
  /// The time out's durations are open beside the menu.
  bool _timeOut = false;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final person = widget.person;
    final group = state.contacts.group(widget.group);
    final voice = widget.voice;
    // Only someone above them in the group, in a room of the group.
    final above =
        group != null && voice != null && group.outranks.contains(person.id);
    final menu = _menu(context, state, above ? group : null);
    if (!above) return menu;
    final room = Contacts.voiceName(state.contacts.voiceRoom(group.id, voice));
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        menu,
        AnimatedSize(
          duration: springTime,
          curve: spring,
          alignment: Alignment.bottomLeft,
          child: _timeOut
              ? Padding(
                  padding: const EdgeInsets.only(left: 6),
                  child: _TimeOutMenu(
                    person: person,
                    room: room,
                    onPick: (minutes) {
                      Navigator.of(context).pop();
                      state.keepOutOfRoom(group.id, voice, person, minutes);
                    },
                  ),
                )
              : const SizedBox.shrink(),
        ),
      ],
    );
  }

  Widget _menu(BuildContext context, AppState state, UiGroup? group) {
    final person = widget.person;
    final c = context.fg;
    final id = person.id;
    final volume = state.prefs.volumeOf(id);
    final silenced = state.prefs.silenced.contains(id);
    final peer = state.callState.call?.peers[id];
    // While they share their screen with sound, its sound has its own volume.
    final screen = peer != null && peer.sharing && peer.sound;
    final screenVolume = state.prefs.screenVolumeOf(id);
    final screenSilenced = state.prefs.screenSilenced.contains(id);
    final scale = tx(c.fg3, size: 11, monospace: true, height: 1.2);
    return Material(
      type: MaterialType.transparency,
      child: Container(
        width: 300,
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
            Container(
              padding: const EdgeInsets.fromLTRB(10, 10, 10, 12),
              decoration: BoxDecoration(
                border: Border(bottom: BorderSide(color: c.border)),
              ),
              child: Row(
                children: [
                  Avatar(avatar: person.avatar, size: 32),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          person.name,
                          overflow: TextOverflow.ellipsis,
                          style: tx(c.fg, weight: w600, height: 1.3),
                        ),
                        Text(
                          l10n.volumeOnlyYouHear,
                          style: tx(c.fg2, size: 12, height: 1.3),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            Padding(
              padding: EdgeInsets.fromLTRB(10, 12, 10, screen ? 4 : 8),
              child: _Level(
                label: l10n.volumeVoice,
                volume: volume,
                silenced: silenced,
                onChanged: (v) => state.setVolume(id, v, save: false),
                onChangeEnd: (v) => state.setVolume(id, v),
              ),
            ),
            if (screen)
              Padding(
                padding: const EdgeInsets.fromLTRB(10, 12, 10, 8),
                child: _Level(
                  label: l10n.volumeScreenSound,
                  volume: screenVolume,
                  silenced: screenSilenced,
                  onChanged: (v) => state.setScreenVolume(id, v, save: false),
                  onChangeEnd: (v) => state.setScreenVolume(id, v),
                ),
              ),
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(l10n.volumePercent(0), style: scale),
                  Text(l10n.volumePercent(100), style: scale),
                  Text(l10n.volumePercent(200), style: scale),
                ],
              ),
            ),
            Container(
              height: 1,
              margin: const EdgeInsets.symmetric(vertical: 4),
              color: c.border,
            ),
            Semantics(
              toggled: silenced,
              child: _MenuRow(
                icon: Ic.volumeOff,
                text: screen ? l10n.volumeMuteVoiceForMe : l10n.volumeMuteForMe,
                trailing: FgSwitch(on: silenced, small: true),
                onTap: () => state.setSilenced(id, !silenced),
              ),
            ),
            if (screen)
              Semantics(
                toggled: screenSilenced,
                child: _MenuRow(
                  icon: Ic.volumeOff,
                  text: l10n.volumeMuteScreenForMe,
                  trailing: FgSwitch(on: screenSilenced, small: true),
                  onTap: () => state.setScreenSilenced(id, !screenSilenced),
                ),
              ),
            _MenuRow(
              icon: Ic.retry,
              text: l10n.volumeBackTo100,
              onTap: volume == 100 && (!screen || screenVolume == 100)
                  ? null
                  : () {
                      state.setVolume(id, 100);
                      if (screen) state.setScreenVolume(id, 100);
                    },
            ),
            if (group != null) ...[
              Container(
                height: 1,
                margin: const EdgeInsets.symmetric(vertical: 4),
                color: c.border,
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(10, 6, 10, 2),
                child: Text(
                  l10n.volumeBelowYou,
                  style: tx(c.fg3, size: 11, weight: w500, spacing: .4),
                ),
              ),
              _MenuRow(
                icon: Ic.leave,
                text: l10n.volumeRemoveFrom(
                  Contacts.voiceName(
                    state.contacts.voiceRoom(group.id, widget.voice!),
                  ),
                ),
                color: c.red,
                onTap: () async {
                  final voice = widget.voice!;
                  final room = Contacts.voiceName(
                    state.contacts.voiceRoom(group.id, voice),
                  );
                  final navigator = Navigator.of(context)..pop();
                  final ok = await confirmDialog(
                    navigator.context,
                    title: l10n.volumeRemoveTitle(person.name, room),
                    text: l10n.volumeRemoveText(person.name, room),
                    action: l10n.commonRemove,
                  );
                  if (ok) await state.removeFromRoom(group.id, voice, person);
                },
              ),
              _MenuRow(
                icon: Ic.clock,
                text: l10n.volumeTimeOutOf(
                  Contacts.voiceName(
                    state.contacts.voiceRoom(group.id, widget.voice!),
                  ),
                ),
                color: c.red,
                selected: _timeOut,
                trailing: FgIcon(Ic.chevronRight, size: 14, color: c.fg2),
                onTap: () => setState(() => _timeOut = !_timeOut),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// How long to keep someone out of a room, beside the person's menu.
class _TimeOutMenu extends StatelessWidget {
  const _TimeOutMenu({
    required this.person,
    required this.room,
    required this.onPick,
  });

  final UiPerson person;
  final String room;

  /// With the minutes, or none until someone above them lets them back.
  final ValueChanged<int?> onPick;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Material(
      type: MaterialType.transparency,
      child: Container(
        width: 240,
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
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
              child: Text(
                l10n.volumeTimeOutText(person.name, room),
                style: tx(c.fg2, size: 12, height: 1.35),
              ),
            ),
            for (final (text, minutes) in timeOuts)
              _MenuRow(text: text, onTap: () => onPick(minutes)),
            Container(
              height: 1,
              margin: const EdgeInsets.symmetric(vertical: 4),
              color: c.border,
            ),
            _MenuRow(
              text: l10n.volumeUntilLetBack,
              color: c.red,
              onTap: () => onPick(null),
            ),
          ],
        ),
      ),
    );
  }
}

/// One volume of the menu: its name, its percent and a slider from 0 to 200 %.
class _Level extends StatelessWidget {
  const _Level({
    required this.label,
    required this.volume,
    required this.silenced,
    required this.onChanged,
    required this.onChangeEnd,
  });

  final String label;
  final int volume;
  final bool silenced;
  final ValueChanged<int> onChanged;
  final ValueChanged<int> onChangeEnd;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Text(label, style: tx(silenced ? c.fg3 : c.fg, weight: w500)),
            const Spacer(),
            Text(
              l10n.volumePercent(volume),
              style: tx(silenced ? c.fg3 : c.fg, size: 12, monospace: true),
            ),
          ],
        ),
        const SizedBox(height: 4),
        VolumeSlider(
          volume: volume,
          dim: silenced,
          label: label,
          onChanged: onChanged,
          onChangeEnd: onChangeEnd,
        ),
      ],
    );
  }
}

/// A volume from 0 to 200 % in steps of 5, drawn in the design's colours.
class VolumeSlider extends StatelessWidget {
  const VolumeSlider({
    super.key,
    required this.volume,
    required this.onChanged,
    required this.onChangeEnd,
    this.label,
    this.dim = false,
    this.light = false,
  });

  final int volume;
  final ValueChanged<int> onChanged;
  final ValueChanged<int> onChangeEnd;
  final String? label;

  /// Muted: drawn greyed.
  final bool dim;

  /// Over a picture: white, whatever the theme.
  final bool light;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final on = light ? Colors.white : c.fg;
    final off = light ? Colors.white30 : c.borderStrong;
    return SliderTheme(
      data: SliderThemeData(
        trackHeight: 4,
        activeTrackColor: dim ? off : on,
        inactiveTrackColor: off,
        thumbColor: dim ? (light ? Colors.white54 : c.fg3) : on,
        overlayColor: on.withValues(alpha: .12),
        trackShape: const RoundedRectSliderTrackShape(),
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
        overlayShape: const RoundSliderOverlayShape(overlayRadius: 14),
        tickMarkShape: SliderTickMarkShape.noTickMark,
        padding: const EdgeInsets.symmetric(horizontal: 7),
      ),
      child: Semantics(
        label: label,
        child: Slider(
          value: volume.toDouble(),
          max: 200,
          divisions: 40,
          semanticFormatterCallback: (v) => l10n.volumePercent(v.round()),
          onChanged: (v) => onChanged(v.round()),
          onChangeEnd: (v) => onChangeEnd(v.round()),
        ),
      ),
    );
  }
}

class _MenuRow extends StatelessWidget {
  const _MenuRow({
    required this.text,
    this.icon,
    this.trailing,
    this.onTap,
    this.color,
    this.selected = false,
  });

  final Ic? icon;
  final String text;
  final Widget? trailing;
  final VoidCallback? onTap;

  /// The text's and icon's colour, for one that takes something away.
  final Color? color;

  /// Its own menu is open beside it.
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final enabled = onTap != null;
    final icon = this.icon;
    return Pressable(
      onTap: onTap,
      label: text,
      builder: (context, hovered) => Container(
        height: 36,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: hovered || selected ? c.hover : null,
          borderRadius: BorderRadius.circular(6),
        ),
        child: Row(
          children: [
            if (icon != null) ...[
              FgIcon(icon, size: 16, color: enabled ? color ?? c.fg2 : c.fg3),
              const SizedBox(width: 10),
            ],
            Expanded(
              child: Text(
                text,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tx(enabled ? color ?? c.fg : c.fg3),
              ),
            ),
            ?trailing,
          ],
        ),
      ),
    );
  }
}
