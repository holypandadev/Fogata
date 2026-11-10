part of '../room.dart';

// The call's grid: a tile for each person (their avatar, or their camera), how many fit
// the room, the strip under a spotlight, the cards of a live screen, and the tile of
// someone being rung.

/// A participant's tile: their avatar, or later their camera.
class _Tile extends StatelessWidget {
  const _Tile({
    required this.person,
    required this.label,
    this.avatarSize = 88,
    this.away = false,
    this.connecting = false,
    this.connected = false,
    this.video,
    this.mirror = false,
    this.speaking = false,
    this.muted = false,
    this.volume = 100,
    this.silenced = false,
    this.quality = false,
    this.onTap,
  });

  final UiPerson person;
  final String label;
  final double avatarSize;

  /// Shows how the person's connection is, top right, and dims them while it is lost.
  final bool quality;

  /// A click on the tile: enlarges a camera.
  final VoidCallback? onTap;
  final bool away;
  final bool connecting;
  final bool connected;

  /// Camera to show instead of the avatar.
  final RTCVideoRenderer? video;
  final bool mirror;
  final bool speaking;
  final bool muted;

  /// How loud this person plays on this device, in percent, and whether not at all.
  final int volume;
  final bool silenced;

  @override
  Widget build(BuildContext context) {
    final video = this.video;
    if (video == null) return _build(context, null);
    // The picture once its first frame came; until then, as with no camera, the avatar.
    return ValueListenableBuilder<RTCVideoValue>(
      valueListenable: video,
      builder: (context, value, _) =>
          _build(context, value.width > 0 ? video : null),
    );
  }

  Widget _build(BuildContext context, RTCVideoRenderer? video) {
    final c = context.fg;
    // As wide as its words, and a long name is cut short in a narrow tile. Screen
    // readers hear it as the tile's label instead.
    final name = ExcludeSemantics(
      child: Container(
        height: 26,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        decoration: BoxDecoration(
          color: connecting ? c.bg1 : const Color.fromRGBO(0, 0, 0, .6),
          borderRadius: BorderRadius.circular(6),
          boxShadow: connecting
              ? [BoxShadow(color: c.border, spreadRadius: 1)]
              : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (connected) ...[
              const Dot(Color(0xFF3DD68C)),
              const SizedBox(width: 6),
            ],
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tx(
                  connecting ? c.fg2 : Colors.white,
                  size: 12,
                  weight: w500,
                  height: 1.2,
                ),
              ),
            ),
            if (silenced) ...[
              const SizedBox(width: 6),
              FgIcon(Ic.volumeOff, size: 12, color: c.red),
            ] else if (volume != 100) ...[
              const SizedBox(width: 6),
              Text(
                l10n.volumePercent(volume),
                style: tx(
                  connecting ? c.fg3 : Colors.white70,
                  size: 11,
                  monospace: true,
                  height: 1.2,
                ),
              ),
            ],
            if (speaking) ...[
              const SizedBox(width: 8),
              const SpeakingBars(color: Color(0xFF3DD68C)),
            ] else if (muted) ...[
              const SizedBox(width: 6),
              const FgIcon(Ic.micOff, size: 12, color: Colors.white),
            ],
          ],
        ),
      ),
    );
    final tag = Positioned(
      left: 10,
      right: 10,
      bottom: 10,
      child: Align(alignment: Alignment.centerLeft, child: name),
    );
    // To a screen reader, the tile is its name, whether the microphone is off and how
    // loud it plays here, said once: the icons and the name's tag say it to the eye.
    var said = muted ? l10n.roomTileMicOff(label) : label;
    if (silenced) {
      said = l10n.roomTileMutedByYou(said);
    } else if (volume != 100) {
      said = l10n.roomTileVolume(said, l10n.volumePercent(volume));
    }
    if (connecting) {
      return Semantics(
        container: true,
        label: said,
        child: Skeleton(radius: 12, child: Stack(children: [tag])),
      );
    }
    // Without a picture, the tile takes on the main colour of the avatar shown in it,
    // mixed into the usual one so the name and the icons on it still read.
    Widget tinted(Widget Function(Color colour) build) => video != null
        ? build(c.tile)
        : MainColour(
            avatar: person.avatar,
            builder: (context, colour) => build(
              colour == null ? c.tile : Color.lerp(c.tile, colour, .35)!,
            ),
          );
    final tile = tinted(
      (colour) => AnimatedContainer(
        duration: const Duration(milliseconds: 240),
        decoration: BoxDecoration(
          color: colour,
          borderRadius: BorderRadius.circular(12),
          boxShadow: [
            BoxShadow(
              color: speaking ? c.green : c.border,
              spreadRadius: speaking ? 2 : 1,
            ),
          ],
        ),
        child: Stack(
          children: [
            if (video != null)
              Positioned.fill(
                child: QualityDim(
                  id: quality ? person.id : null,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(12),
                    child: RTCVideoView(
                      video,
                      mirror: mirror,
                      objectFit:
                          RTCVideoViewObjectFit.RTCVideoViewObjectFitCover,
                    ),
                  ),
                ),
              )
            else
              Positioned.fill(
                child: _TileAvatar(
                  avatar: person.avatar,
                  id: person.id,
                  size: avatarSize,
                  dim: away,
                  quality: quality ? person.id : null,
                  name: name,
                ),
              ),
            if (video != null) tag,
            if (quality)
              Positioned(right: 10, top: 10, child: TileQuality(id: person.id)),
          ],
        ),
      ),
    );
    final onTap = this.onTap;
    if (onTap == null) {
      return Semantics(label: said, child: tile);
    }
    // Reached with Tab and Enter too, with the focus ring around it.
    return Pressable(
      onTap: onTap,
      label: said,
      focusRadius: 12,
      builder: (context, _) => tile,
    );
  }
}

