import 'package:flutter/material.dart';

import '../app_state.dart';
import '../direct_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/direct.dart';
import '../theme.dart';
import '../widgets.dart';
import 'direct.dart' show UsageBar, filesColor, storageSize;

/// Settings, storage: what direct messages take on this computer, the room they may take,
/// what comes by itself, the timer of new chats, and each chat's files to clear.
class StorageSettings extends StatefulWidget {
  const StorageSettings({super.key});

  @override
  State<StorageSettings> createState() => _StorageSettingsState();
}

class _StorageSettingsState extends State<StorageSettings> {
  /// Opened from a chat's "Free up space…": the settings go straight to here once it
  /// shows.
  bool _bringIntoView = false;

  @override
  void initState() {
    super.initState();
    final direct = AppScope.read(context).direct;
    direct.loadStorage();
    _bringIntoView = direct.storageOnOpen;
    direct.storageOnOpen = false;
  }

  Future<void> _pickLimit(UiDirectStorage storage, int limit) async {
    final direct = AppScope.read(context).direct;
    final s = storage.settings;
    if (limit == s.limit) return;
    final used = _counted(storage.total);
    if (limit != 0 && used > limit) {
      final yes = await showFgDialog<bool>(
        context,
        maxWidth: 440,
        builder: (context) => _ConfirmLimit(limit: limit),
      );
      if (!(yes ?? false)) return;
    }
    await direct.setSettings(
      UiDirectSettings(
        limit: limit,
        autoPictures: s.autoPictures,
        autoFiles: s.autoFiles,
        timerMs: s.timerMs,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final direct = state.direct;
    final storage = direct.storage;
    if (storage == null) return const SizedBox.shrink();
    if (_bringIntoView) {
      _bringIntoView = false;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          Scrollable.ensureVisible(
            context,
            duration: springTime,
            curve: Curves.easeOut,
          );
        }
      });
    }
    final s = storage.settings;
    final total = storage.total;
    final used = _counted(total);
    final scale = s.limit == 0 ? 2000000000 : s.limit;
    final over = s.limit != 0 && used > s.limit;
    Widget divider() => Padding(
      padding: const EdgeInsets.symmetric(vertical: 14),
      child: Container(height: 1, color: c.border),
    );
    Widget legend(String text, Color color) => Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Dot(color, size: 8),
        const SizedBox(width: 6),
        Text(text, style: tx(c.fg2, size: 12)),
      ],
    );
    Widget toggle(String label, String note, bool on, ValueChanged<bool> set) =>
        Pressable(
          onTap: () => set(!on),
          label: on
              ? l10n.settingsSwitchOn(label)
              : l10n.settingsSwitchOff(label),
          focusRadius: 10,
          builder: (context, hovered) => Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(label, style: tx(c.fg, weight: w500)),
                      Text(note, style: tx(c.fg3, size: 12)),
                    ],
                  ),
                ),
                const SizedBox(width: 14),
                FgSwitch(on: on),
              ],
            ),
          ),
        );
    UiDirectSettings changed({bool? pictures, bool? files, int? timer}) =>
        UiDirectSettings(
          limit: s.limit,
          autoPictures: pictures ?? s.autoPictures,
          autoFiles: files ?? s.autoFiles,
          timerMs: timer ?? s.timerMs,
        );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Panel(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  l10n.storageTitle,
                  style: tx(c.fg, size: 16, weight: w600, spacing: -.16),
                ),
                const SizedBox(height: 4),
                Text(l10n.storageNote, style: tx(c.fg2, size: 13)),
                const SizedBox(height: 18),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.baseline,
                  textBaseline: TextBaseline.alphabetic,
                  children: [
                    Text(l10n.storageDirect, style: tx(c.fg, weight: w600)),
                    const Spacer(),
                    Text(
                      storageSize(used),
                      style: tx(c.fg, size: 13, monospace: true),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      s.limit == 0
                          ? l10n.storageOfNoLimit
                          : l10n.storageOfLimit(limitWord(s.limit)),
                      style: tx(c.fg3, size: 13),
                    ),
                  ],
                ),
                const SizedBox(height: 14),
                UsageBar(
                  parts: [
                    (total.pictures, c.blue),
                    (total.videos, c.amber),
                    (total.files, filesColor),
                    (total.messages, c.fg),
                  ],
                  total: used > scale ? used : scale,
                ),
                const SizedBox(height: 14),
                Wrap(
                  spacing: 18,
                  runSpacing: 6,
                  children: [
                    legend(
                      l10n.storageLegendPictures(storageSize(total.pictures)),
                      c.blue,
                    ),
                    legend(
                      l10n.storageLegendVideos(storageSize(total.videos)),
                      c.amber,
                    ),
                    legend(
                      l10n.storageLegendFiles(storageSize(total.files)),
                      filesColor,
                    ),
                    legend(
                      l10n.storageLegendMessages(storageSize(total.messages)),
                      c.fg,
                    ),
                  ],
                ),
                divider(),
                Text(l10n.storageRoom, style: tx(c.fg, weight: w500)),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Segmented(
                    key: const ValueKey('storage-limit'),
                    height: 38,
                    tabs: [
                      for (final l in storageLimits) (limitWord(l), null, null),
                    ],
                    selected: storageLimits.indexOf(s.limit),
                    onSelect: (i) => _pickLimit(storage, storageLimits[i]),
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  s.limit == 0
                      ? l10n.storageNoLimitNote
                      : over
                      ? l10n.storageOverNote(limitWord(s.limit))
                      : l10n.storageLimitNote,
                  style: tx(c.fg3, size: 12),
                ),
                divider(),
                toggle(
                  l10n.storageAutoPictures,
                  l10n.storageAutoPicturesNote,
                  s.autoPictures,
                  (on) => direct.setSettings(changed(pictures: on)),
                ),
                const SizedBox(height: 12),
                toggle(
                  l10n.storageAutoFiles,
                  l10n.storageAutoFilesNote,
                  s.autoFiles,
                  (on) => direct.setSettings(changed(files: on)),
                ),
                divider(),
                Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            l10n.storageTimer,
                            style: tx(c.fg, weight: w500),
                          ),
                          Text(
                            l10n.storageTimerNote,
                            style: tx(c.fg3, size: 12),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 14),
                    Segmented(
                      key: const ValueKey('storage-timer'),
                      height: 34,
                      tabs: [
                        for (final t in timers)
                          (
                            t == dayMs ? l10n.storageTimer24h : timerWord(t),
                            null,
                            null,
                          ),
                      ],
                      selected: timers.indexOf(s.timerMs),
                      onSelect: (i) =>
                          direct.setSettings(changed(timer: timers[i])),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
        if (storage.chats.isNotEmpty) ...[
          const SizedBox(height: 16),
          Panel(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 20, 20, 8),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l10n.storageByChat, style: tx(c.fg, weight: w600)),
                  const SizedBox(height: 2),
                  Text(l10n.storageByChatNote, style: tx(c.fg3, size: 12)),
                  const SizedBox(height: 8),
                  for (final chat in storage.chats)
                    _ChatUsage(
                      chat: chat,
                      cleared: direct.cleared.contains(chat.friend.id),
                    ),
                ],
              ),
            ),
          ),
        ],
      ],
    );
  }
}

