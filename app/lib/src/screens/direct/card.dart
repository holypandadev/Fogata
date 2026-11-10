part of '../direct.dart';

// The friend's card beside a wide chat: who they are, how they are, and what the chat
// takes on this computer.

class _Card extends StatelessWidget {
  const _Card({required this.page});
  final UiDirectPage page;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.only(context, directParts);
    final c = context.fg;
    final friend = page.friend;
    final banner = state.direct.banners[friend.id];
    final online = page.online && page.endedMs == null;
    final usage = page.usage;
    final rows = [
      (l10n.directUsageMessages, usage.messages, c.fg),
      (l10n.directUsagePictures, usage.pictures, c.blue),
      (l10n.directUsageVideos, usage.videos, c.amber),
      (l10n.directUsageFiles, usage.files, _violet),
    ];
    final total = rows.fold<int>(0, (n, r) => n + r.$2);
    return Container(
      width: 300,
      decoration: BoxDecoration(
        color: c.bg1,
        border: Border(left: BorderSide(color: c.border)),
      ),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Stack(
              clipBehavior: Clip.none,
              children: [
                SizedBox(
                  height: 84,
                  child: PanelBanner(
                    color: banner == null ? c.hover : Color(banner.color),
                    picture: banner?.picture,
                    id: friend.id,
                  ),
                ),
                Positioned(
                  left: 16,
                  top: 48,
                  child: Container(
                    width: 76,
                    height: 76,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: c.bg1,
                      shape: BoxShape.circle,
                    ),
                    child: Avatar(
                      avatar: friend.avatar,
                      size: 66,
                      dim: !online,
                      id: friend.id,
                    ),
                  ),
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 50, 16, 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    friend.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: tx(c.fg, size: 18, weight: w600, height: 1.25),
                  ),
                  Text(
                    friend.short,
                    style: tx(c.fg3, size: 12, monospace: true),
                  ),
                  // A former friend's bio is not known any longer.
                  if (state.contacts.friends
                          .where((f) => f.person.id == friend.id)
                          .firstOrNull
                      case final f?)
                    CardBio(f.bio),
                  const SizedBox(height: 14),
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: c.bg2,
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: c.border),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Text(
                          l10n.directUsageHeading,
                          style: tx(c.fg3, size: 11, weight: w500, spacing: .4),
                        ),
                        const SizedBox(height: 10),
                        _UsageBar(
                          parts: [for (final r in rows) (r.$2, r.$3)],
                          total: total,
                          height: 6,
                        ),
                        const SizedBox(height: 10),
                        for (final (label, size, color) in rows) ...[
                          Row(
                            children: [
                              Dot(color, size: 8),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(label, style: tx(c.fg2, size: 13)),
                              ),
                              Text(
                                storageSize(size),
                                style: tx(c.fg, size: 12, monospace: true),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                        ],
                        const SizedBox(height: 2),
                        FgButton(
                          key: const ValueKey('direct-free'),
                          text: l10n.directFreeUpSpace,
                          height: 32,
                          fontSize: 13,
                          expand: true,
                          onTap: () {
                            state.direct.storageOnOpen = true;
                            state.openSettings();
                          },
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A bar split by what takes the space, each part in its colour.
class _UsageBar extends StatelessWidget {
  const _UsageBar({
    required this.parts,
    required this.total,
    required this.height,
  });
  final List<(int, Color)> parts;

  /// What the whole bar stands for.
  final int total;
  final double height;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return ClipRRect(
      borderRadius: BorderRadius.circular(999),
      child: Container(
        height: height,
        color: c.border,
        child: LayoutBuilder(
          builder: (context, box) => Row(
            children: [
              for (final (size, color) in parts)
                if (size > 0 && total > 0)
                  AnimatedContainer(
                    duration: const Duration(milliseconds: 450),
                    curve: spring,
                    width: box.maxWidth * math.min(1, size / total),
                    color: color,
                  ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The bar of what direct messages take, for the settings: the same parts and colours.
class UsageBar extends StatelessWidget {
  const UsageBar({
    super.key,
    required this.parts,
    required this.total,
    this.height = 10,
  });
  final List<(int, Color)> parts;
  final int total;
  final double height;

  @override
  Widget build(BuildContext context) =>
      _UsageBar(parts: parts, total: total, height: height);
}

/// The colour of files in what direct messages take.
const filesColor = _violet;