/// A tile's avatar, as large as asked and in the middle, with the [name] at the bottom
/// left. In a low tile (in the strip under a screen or camera, or in a full grid) the
/// avatar gets smaller, still in the middle, to stay clear of the name.
class _TileAvatar extends StatelessWidget {
  const _TileAvatar({
    required this.avatar,
    required this.id,
    required this.size,
    required this.dim,
    required this.quality,
    required this.name,
  });
  final UiAvatar avatar;
  final String id;
  final double size;
  final bool dim;

  /// Whose connection dims the avatar while it is lost.
  final String? quality;
  final Widget name;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      Widget face(double size) => QualityDim(
        id: quality,
        child: Avatar(avatar: avatar, size: size, dim: dim, id: id),
      );
      // The name's band at the bottom (10 + 26) and a gap (2), kept free above the
      // avatar too so it stays in the middle.
      final fits = math
          .min(box.maxWidth - 24, box.maxHeight - 76)
          .clamp(24, size)
          .toDouble();
      return Stack(
        children: [
          Center(child: face(fits)),
          Positioned(
            left: 10,
            right: 10,
            bottom: 10,
            child: Align(alignment: Alignment.centerLeft, child: name),
          ),
        ],
      );
    },
  );
}

/// How tiles fill an area: how many columns, the size of each, and whether they scroll.
typedef GridFit = ({int columns, Size tile, bool scrolls});

/// The least a tile is drawn at: smaller, its name and controls would not fit. Below it
/// the tiles keep this size and scroll.
const minTile = Size(160, 100);

/// Space between tiles.
const tileGap = 12.0;

/// Where `n` tiles go in `area`: as large as it allows, never taller than 3:4 of their
/// width nor wider than 16:9. When even the best fit is smaller than [minTile], as many
/// columns as fit at its width, and the rows scroll.
@visibleForTesting
GridFit gridFit(int n, Size area) {
  GridFit best = (columns: 1, tile: Size.zero, scrolls: false);
  for (var columns = 1; columns <= n; columns++) {
    final rows = (n / columns).ceil();
    final width = (area.width - tileGap * (columns - 1)) / columns;
    final fit = (area.height - tileGap * (rows - 1)) / rows;
    if (width <= 0 || fit <= 0) continue;
    final height = math.min(width * .75, fit);
    final tile = Size(math.min(width, height * 16 / 9), height);
    if (tile.width * tile.height > best.tile.width * best.tile.height + .5) {
      best = (columns: columns, tile: tile, scrolls: false);
    }
  }
  if (best.tile.width >= minTile.width && best.tile.height >= minTile.height) {
    return best;
  }
  final columns = ((area.width + tileGap) / (minTile.width + tileGap))
      .floor()
      .clamp(1, math.max(1, n))
      .toInt();
  final width = math.max(
    minTile.width,
    (area.width - tileGap * (columns - 1)) / columns,
  );
  return (
    columns: columns,
    tile: Size(width, math.max(minTile.height, width * 9 / 16)),
    scrolls: true,
  );
}

