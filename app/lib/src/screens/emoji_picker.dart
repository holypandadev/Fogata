import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart'
    show RenderProxyBox, SliverLayoutDimensions;
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../emoji.dart';
import '../icons.dart';
import '../l10n.dart';
import '../theme.dart';
import '../widgets.dart';

/// The smiley button of a message box: opens the emoji picker above it, and puts the
/// emoji picked at the box's cursor. Shift keeps the picker open for more. The picker
/// stays inside the [PickerArea] around the button, and closes when the button is
/// hidden: its area slides away, or it is offstage.
class EmojiButton extends StatefulWidget {
  const EmojiButton({
    super.key,
    required this.controller,
    required this.focusNode,
    this.size = 34,
    this.max,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final double size;

  /// The most characters the message can have: an emoji that does not fit is not put.
  final int? max;

  @override
  State<EmojiButton> createState() => _EmojiButtonState();
}

class _EmojiButtonState extends State<EmojiButton> {
  final _portal = OverlayPortalController();

  /// The box's selection when the picker opened: its search field takes the focus.
  TextSelection? _selection;

  void _toggle() {
    if (_portal.isShowing) {
      _close();
      return;
    }
    _selection = widget.controller.selection;
    setState(_portal.show);
  }

  void _close() {
    if (!_portal.isShowing || !mounted) return;
    setState(_portal.hide);
    _back();
  }

  /// Whether the button is in view (see [_anchorShown]).
  bool _shown = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _shown = _anchorShown(context);
    // Nothing to point at any more: the picker is not drawn from now, and goes once the
    // frame is built; the focus stays where it is.
    if (!_shown && _portal.isShowing) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_shown && _portal.isShowing) setState(_portal.hide);
      });
    }
  }

  /// Back in the message box, its cursor where it was.
  void _back() {
    widget.focusNode.requestFocus();
    final selection = _selection;
    if (selection != null &&
        selection.isValid &&
        selection.end <= widget.controller.text.length) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) widget.controller.selection = selection;
      });
    }
  }

  void _pick(Emoji emoji, String text, {required bool keepOpen}) {
    final state = AppScope.read(context);
    final controller = widget.controller;
    if (_selection case final selection?
        when selection.end <= controller.text.length) {
      controller.selection = selection;
    }
    if (!insertAtCursor(controller, text, max: widget.max)) {
      state.say(l10n.emojiPickerLimit);
      return;
    }
    _selection = controller.selection;
    state.useEmoji(emoji.text);
    if (!keepOpen) _close();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return OverlayPortal.overlayChildLayoutBuilder(
      controller: _portal,
      overlayChildBuilder: (context, info) {
        if (!_shown) return const SizedBox.shrink();
        final anchor = MatrixUtils.transformRect(
          info.childPaintTransform,
          Offset.zero & info.childSize,
        );
        final area = _areaOf(this.context, info);
        // As many columns as the area holds, from the most to a few.
        final columns = ((area.width - 2 * _Above.gap - _pickerSides) / _cell)
            .floor()
            .clamp(_minColumns, _columns);
        return Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: _close,
              ),
            ),
            Positioned.fill(
              child: CustomSingleChildLayout(
                delegate: _Above(anchor: anchor, area: area),
                child: PopIn(
                  alignment: Alignment.bottomRight,
                  child: EmojiPicker(
                    onPick: _pick,
                    onClose: _close,
                    columns: columns,
                  ),
                ),
              ),
            ),
          ],
        );
      },
      child: Springy(
        key: const ValueKey('emoji-button'),
        onTap: _toggle,
        tooltip: l10n.emojiPickerButton,
        label: l10n.emojiPickerButton,
        builder: (context, hovered) => Container(
          width: widget.size,
          height: widget.size,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: hovered || _portal.isShowing ? c.hover : null,
            shape: BoxShape.circle,
          ),
          child: FgIcon(
            Ic.smiley,
            size: 17,
            color: hovered || _portal.isShowing ? c.fg : c.fg2,
          ),
        ),
      ),
    );
  }
}

