import 'package:flutter/material.dart';

import '../icons.dart';
import '../l10n.dart';
import '../restart.dart';
import '../theme.dart';
import '../widgets.dart';

/// Under the header while the network part is stopped: nothing reaches this device until
/// the application starts again, which it does by itself once, or with the button.
class StoppedBanner extends StatelessWidget {
  const StoppedBanner({super.key, required this.stopping});
  final Stopping stopping;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: stopping,
    builder: (context, _) {
      if (!stopping.stopped) return const SizedBox.shrink();
      final c = context.fg;
      final then =
          stopping.failure ??
          (stopping.restarting
              ? l10n.stoppedStartingAgain
              : l10n.stoppedRestartToConnect);
      return Padding(
        padding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
        child: Container(
          padding: const EdgeInsets.fromLTRB(14, 8, 8, 8),
          decoration: BoxDecoration(
            color: c.redSoft,
            borderRadius: BorderRadius.circular(999),
            border: Border.all(color: c.redLine),
          ),
          child: Row(
            children: [
              FgIcon(Ic.warning, size: 16, color: c.red),
              const SizedBox(width: 10),
              Expanded(
                child: Text.rich(
                  TextSpan(
                    children: [
                      TextSpan(
                        text: l10n.stoppedTitle,
                        style: const TextStyle(fontWeight: w600),
                      ),
                      TextSpan(text: ' $then'),
                    ],
                  ),
                  style: tx(c.red),
                ),
              ),
              const SizedBox(width: 10),
              FgButton(
                text: l10n.stoppedRestartNow,
                icon: Ic.retry,
                height: 30,
                fontSize: 13,
                kind: Kind.danger,
                busy: stopping.starting,
                onTap: () => stopping.restart(),
              ),
            ],
          ),
        ),
      );
    },
  );
}