/// Tiles in rows and columns, centred, as large as [gridFit] allows; they scroll when
/// there are too many to keep a usable size.
class _Grid extends StatelessWidget {
  const _Grid({required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final n = children.length;
    return LayoutBuilder(
      builder: (context, box) {
        final fit = gridFit(n, box.biggest);
        final rows = (n / fit.columns).ceil();
        final grid = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var r = 0; r < rows; r++) ...[
              if (r > 0) const SizedBox(height: tileGap),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (
                    var col = 0;
                    col < fit.columns && r * fit.columns + col < n;
                    col++
                  ) ...[
                    if (col > 0) const SizedBox(width: tileGap),
                    SizedBox.fromSize(
                      size: fit.tile,
                      child: children[r * fit.columns + col],
                    ),
                  ],
                ],
              ),
            ],
          ],
        );
        if (!fit.scrolls) return Center(child: grid);
        return PageScroll(
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minWidth: box.maxWidth,
              minHeight: box.maxHeight,
            ),
            child: Center(child: grid),
          ),
        );
      },
    );
  }
}

/// The others under a watched screen or an enlarged camera: one row of tiles, centred.
/// When they do not all fit, the row scrolls sideways, with the mouse wheel or the
/// arrows at its ends.
class _Strip extends StatefulWidget {
  const _Strip({required this.tiles});
  final List<Widget> tiles;

  /// The narrowest and widest a tile of the strip is drawn.
  static const minWidth = 136.0;
  static const maxWidth = 220.0;

  @override
  State<_Strip> createState() => _StripState();
}

class _StripState extends State<_Strip> {
  final _scroll = ScrollController();
  bool _back = false;
  bool _forward = false;

  /// Whether [_ends] runs after the frame being built: once per frame, however many
  /// builds it has.
  bool _endsPending = false;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_ends);
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  /// Shows an arrow at each end with more tiles past it.
  void _ends() {
    if (!mounted || !_scroll.hasClients) return;
    final at = _scroll.position;
    final back = at.pixels > 1;
    final forward = at.pixels < at.maxScrollExtent - 1;
    if (back != _back || forward != _forward) {
      setState(() {
        _back = back;
        _forward = forward;
      });
    }
  }

  void _page(int direction) {
    if (!_scroll.hasClients) return;
    final at = _scroll.position;
    _scroll.animateTo(
      (at.pixels + direction * at.viewportDimension * .8).clamp(
        0,
        at.maxScrollExtent,
      ),
      duration: springTime,
      curve: Curves.easeOutCubic,
    );
  }

  /// The mouse wheel turned up or down moves the row sideways.
  void _wheel(PointerSignalEvent event) {
    if (event is! PointerScrollEvent || !_scroll.hasClients) return;
    final delta = event.scrollDelta.dx != 0
        ? event.scrollDelta.dx
        : event.scrollDelta.dy;
    if (delta == 0) return;
    GestureBinding.instance.pointerSignalResolver.register(event, (_) {
      final at = _scroll.position;
      _scroll.jumpTo((at.pixels + delta).clamp(0, at.maxScrollExtent));
    });
  }

  Widget _arrow(Ic icon, String tip, int direction) => Springy(
    onTap: () => _page(direction),
    tooltip: tip,
    builder: (context, hovered) {
      final c = context.fg;
      return Container(
        width: 32,
        height: 32,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? Color.lerp(c.bg2, c.fg, .06) : c.bg2,
          shape: BoxShape.circle,
          border: Border.all(color: c.borderStrong),
        ),
        child: FgIcon(icon, size: 14, color: c.fg),
      );
    },
  );

  @override
  Widget build(BuildContext context) {
    final tiles = widget.tiles;
    final n = tiles.length;
    return LayoutBuilder(
      builder: (context, box) {
        final width = ((box.maxWidth - tileGap * (n - 1)) / n).clamp(
          _Strip.minWidth,
          _Strip.maxWidth,
        );
        if (n * width + tileGap * (n - 1) <= box.maxWidth + .5) {
          return Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              for (var i = 0; i < n; i++) ...[
                if (i > 0) const SizedBox(width: tileGap),
                SizedBox(width: width, child: tiles[i]),
              ],
            ],
          );
        }
        if (!_endsPending) {
          _endsPending = true;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            _endsPending = false;
            _ends();
          });
        }
        return Stack(
          children: [
            Positioned.fill(
              child: Listener(
                onPointerSignal: _wheel,
                child: ListView.separated(
                  controller: _scroll,
                  scrollDirection: Axis.horizontal,
                  itemCount: n,
                  separatorBuilder: (context, _) =>
                      const SizedBox(width: tileGap),
                  itemBuilder: (context, i) =>
                      SizedBox(width: width, child: tiles[i]),
                ),
              ),
            ),
            if (_back)
              Positioned(
                left: 6,
                top: 0,
                bottom: 0,
                child: Center(
                  child: _arrow(Ic.chevronLeft, l10n.roomEarlierPeople, -1),
                ),
              ),
            if (_forward)
              Positioned(
                right: 6,
                top: 0,
                bottom: 0,
                child: Center(
                  child: _arrow(Ic.chevronRight, l10n.roomMorePeople, 1),
                ),
              ),
          ],
        );
      },
    );
  }
}