/// Whether what [context] belongs to is in view: its [PickerArea] is shown and it is not
/// offstage. Panels floating over it close when it is not.
bool _anchorShown(BuildContext context) =>
    (context.dependOnInheritedWidgetOfExactType<_AreaShown>()?.shown ?? true) &&
    TickerMode.valuesOf(context).enabled;

/// Where the [PickerArea] around [context] is in the overlay, within the window: the
/// whole window when there is none.
Rect _areaOf(BuildContext context, OverlayChildLayoutInfo info) {
  final window = Offset.zero & info.overlaySize;
  final area = context.findAncestorRenderObjectOfType<_RenderArea>();
  final anchor = context.findRenderObject();
  if (area == null || anchor == null || !area.hasSize) return window;
  final back = Matrix4.tryInvert(anchor.getTransformTo(area));
  if (back == null) return window;
  return MatrixUtils.transformRect(
    info.childPaintTransform.clone()..multiply(back),
    Offset.zero & area.size,
  ).intersect(window);
}

/// Puts a panel above the anchor, their right (or left) edges lined up, 8 px inside the
/// area (the window, or a column of it); shorter when the area has less room above.
class _Above extends SingleChildLayoutDelegate {
  _Above({required this.anchor, required this.area, this.alignLeft = false});
  final Rect anchor;
  final Rect area;
  final bool alignLeft;

  static const gap = 8.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints(
        maxWidth: math.max(0, area.width - 2 * gap),
        maxHeight: math.max(0, anchor.top - area.top - 2 * gap),
      );

  @override
  Offset getPositionForChild(Size size, Size child) {
    final x = (alignLeft ? anchor.left : anchor.right - child.width)
        .clamp(
          area.left + gap,
          math.max(area.left + gap, area.right - child.width - gap),
        )
        .toDouble();
    final y = math.max(area.top + gap, anchor.top - gap - child.height);
    return Offset(x, y);
  }

  @override
  bool shouldRelayout(_Above old) =>
      old.anchor != anchor || old.area != area || old.alignLeft != alignLeft;
}

/// The part of the window an emoji picker opened inside it stays in, such as the column
/// of a chat. While not [shown] (sliding out of view), the pickers in it close.
class PickerArea extends StatelessWidget {
  const PickerArea({super.key, this.shown = true, required this.child});
  final bool shown;
  final Widget child;

  @override
  Widget build(BuildContext context) => _AreaShown(
    shown: shown,
    child: _AreaBox(child: child),
  );
}

class _AreaShown extends InheritedWidget {
  const _AreaShown({required this.shown, required super.child});
  final bool shown;

  @override
  bool updateShouldNotify(_AreaShown old) => old.shown != shown;
}

class _AreaBox extends SingleChildRenderObjectWidget {
  const _AreaBox({super.child});

  @override
  RenderObject createRenderObject(BuildContext context) => _RenderArea();
}

class _RenderArea extends RenderProxyBox {}

/// A floating panel coming in with the controls' spring.
class PopIn extends StatelessWidget {
  const PopIn({super.key, required this.child, required this.alignment});
  final Widget child;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween(begin: 0, end: 1),
    duration: springTime,
    curve: spring,
    child: child,
    builder: (context, t, child) => Opacity(
      opacity: t.clamp(0.0, 1.0),
      child: Transform.translate(
        offset: Offset(0, 6 * (1 - t)),
        child: Transform.scale(
          scale: .95 + .05 * t,
          alignment: alignment,
          child: child,
        ),
      ),
    ),
  );
}

/// The most columns of emoji, and the fewest a narrow area gets.
const _columns = 8;
const _minColumns = 6;
const _cell = 40.0;
const _header = 30.0;
const _pad = 10.0;
// The grid's padding and the border, around its columns.
const _pickerSides = 2 * _pad + 2;
const _recentShown = 16;

/// A line of the picker's list: a section's title, or up to a line's columns of emoji starting
/// at [start] of the list of all those shown.
class _Line {
  const _Line.title(this.title, this.section) : emoji = const [], start = 0;
  const _Line.emoji(this.emoji, this.start, this.section) : title = null;
  final String? title;
  final List<Emoji> emoji;
  final int start;
  final int section;
}