int _attachments(UiDirectUsage u) => u.pictures + u.videos + u.files;

/// What the room for direct messages counts: the attachments and the messages, which
/// always stay.
int _counted(UiDirectUsage u) => _attachments(u) + u.messages;

class _ChatUsage extends StatelessWidget {
  const _ChatUsage({required this.chat, required this.cleared});
  final UiDirectChatUsage chat;
  final bool cleared;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    final files = _attachments(chat.usage);
    return Container(
      constraints: const BoxConstraints(minHeight: 52),
      padding: const EdgeInsets.symmetric(vertical: 4),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: c.border)),
      ),
      child: Row(
        children: [
          Avatar(avatar: chat.friend.avatar, size: 30),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  chat.friend.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, weight: w500, height: 1.3),
                ),
                Text(
                  files == 0 ? l10n.storageMessagesOnly : storageSize(files),
                  style: tx(c.fg3, size: 12, monospace: true, height: 1.3),
                ),
              ],
            ),
          ),
          if (files > 0)
            FgButton(
              key: ValueKey('clear-files:${chat.friend.id}'),
              text: l10n.storageClearFiles,
              height: 30,
              fontSize: 12,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              tooltip: l10n.storageClearFilesTooltip(chat.friend.name),
              onTap: () => state.direct.clearFiles(chat.friend.id),
            )
          else if (cleared)
            Container(
              height: 30,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              decoration: BoxDecoration(
                color: c.greenSoft,
                borderRadius: BorderRadius.circular(999),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  FgIcon(Ic.check, size: 12, stroke: 2.6, color: c.green),
                  const SizedBox(width: 6),
                  Text(
                    l10n.storageCleared,
                    style: tx(c.green, size: 12, weight: w500, height: 1.2),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _ConfirmLimit extends StatelessWidget {
  const _ConfirmLimit({required this.limit});
  final int limit;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.storageConfirmTitle(limitWord(limit)),
                style: tx(c.fg, size: 16, weight: w600),
              ),
              const SizedBox(height: 6),
              Text(
                l10n.storageConfirmText(limitWord(limit)),
                style: tx(c.fg2, size: 13),
              ),
            ],
          ),
        ),
        Footer(
          children: [
            const Spacer(),
            FgButton(
              text: l10n.commonCancel,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(false),
            ),
            const SizedBox(width: 8),
            FgButton(
              key: const ValueKey('confirm-limit'),
              text: l10n.storageRemoveOldest,
              kind: Kind.danger,
              height: 32,
              fontSize: 13,
              onTap: () => Navigator.of(context).pop(true),
            ),
          ],
        ),
      ],
    );
  }
}
