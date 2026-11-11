import 'dart:async';

import 'package:flutter/material.dart';

import '../l10n.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import '../theme.dart';

/// Why others may not reach this computer, in plain words: this computer's firewall
/// letting nothing in ([firewall]) and each cause the node saw on this side ([reach]).
/// With nothing in the way here right now, that the trouble is likely on the other side:
/// [friend]'s (offline, or their network), or, for a room or a call, the others'.
/// Nothing when nothing is known: a guess would send the person after the wrong thing.
///
/// Without [fine], only what is in the way on this side is said: where the place already
/// says that the other one is offline, as a friend's card does, "the trouble is likely on
/// their side" adds nothing to act on.
List<String> reachTexts(
  UiReach? reach, {
  required bool firewall,
  String? friend,
  bool fine = true,
}) {
  final said = [
    if (firewall) l10n.reachFirewall,
    for (final cause in reach?.causes ?? const <UiCause>[])
      switch (cause) {
        UiCause.offline => l10n.reachOffline,
        UiCause.udpBlocked => l10n.reachUdpBlocked,
        UiCause.symmetricNat => l10n.reachSymmetricNat,
        UiCause.noIpv6 => l10n.reachNoIpv6,
      },
  ];
  if (fine && said.isEmpty && (reach?.fine ?? false)) {
    said.add(
      friend == null ? l10n.reachFineOthers : l10n.reachFineFriend(friend),
    );
  }
  return said;
}

/// [reachTexts] where a connection failed: a friend's card while the friend is offline,
/// a room or a call that could not connect. It asks the core what its node has seen when
/// it shows ([load]), which takes no wait, and again every [every] while it shows: the
/// network may change meanwhile, and "fine" holds only while it is seen.
class WhyUnreached extends StatefulWidget {
  const WhyUnreached({
    super.key,
    required this.firewall,
    this.friend,
    this.load = api.reachability,
    this.fine = true,
    this.center = false,
    this.size = 12,
    this.top = 0,
    this.every = const Duration(seconds: 10),
  });

  /// This computer's own firewall lets nothing in for Fogata (the bell has the command).
  final bool firewall;

  /// The friend that is not reached, when it is one friend.
  final String? friend;
  final Future<UiReach> Function() load;

  /// Whether to say, with nothing in the way here, that the trouble is likely on the
  /// other side (see [reachTexts]).
  final bool fine;
  final bool center;
  final double size;

  /// The space above, when there is something to say.
  final double top;

  /// How often to ask again while shown.
  final Duration every;

  @override
  State<WhyUnreached> createState() => _WhyUnreachedState();
}

class _WhyUnreachedState extends State<WhyUnreached> {
  UiReach? _reach;
  Timer? _again;

  @override
  void initState() {
    super.initState();
    _ask();
    _again = Timer.periodic(widget.every, (_) => _ask());
  }

  @override
  void dispose() {
    _again?.cancel();
    super.dispose();
  }

  Future<void> _ask() async {
    UiReach? reach;
    try {
      reach = await widget.load();
    } catch (_) {
      // A core that cannot say leaves only what is known here (the firewall): what it
      // said before may be old now.
    }
    if (mounted) setState(() => _reach = reach);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final said = reachTexts(
      _reach,
      firewall: widget.firewall,
      friend: widget.friend,
      fine: widget.fine,
    );
    if (said.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: EdgeInsets.only(top: widget.top),
      child: Column(
        key: const ValueKey('why-unreached'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: widget.center
            ? CrossAxisAlignment.center
            : CrossAxisAlignment.start,
        children: [
          for (var i = 0; i < said.length; i++) ...[
            if (i > 0) const SizedBox(height: 6),
            Text(
              said[i],
              textAlign: widget.center ? TextAlign.center : TextAlign.start,
              style: tx(c.fg2, size: widget.size, height: 1.4),
            ),
          ],
        ],
      ),
    );
  }
}