/// How many times a picker laid its lines out; tests count them.
@visibleForTesting
int emojiLayouts = 0;

/// The emoji picker: a search field and the skin tone, the groups to jump to, the emoji
/// in a grid (the recently used first) and the name of the one under the pointer or
/// chosen. Arrow keys choose, Enter picks (Shift+Enter keeps it open), Esc closes.
class EmojiPicker extends StatefulWidget {
  const EmojiPicker({
    super.key,
    required this.onPick,
    required this.onClose,
    this.columns = _columns,
  });

  /// Called with the emoji and the form to put (in the skin tone chosen).
  final void Function(Emoji emoji, String text, {required bool keepOpen})
  onPick;
  final VoidCallback onClose;

  /// How many emoji a line has; the picker is as wide as they are.
  final int columns;

  @override
  State<EmojiPicker> createState() => _EmojiPickerState();
}

class _EmojiPickerState extends State<EmojiPicker> {
  final _query = TextEditingController();
  final _search = FocusNode();
  final _scroll = ScrollController();

  /// The emoji chosen with the keyboard, by its place among those shown.
  int _chosen = 0;
  Emoji? _hovered;
  bool _tones = false;
  int _section = 0;
  bool _colorFont = true;

  List<_Line> _lines = const [];
  List<Emoji> _shown = const [];

  /// Where each section starts in the list, and the group it shows (null for the
  /// recently used or the results).
  List<(double, EmojiGroup?)> _sections = const [];

  @override
  void initState() {
    super.initState();
    _query.addListener(() {
      setState(() => _chosen = 0);
      if (_scroll.hasClients) _scroll.jumpTo(0);
    });
    _scroll.addListener(_followScroll);
    hasColorEmojiFont().then((has) {
      if (mounted && !has) setState(() => _colorFont = false);
    });
    // The names of the language in use come in a moment; the list is laid out again
    // then, for a search typed meanwhile and for the names shown.
    if (!emojiWordsLoaded) {
      loadEmojiWords().then((_) {
        if (mounted) setState(() => _laidOutFor = null);
      });
    }
  }

  @override
  void dispose() {
    _query.dispose();
    _search.dispose();
    _scroll.dispose();
    super.dispose();
  }

  /// The group whose emoji are at the top of the list lights up.
  void _followScroll() {
    final at = _scroll.offset + 1;
    var section = 0;
    for (var i = 0; i < _sections.length; i++) {
      if (_sections[i].$1 <= at) section = i;
    }
    if (section != _section) setState(() => _section = section);
  }

  /// Every emoji by group, in the order the groups show.
  static final List<(EmojiGroup, List<Emoji>)> _groups = [
    for (final group in EmojiGroup.values)
      (
        group,
        [
          for (final e in allEmoji)
            if (e.group == group) e,
        ],
      ),
  ];

  /// What the lines were last laid out for: the search, the recent emoji, the columns and
  /// the language of the headings.
  (String, String, int, String)? _laidOutFor;

  /// Lays the lines out again only when the search or the recent emoji change, not on
  /// each build (the pointer crossing a cell builds again).
  void _layout(AppState state) {
    final query = _query.text.trim();
    final columns = widget.columns;
    final key = (
      query,
      state.prefs.recentEmoji.join('\n'),
      columns,
      currentLanguage,
    );
    if (key == _laidOutFor) return;
    _laidOutFor = key;
    emojiLayouts++;
    final sections = <(String, EmojiGroup?, List<Emoji>)>[];
    if (query.isNotEmpty) {
      sections.add((
        l10n.emojiPickerResults,
        null,
        searchEmoji(query, recent: state.prefs.recentEmoji),
      ));
    } else {
      final recent = [
        for (final text in state.prefs.recentEmoji) ?emojiOf(text),
      ].take(_recentShown).toList();
      if (recent.isNotEmpty) {
        sections.add((l10n.emojiPickerRecent, null, recent));
      }
      for (final (group, list) in _groups) {
        if (list.isNotEmpty) sections.add((group.title, group, list));
      }
    }
    final lines = <_Line>[];
    final shown = <Emoji>[];
    final starts = <(double, EmojiGroup?)>[];
    var offset = 0.0;
    for (var s = 0; s < sections.length; s++) {
      final (title, group, list) = sections[s];
      starts.add((offset, group));
      if (query.isEmpty) {
        lines.add(_Line.title(title, s));
        offset += _header;
      }
      for (var i = 0; i < list.length; i += columns) {
        final row = list.sublist(i, math.min(i + columns, list.length));
        lines.add(_Line.emoji(row, shown.length, s));
        shown.addAll(row);
        offset += _cell;
      }
    }
    _lines = lines;
    _shown = shown;
    _sections = starts;
    if (_chosen >= shown.length) _chosen = math.max(0, shown.length - 1);
  }

