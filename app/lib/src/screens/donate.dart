import 'package:flutter/material.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../report.dart' show openExternal;
import '../rust/api/types.dart' show UiAvatar;
import '../theme.dart';
import '../widgets.dart';

/// The page donations go through.
const kofiUrl = 'https://ko-fi.com/holypandadev';

/// Settings › Donate to Fogata: the person's own picture, why a donation helps, and a
/// button that opens the donation page in the browser.
class DonatePage extends StatelessWidget {
  const DonatePage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, const {Contacts});
    final c = context.fg;
    final person = state.contacts.profile?.person;
    return LayoutBuilder(
      builder: (context, box) => SingleChildScrollView(
        child: ConstrainedBox(
          constraints: BoxConstraints(minHeight: box.maxHeight),
          child: Align(
            // A little above the middle, as the page's title is not shown.
            alignment: const Alignment(0, -.16),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Container(
                      width: 96,
                      height: 96,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        border: Border.all(color: c.borderStrong, width: 2),
                      ),
                      child: Avatar(
                        avatar: person?.avatar ?? const UiAvatar(preset: 0),
                        size: 92,
                        id: person?.id,
                      ),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      l10n.donateTitle,
                      textAlign: TextAlign.center,
                      style: tx(
                        c.fg,
                        size: 22,
                        weight: FontWeight.w700,
                        height: 1.3,
                        spacing: -.22,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      l10n.donateText,
                      textAlign: TextAlign.center,
                      style: tx(c.fg2, height: 1.45),
                    ),
                    const SizedBox(height: 22),
                    FgButton(
                      key: const ValueKey('donate'),
                      text: l10n.donateButton,
                      icon: Ic.heart,
                      iconColor: c.red,
                      kind: Kind.primary,
                      height: 42,
                      fontSize: 14.5,
                      padding: const EdgeInsets.symmetric(horizontal: 26),
                      onTap: () => openExternal(Uri.parse(kofiUrl)),
                    ),
                    const SizedBox(height: 14),
                    Text(
                      l10n.donateNote,
                      textAlign: TextAlign.center,
                      style: tx(c.fg3, size: 12),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