/// Builds again each time who is talking in [call] changes, while the rest of the room
/// stays as it is.
class Talking extends StatelessWidget {
  const Talking({super.key, required this.call, required this.builder});
  final Call? call;
  final WidgetBuilder builder;

  @override
  Widget build(BuildContext context) {
    final talking = call?.talking;
    if (talking == null) return builder(context);
    return ListenableBuilder(
      listenable: talking,
      builder: (context, _) => builder(context),
    );
  }
}

class _Tiles extends StatelessWidget {
  const _Tiles({required this.state, this.connecting = false});
  final AppState state;
  final bool connecting;

  @override
  Widget build(BuildContext context) {
    final room = state.roomState.room;
    final profile = state.contacts.profile!;
    if (connecting) {
      final expected = _expected(state);
      final paths = state.contacts.paths[state.roomState.roomGroup] ?? const {};
      return _Grid(
        children: [
          _Tile(
            person: profile.person,
            label: l10n.roomTileYou(profile.person.name),
            avatarSize: 72,
          ),
          for (final p in expected)
            paths.containsKey(p.id)
                ? _Tile(
                    person: p,
                    label: p.name,
                    avatarSize: 72,
                    connected: true,
                  )
                : _Tile(
                    person: p,
                    label: l10n.roomTileConnecting(p.name),
                    connecting: true,
                  ),
        ],
      );
    }
    final call = state.callState.call;
    final people = [...room!.participants]
      ..sort((a, b) => a.me == b.me ? 0 : (a.me ? 1 : -1));
    // The camera enlarged here, while it is on.
    final focusedId = call?.focused;
    final focusedPeer = focusedId == null ? null : call!.peers[focusedId];
    final focusedPerson = people
        .where((p) => p.person.id == focusedId)
        .firstOrNull
        ?.person;
    final RTCVideoRenderer? focusedCamera = focusedPerson == null
        ? null
        : focusedId == profile.person.id
        ? (call!.camera ? call.localCamera : null)
        : (focusedPeer != null && focusedPeer.connected && focusedPeer.cam
              ? focusedPeer.camera
              : null);
    final tiles = <Widget>[];
    // Shared screens first: this device's own, and a card for each of the others',
    // which is only sent here once watched.
    if (call != null && call.sharing && !call.ownScreen) {
      tiles.add(
        _OwnShareCard(
          watchers: call.watchers,
          onPreview: () => call.showOwnScreen(true),
        ),
      );
    }
    UiParticipant? watched;
    for (final p in people) {
      final peer = call?.peers[p.person.id];
      if (p.me || peer == null || !peer.sharing || !peer.connected) continue;
      if (call!.watching == p.person.id) {
        watched = p;
      } else {
        tiles.add(
          _LiveCard(person: p.person, onWatch: () => call.watch(p.person.id)),
        );
      }
    }
    for (final p in people) {
      // The enlarged camera is not repeated in the strip under it.
      if (focusedCamera != null && p.person.id == focusedId) continue;
      if (p.me) {
        final camera = call != null && call.camera;
        tiles.add(
          Talking(
            call: call,
            builder: (context) => _Tile(
              person: p.person,
              label: l10n.roomTileYou(p.person.name),
              video: camera ? call.localCamera : null,
              mirror: state.prefs.mirrorCamera,
              speaking: call?.speaking ?? false,
              muted: call != null && !call.mic,
              quality: true,
              onTap: camera ? () => call.focus(p.person.id) : null,
            ),
          ),
        );
        continue;
      }
      final peer = call?.peers[p.person.id];
      final reached = peer != null && peer.connected;
      // A camera whose picture failed for good shows the avatar, as one that is off.
      final camera = reached && peer.cam && !peer.camera.failed;
      tiles.add(
        VolumeTarget(
          person: p.person,
          group: state.roomState.roomGroup,
          voice: state.roomState.roomVoice,
          child: Talking(
            call: call,
            builder: (context) => _Tile(
              person: p.person,
              label: p.away
                  ? l10n.roomTileAway(p.person.name)
                  : (peer != null && peer.voiceless
                        ? l10n.roomTileNoSound(p.person.name)
                        : call != null && !reached
                        ? l10n.roomTileConnecting(p.person.name)
                        : p.person.name),
              away: p.away,
              video: camera ? peer.camera : null,
              speaking: reached && peer.speaking,
              muted: reached && !peer.mic,
              volume: state.prefs.volumeOf(p.person.id),
              silenced: state.prefs.silenced.contains(p.person.id),
              quality: true,
              onTap: camera ? () => call!.focus(p.person.id) : null,
            ),
          ),
        ),
      );
    }
    // A direct call: who left a moment ago and may come back, and who is being rung.
    final direct = state.callState.callIn(state.roomState.roomGroup);
    if (direct != null) {
      final inRoom = {for (final p in people) p.person.id};
      for (final p in direct.away) {
        if (inRoom.contains(p.id)) continue;
        tiles.add(
          _Tile(person: p, label: l10n.roomTileLeft(p.name), away: true),
        );
      }
      for (final p in direct.adding) {
        if (inRoom.contains(p.id)) continue;
        tiles.add(
          _RingingTile(person: p, onCancel: () => state.cancelAdd(p.id)),
        );
      }
    }
    if (call != null && call.ownScreen && call.sharing) {
      return _Spotlight(
        id: profile.person.id,
        kind: _Shown.ownScreen,
        title: l10n.pipYourScreen,
        renderer: call.localScreen,
        watchers: call.watchers,
        closeLabel: l10n.commonBack,
        onClose: () => call.showOwnScreen(false),
        strip: tiles,
      );
    }
    if (call != null && focusedCamera != null && focusedPerson != null) {
      final mine = focusedPerson.id == profile.person.id;
      return _Spotlight(
        id: focusedPerson.id,
        kind: _Shown.camera,
        title: mine ? l10n.pipYourCamera : focusedPerson.name,
        avatar: focusedPerson.avatar,
        renderer: focusedCamera,
        mirror: mine && state.prefs.mirrorCamera,
        from: mine ? null : focusedPerson.id,
        closeLabel: l10n.commonBack,
        onClose: () => call.focus(null),
        strip: tiles,
      );
    }
    if (watched != null && call != null) {
      final id = watched.person.id;
      return _Spotlight(
        id: id,
        title: l10n.pipScreenOf(watched.person.name),
        renderer: call.peers[id]!.screen,
        watchers: call.watchersOf(id),
        from: id,
        closeLabel: l10n.pipStopWatching,
        onClose: () => call.watch(null),
        soundOf: call.peers[id]!.sound ? id : null,
        strip: tiles,
      );
    }
    return _Grid(children: tiles);
  }
}