  double _extent(int index, SliverLayoutDimensions _) =>
      _lines[index].title != null ? _header : _cell;

  /// The line holding emoji [index] of those shown.
  int _lineOf(int index) {
    for (var i = 0; i < _lines.length; i++) {
      final line = _lines[i];
      if (line.title == null &&
          index >= line.start &&
          index < line.start + line.emoji.length) {
        return i;
      }
    }
    return 0;
  }

  double _top(int line) {
    var y = 0.0;
    for (var i = 0; i < line; i++) {
      y += _lines[i].title != null ? _header : _cell;
    }
    return y;
  }

  /// Moves the choice by [dx] emoji along a line or [dy] lines, and scrolls it into view.
  void _move({int dx = 0, int dy = 0}) {
    if (_shown.isEmpty) return;
    var chosen = _chosen;
    if (dx != 0) {
      chosen = (chosen + dx).clamp(0, _shown.length - 1);
    } else {
      var line = _lineOf(chosen);
      final column = chosen - _lines[line].start;
      do {
        line += dy;
      } while (line >= 0 && line < _lines.length && _lines[line].title != null);
      if (line < 0 || line >= _lines.length) return;
      final to = _lines[line];
      chosen = to.start + math.min(column, to.emoji.length - 1);
    }
    setState(() {
      _chosen = chosen;
      _hovered = null;
    });
    _reveal(_lineOf(chosen));
  }

  void _reveal(int line) {
    if (!_scroll.hasClients) return;
    final top = _top(line);
    final view = _scroll.position.viewportDimension;
    final now = _scroll.offset;
    // The section's title stays in view above its first line.
    final above = line > 0 && _lines[line - 1].title != null ? _header : 0.0;
    if (top - above < now) {
      _scroll.jumpTo(top - above);
    } else if (top + _cell > now + view) {
      _scroll.jumpTo(top + _cell - view);
    }
  }

  void _jump(int section) {
    if (!_scroll.hasClients || section >= _sections.length) return;
    final to = math.min(
      _sections[section].$1,
      _scroll.position.maxScrollExtent,
    );
    _scroll.animateTo(
      to,
      duration: const Duration(milliseconds: 320),
      curve: Curves.easeOutCubic,
    );
    final first = _lines.indexWhere(
      (l) => l.section == section && l.title == null,
    );
    if (first >= 0) setState(() => _chosen = _lines[first].start);
  }

