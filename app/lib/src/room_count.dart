import 'package:flutter/material.dart';

import 'icons.dart';
import 'l10n.dart';
import 'rust/api/types.dart';
import 'theme.dart';
import 'widgets.dart';

/// People in a voice room now.
int peopleInRoom(UiVoiceRoom? v) => v?.room?.participants.length ?? 0;

/// How many people are in a voice room and how many it takes, as "3/8". A full room says
/// so in amber. As a [pill], for the header, with the people icon.
class RoomCount extends StatelessWidget {
  const RoomCount({
    super.key,
    required this.count,
    required this.max,
    this.pill = false,
    this.fullWord = true,
  });

  final int count;
  final int max;
  final bool pill;

  /// Whether a full room also says "Full". Without it only the count turns amber: for
  /// the room this device is in, whose name already takes the room beside it.
  final bool fullWord;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final full = count >= max;
    final label = full
        ? l10n.roomCountFullLabel(count, max)
        : l10n.roomCountLabel(count, max);
    if (pill) {
      final color = full ? c.amber : c.fg2;
      return Semantics(
        label: label,
        excludeSemantics: true,
        child: Pill(
          text: full ? l10n.roomCountFullPrefix : '',
          monoTail: '$count/$max',
          color: color,
          background: full ? c.amberSoft : c.hover,
          leading: FgIcon(Ic.users, size: 12, color: color),
        ),
      );
    }
    return Semantics(
      label: label,
      excludeSemantics: true,
      child: Text.rich(
        TextSpan(
          children: [
            if (full && fullWord)
              TextSpan(
                text: l10n.roomCountFullWord,
                style: tx(c.amber, size: 12, weight: w500),
              ),
            TextSpan(
              text: '$count/$max',
              style: tx(
                full ? c.amber : (count > 0 ? c.fg2 : c.fg3),
                size: 12,
                monospace: true,
              ),
            ),
          ],
        ),
        maxLines: 1,
        softWrap: false,
      ),
    );
  }
}