/// Someone shares their screen: nothing of it comes here until it is watched.
class _LiveCard extends StatelessWidget {
  const _LiveCard({required this.person, required this.onWatch});
  final UiPerson person;
  final VoidCallback onWatch;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return LayoutBuilder(
      builder: (context, box) {
        // Small in the strip under a watched screen, or in a short tile where the two lines
        // of words would not fit with the picture: the name and the button only.
        final small = box.maxHeight < 200 || box.maxWidth < 200;
        return Container(
          decoration: BoxDecoration(
            color: c.tile,
            borderRadius: BorderRadius.circular(12),
            boxShadow: [BoxShadow(color: c.border, spreadRadius: 1)],
          ),
          child: Stack(
            children: [
              const Positioned(left: 10, top: 10, child: _LivePill()),
              Center(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!small) ...[
                        Avatar(avatar: person.avatar, size: 64, id: person.id),
                        const SizedBox(height: 14),
                      ],
                      Text(
                        small ? person.name : l10n.roomIsSharing(person.name),
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, weight: w600),
                      ),
                      if (!small)
                        Text(
                          l10n.roomNothingSentUntilWatch,
                          textAlign: TextAlign.center,
                          style: tx(c.fg2, size: 13),
                        ),
                      SizedBox(height: small ? 8 : 14),
                      FgButton(
                        text: l10n.roomWatch,
                        icon: Ic.screen,
                        kind: Kind.primary,
                        height: small ? 30 : 36,
                        onTap: onWatch,
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// This device's own share, in the tiles: that it is shared and to how many, without its
/// picture. Drawn live, the picture showed in the window shared whenever that was Fogata's
/// (or the screen under it), was captured again with it, and so on without end: every frame
/// changed, and the share went out at a few frames a second. Under Wayland nothing can
/// leave Fogata's window out of what the system's dialog shares, so the picture shows only
/// when asked, enlarged, with a word of that.
class _OwnShareCard extends StatelessWidget {
  const _OwnShareCard({required this.watchers, required this.onPreview});
  final int watchers;
  final VoidCallback onPreview;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return LayoutBuilder(
      builder: (context, box) {
        // Small in the strip, or in a short tile: the words and the button only.
        final small = box.maxHeight < 200 || box.maxWidth < 200;
        return Container(
          decoration: BoxDecoration(
            color: c.tile,
            borderRadius: BorderRadius.circular(12),
            boxShadow: [BoxShadow(color: c.border, spreadRadius: 1)],
          ),
          child: Stack(
            children: [
              const Positioned(left: 10, top: 10, child: _LivePill()),
              Center(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!small) ...[
                        FgIcon(Ic.screen, size: 32, color: c.fg2),
                        const SizedBox(height: 12),
                      ],
                      Text(
                        l10n.roomSharingYourScreen,
                        textAlign: TextAlign.center,
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg, weight: w600),
                      ),
                      Text(
                        watchers == 0
                            ? l10n.roomNobodyWatching
                            : l10n.roomPeopleWatching(watchers),
                        overflow: TextOverflow.ellipsis,
                        style: tx(c.fg2, size: 13),
                      ),
                      SizedBox(height: small ? 8 : 14),
                      FgButton(
                        text: l10n.roomShowPreview,
                        icon: Ic.eye,
                        kind: Kind.secondary,
                        height: small ? 30 : 36,
                        onTap: onPreview,
                      ),
                      if (!small) ...[
                        const SizedBox(height: 10),
                        Text(
                          l10n.roomPreviewMirror,
                          textAlign: TextAlign.center,
                          style: tx(c.fg3, size: 12),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _LivePill extends StatelessWidget {
  const _LivePill();

  @override
  Widget build(BuildContext context) => Container(
    // Taller with larger text, so the word is never cut.
    height: heightForText(context, 20, 11),
    padding: const EdgeInsets.symmetric(horizontal: 7),
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: context.fg.redSolid,
      borderRadius: BorderRadius.circular(4),
    ),
    child: Text(
      l10n.roomLive,
      style: tx(Colors.white, size: 11, weight: w600, height: 1, spacing: .4),
    ),
  );
}

/// Someone being rung into the call: their picture in rings, and a way to stop.
class _RingingTile extends StatelessWidget {
  const _RingingTile({required this.person, required this.onCancel});
  final UiPerson person;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      key: ValueKey('calling-tile:${person.id}'),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.border),
      ),
      child: Center(
        child: FittedBox(
          fit: BoxFit.scaleDown,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Rings(
                  color: c.fg3,
                  size: 120,
                  child: Avatar(avatar: person.avatar, size: 72),
                ),
                const SizedBox(height: 6),
                Text(l10n.roomCallingName(person.name), style: tx(c.fg2)),
                const SizedBox(height: 10),
                FgButton(
                  text: l10n.commonCancel,
                  height: 30,
                  fontSize: 13,
                  onTap: onCancel,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