  void _pick(Emoji emoji, {bool? keepOpen}) {
    final tone = AppScope.read(context).prefs.emojiTone;
    widget.onPick(
      emoji,
      emoji.toned(tone),
      keepOpen: keepOpen ?? HardwareKeyboard.instance.isShiftPressed,
    );
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.escape) {
      if (_tones) {
        setState(() => _tones = false);
        _search.requestFocus();
      } else {
        widget.onClose();
      }
      return KeyEventResult.handled;
    }
    // The arrows and Enter act on the grid only from the search field; elsewhere they
    // move the focus or press what has it.
    if (!_search.hasFocus) return KeyEventResult.ignored;
    if (key == LogicalKeyboardKey.arrowRight) {
      _move(dx: 1);
    } else if (key == LogicalKeyboardKey.arrowLeft) {
      _move(dx: -1);
    } else if (key == LogicalKeyboardKey.arrowDown) {
      _move(dy: 1);
    } else if (key == LogicalKeyboardKey.arrowUp) {
      _move(dy: -1);
    } else if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      if (_shown.isNotEmpty) _pick(_shown[_chosen]);
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    _layout(state);
    final searching = _query.text.trim().isNotEmpty;
    final shownEmoji =
        _hovered ??
        (_shown.isEmpty ? null : _shown[_chosen.clamp(0, _shown.length - 1)]);
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onKeyEvent: _key,
      child: FocusScope(
        child: Container(
          key: const ValueKey('emoji-picker'),
          width: widget.columns * _cell + _pickerSides,
          height: 420,
          clipBehavior: Clip.antiAlias,
          // Inside the outline, as when it was drawn under what is inside.
          padding: const EdgeInsets.all(1),
          decoration: BoxDecoration(
            color: c.bg1,
            borderRadius: BorderRadius.circular(18),
          ),
          // Over what is inside, which would cover it at the rounded corners.
          foregroundDecoration: BoxDecoration(
            borderRadius: BorderRadius.circular(18),
            border: Border.all(color: c.border),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(_pad, _pad, _pad, 8),
                child: SizedBox(
                  height: 34,
                  child: AnimatedSwitcher(
                    duration: springTime,
                    switchInCurve: spring,
                    switchOutCurve: Curves.easeIn,
                    transitionBuilder: (child, t) => FadeTransition(
                      opacity: t,
                      child: ScaleTransition(
                        scale: Tween(begin: .94, end: 1.0).animate(t),
                        alignment: Alignment.centerRight,
                        child: child,
                      ),
                    ),
                    child: _tones
                        ? _ToneRow(
                            key: const ValueKey('tones'),
                            tone: state.prefs.emojiTone,
                            onPick: (tone) {
                              state.setEmojiTone(tone);
                              setState(() => _tones = false);
                              _search.requestFocus();
                            },
                          )
                        : Row(
                            key: const ValueKey('search'),
                            children: [
                              Expanded(
                                child: FgField(
                                  key: const ValueKey('emoji-search'),
                                  controller: _query,
                                  focusNode: _search,
                                  autofocus: true,
                                  height: 34,
                                  radius: 999,
                                  fontSize: 13,
                                  placeholder: l10n.emojiPickerSearch,
                                  leading: FgIcon(
                                    Ic.search,
                                    size: 14,
                                    color: c.fg3,
                                  ),
                                  label: l10n.emojiPickerSearch,
                                ),
                              ),
                              const SizedBox(width: 6),
                              Springy(
                                key: const ValueKey('emoji-tone'),
                                onTap: () => setState(() => _tones = true),
                                tooltip: l10n.emojiPickerSkinTone,
                                label: l10n.emojiPickerSkinTone,
                                builder: (context, hovered) => Container(
                                  width: 34,
                                  height: 34,
                                  alignment: Alignment.center,
                                  decoration: BoxDecoration(
                                    color: hovered ? c.hover : null,
                                    shape: BoxShape.circle,
                                    border: Border.all(color: c.border),
                                  ),
                                  child: Text(
                                    _hand(state.prefs.emojiTone),
                                    style: emojiStyle(17),
                                  ),
                                ),
                              ),
                            ],
                          ),
                  ),
                ),
              ),
              if (!searching)
                _Groups(
                  groups: [for (final (_, g) in _sections) g],
                  recent: _sections.isNotEmpty && _sections.first.$2 == null,
                  selected: _section,
                  onSelect: _jump,
                ),
              Container(height: 1, color: c.border),
              Expanded(
                child: _shown.isEmpty
                    ? Center(
                        child: Text(
                          l10n.emojiPickerNone,
                          style: tx(c.fg3, size: 13),
                        ),
                      )
                    : MouseRegion(
                        onExit: (_) => setState(() => _hovered = null),
                        child: ListView.builder(
                          controller: _scroll,
                          padding: const EdgeInsets.symmetric(horizontal: _pad),
                          itemCount: _lines.length,
                          itemExtentBuilder: _extent,
                          itemBuilder: (context, i) {
                            final line = _lines[i];
                            if (line.title case final title?) {
                              return Container(
                                alignment: Alignment.bottomLeft,
                                padding: const EdgeInsets.only(
                                  left: 4,
                                  bottom: 6,
                                ),
                                child: Text(
                                  title,
                                  style: tx(c.fg3, size: 12, weight: w500),
                                ),
                              );
                            }
                            return Row(
                              children: [
                                for (var j = 0; j < line.emoji.length; j++)
                                  _Cell(
                                    emoji: line.emoji[j],
                                    text: line.emoji[j].toned(
                                      state.prefs.emojiTone,
                                    ),
                                    chosen: line.start + j == _chosen,
                                    onHover: (e) =>
                                        setState(() => _hovered = e),
                                    onTap: () => _pick(line.emoji[j]),
                                  ),
                              ],
                            );
                          },
                        ),
                      ),
              ),
              Container(height: 1, color: c.border),
              SizedBox(
                height: _colorFont ? 46 : 62,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 14),
                  child: _colorFont
                      ? Row(
                          children: [
                            if (shownEmoji != null) ...[
                              Text(
                                shownEmoji.toned(state.prefs.emojiTone),
                                style: emojiStyle(22),
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Text(
                                  _capital(shownEmoji.label),
                                  key: const ValueKey('emoji-name'),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: tx(c.fg, size: 13, weight: w500),
                                ),
                              ),
                            ],
                          ],
                        )
                      : Align(
                          alignment: Alignment.centerLeft,
                          child: Text(
                            l10n.emojiPickerNoFont,
                            style: tx(c.fg3, size: 12),
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

String _hand(int tone) => tone == 0 ? '✋' : '✋${skinTones[tone - 1]}';

String _capital(String name) =>
    name.isEmpty ? name : name[0].toUpperCase() + name.substring(1);

/// One emoji of the grid: it grows a little under the pointer, on a soft square that
/// also marks the one chosen with the keyboard.
class _Cell extends StatefulWidget {
  const _Cell({
    required this.emoji,
    required this.text,
    required this.chosen,
    required this.onHover,
    required this.onTap,
  });

  final Emoji emoji;
  final String text;
  final bool chosen;
  final ValueChanged<Emoji> onHover;
  final VoidCallback onTap;

  @override
  State<_Cell> createState() => _CellState();
}

class _CellState extends State<_Cell> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final lit = _hovered || widget.chosen;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) {
        setState(() => _hovered = true);
        widget.onHover(widget.emoji);
      },
      onExit: (_) => setState(() => _hovered = false),
      child: Listener(
        onPointerDown: (_) => setState(() => _pressed = true),
        onPointerUp: (_) => setState(() => _pressed = false),
        onPointerCancel: (_) => setState(() => _pressed = false),
        child: GestureDetector(
          onTap: widget.onTap,
          child: Semantics(
            button: true,
            label: widget.emoji.label,
            child: Container(
              width: _cell,
              height: _cell,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: lit ? c.hover : null,
                borderRadius: BorderRadius.circular(10),
              ),
              child: AnimatedScale(
                scale: _pressed
                    ? .9
                    : _hovered
                    ? 1.14
                    : 1,
                duration: springTime,
                curve: spring,
                child: Text(widget.text, style: emojiStyle(24)),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The groups to jump to, as icons on a pill row: the one in view sits on a raised pill
/// that slides to it.
class _Groups extends StatelessWidget {
  const _Groups({
    required this.groups,
    required this.recent,
    required this.selected,
    required this.onSelect,
  });

  /// One per section: null for the recently used.
  final List<EmojiGroup?> groups;
  final bool recent;
  final int selected;
  final ValueChanged<int> onSelect;

  static const _size = 32.0;
  static const _space = 3.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Padding(
      padding: const EdgeInsets.fromLTRB(_pad, 0, _pad, 8),
      child: SizedBox(
        height: _size,
        child: LayoutBuilder(
          builder: (context, box) {
            // A narrow picker draws them closer, and a little narrower when it must.
            final step = math.min(
              _size + _space,
              (box.maxWidth + _space) / math.max(1, groups.length),
            );
            final size = math.min(_size, step);
            return Stack(
              children: [
                AnimatedPositioned(
                  duration: springTime,
                  curve: spring,
                  left: selected * step,
                  top: 0,
                  width: size,
                  height: _size,
                  child: Container(
                    decoration: BoxDecoration(
                      color: c.dark ? c.raised : c.hover,
                      borderRadius: BorderRadius.circular(999),
                      border: Border.all(color: c.border),
                    ),
                  ),
                ),
                for (var i = 0; i < groups.length; i++)
                  Positioned(
                    left: i * step,
                    top: 0,
                    width: size,
                    height: _size,
                    child: Springy(
                      onTap: () => onSelect(i),
                      tooltip: groups[i]?.title ?? l10n.emojiPickerRecent,
                      label: groups[i]?.title ?? l10n.emojiPickerRecent,
                      builder: (context, hovered) => Center(
                        child: FgIcon(
                          groups[i]?.icon ?? Ic.history,
                          size: 16,
                          color: i == selected || hovered ? c.fg : c.fg3,
                        ),
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}

/// The six skin tones on a pill, in place of the search field while choosing.
class _ToneRow extends StatelessWidget {
  const _ToneRow({super.key, required this.tone, required this.onPick});
  final int tone;
  final ValueChanged<int> onPick;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 4),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.border),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: 8),
            child: Text(l10n.emojiPickerSkinTone, style: tx(c.fg3, size: 12)),
          ),
          for (var i = 0; i <= 5; i++)
            Springy(
              key: ValueKey('tone-$i'),
              onTap: () => onPick(i),
              label: i == 0
                  ? l10n.emojiPickerDefaultTone
                  : l10n.emojiPickerTone(i),
              builder: (context, hovered) => Container(
                width: 30,
                height: 30,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: i == tone || hovered ? c.hover : null,
                  shape: BoxShape.circle,
                  border: i == tone ? Border.all(color: c.borderStrong) : null,
                ),
                child: Text(_hand(i), style: emojiStyle(16)),
              ),
            ),
        ],
      ),
    );
  }
}

/// Watches a message box for ":name" before the cursor and offers the emoji whose name
/// starts that way above it: arrows choose, Enter or Tab puts it in place, Esc leaves
/// the text as it is. A whole ":name:" with an exact name becomes the emoji at once.
class EmojiComplete extends StatefulWidget {
  const EmojiComplete({
    super.key,
    required this.controller,
    required this.child,
    this.max,
  });

  final TextEditingController controller;
  final Widget child;
  final int? max;

  @override
  State<EmojiComplete> createState() => _EmojiCompleteState();
}

final _typed = RegExp(r'(?:^|[\s(])(:[\p{L}\p{N}_+\-]{2,})$', unicode: true);
final _closed = RegExp(r'(?:^|[\s(])(:[\p{L}\p{N}_+\-]{2,}:)$', unicode: true);

class _EmojiCompleteState extends State<EmojiComplete> {
  final _portal = OverlayPortalController();
  List<Emoji> _found = const [];
  int _chosen = 0;

  /// Where the ":name" starts in the text, and the token turned down with Esc.
  int _from = 0;
  String? _dismissed;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_changed);
  }

  @override
  void didUpdateWidget(EmojiComplete old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller.removeListener(_changed);
      widget.controller.addListener(_changed);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    final value = widget.controller.value;
    final sel = value.selection;
    if (!sel.isValid || !sel.isCollapsed || !value.composing.isCollapsed) {
      _hide();
      return;
    }
    final before = value.text.substring(0, sel.baseOffset);
    if (_closed.firstMatch(before) case final m?) {
      final token = m.group(1)!;
      if (emojiNamed(token.substring(1, token.length - 1)) case final emoji?) {
        _put(emoji, before.length - token.length, before.length);
        return;
      }
    }
    final m = _typed.firstMatch(before);
    final token = m?.group(1);
    if (token == null || token == _dismissed) {
      if (token == null) _dismissed = null;
      _hide();
      return;
    }
    final state = AppScope.read(context);
    if (!emojiWordsLoaded) {
      // Searches by English names meanwhile, and again once the language's are in.
      loadEmojiWords().then((_) {
        if (mounted) _changed();
      });
    }
    final found = searchEmoji(
      token.substring(1),
      recent: state.prefs.recentEmoji,
    ).take(8).toList();
    if (found.isEmpty) {
      _hide();
      return;
    }
    setState(() {
      _found = found;
      _chosen = 0;
      _from = before.length - token.length;
    });
    if (!_portal.isShowing && _shown) _portal.show();
  }

  /// Whether the message box is in view (see [_anchorShown]).
  bool _shown = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _shown = _anchorShown(context);
    if (!_shown && _portal.isShowing) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!_shown) _hide();
      });
    }
  }

  void _hide() {
    if (_portal.isShowing && mounted) setState(_portal.hide);
  }

  /// Puts [emoji] in place of the text from [from] to [to].
  void _put(Emoji emoji, int from, int to) {
    final state = AppScope.read(context);
    final controller = widget.controller;
    controller.selection = TextSelection(baseOffset: from, extentOffset: to);
    insertAtCursor(
      controller,
      emoji.toned(state.prefs.emojiTone),
      max: widget.max,
    );
    state.useEmoji(emoji.text);
    _hide();
  }

  void _pick(int i) {
    final end = widget.controller.selection.baseOffset;
    _put(_found[i], _from, end);
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (!_portal.isShowing || _found.isEmpty) return KeyEventResult.ignored;
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.arrowDown) {
      setState(() => _chosen = (_chosen + 1) % _found.length);
    } else if (key == LogicalKeyboardKey.arrowUp) {
      setState(() => _chosen = (_chosen - 1) % _found.length);
    } else if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter ||
        key == LogicalKeyboardKey.tab) {
      _pick(_chosen);
    } else if (key == LogicalKeyboardKey.escape) {
      final value = widget.controller.value;
      final before = value.text.substring(0, value.selection.baseOffset);
      _dismissed = _typed.firstMatch(before)?.group(1);
      _hide();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onKeyEvent: _key,
      child: OverlayPortal.overlayChildLayoutBuilder(
        controller: _portal,
        overlayChildBuilder: (context, info) {
          if (!_shown) return const SizedBox.shrink();
          final anchor = MatrixUtils.transformRect(
            info.childPaintTransform,
            Offset.zero & info.childSize,
          );
          return CustomSingleChildLayout(
            delegate: _Above(
              anchor: anchor.inflate(6),
              area: _areaOf(this.context, info),
              alignLeft: true,
            ),
            child: PopIn(
              alignment: Alignment.bottomLeft,
              child: Container(
                key: const ValueKey('emoji-complete'),
                width: 300,
                padding: const EdgeInsets.all(5),
                decoration: BoxDecoration(
                  color: c.bg1,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: c.border),
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (var i = 0; i < _found.length; i++)
                      _Suggestion(
                        emoji: _found[i],
                        tone: AppScope.read(context).prefs.emojiTone,
                        chosen: i == _chosen,
                        onHover: () => setState(() => _chosen = i),
                        onTap: () => _pick(i),
                      ),
                  ],
                ),
              ),
            ),
          );
        },
        child: widget.child,
      ),
    );
  }
}

class _Suggestion extends StatelessWidget {
  const _Suggestion({
    required this.emoji,
    required this.tone,
    required this.chosen,
    required this.onHover,
    required this.onTap,
  });

  final Emoji emoji;
  final int tone;
  final bool chosen;
  final VoidCallback onHover;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => onHover(),
      child: GestureDetector(
        onTap: onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: chosen ? c.hover : null,
            borderRadius: BorderRadius.circular(999),
          ),
          child: Row(
            children: [
              Text(emoji.toned(tone), style: emojiStyle(18)),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  ':${shortName(emoji)}:',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(chosen ? c.fg : c.fg2, size: 13),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
