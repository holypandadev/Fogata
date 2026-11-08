import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../app_state.dart';
import '../icons.dart';
import '../l10n.dart';
import '../names.dart';
import '../moving.dart';
import '../theme.dart';
import '../widgets.dart';
import 'group_banner.dart';
import 'identity_preview.dart';

/// Lets the person pick the group's picture from the computer; tests put their own here.
@visibleForTesting
Future<Uint8List?> Function() pickGroupPicture = pickGroupPhoto;

Future<void> showNewGroup(BuildContext context) => showFgDialog(
  context,
  maxWidth: 640,
  builder: (_) =>
      AppScope(state: AppScope.read(context), child: const _NewGroup()),
);

class _NewGroup extends StatefulWidget {
  const _NewGroup();

  @override
  State<_NewGroup> createState() => _NewGroupState();
}

class _NewGroupState extends State<_NewGroup> {
  final _name = TextEditingController();
  final _search = TextEditingController();
  int _icon = 0;

  /// A picture from the computer, shown instead of the icon.
  Uint8List? _picture;

  /// The banner at the top of the group's column, if one was chosen.
  Uint8List? _banner;
  bool _picking = false;
  bool _pickingBanner = false;
  final Set<String> _chosen = {};
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _name.addListener(() => setState(() {}));
    _search.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _name.dispose();
    _search.dispose();
    super.dispose();
  }

  Future<void> _pick() async {
    final state = AppScope.read(context);
    setState(() => _picking = true);
    try {
      final picture = await pickGroupPicture();
      if (picture != null && mounted) setState(() => _picture = picture);
    } on PictureProblem catch (e) {
      state.say(e.message, error: true);
    } catch (_) {
      state.say(l10n.newGroupPictureUnreadable, error: true);
    }
    if (mounted) setState(() => _picking = false);
  }

  Future<void> _pickBanner() async {
    setState(() => _pickingBanner = true);
    final banner = await chooseGroupBanner(AppScope.read(context));
    if (!mounted) return;
    setState(() {
      _pickingBanner = false;
      if (banner != null) _banner = banner;
    });
  }

  Future<void> _create() async {
    final state = AppScope.read(context);
    setState(() => _busy = true);
    final name = tidyName(_name.text);
    final id = await state.createGroup(name, _icon, _chosen.toList());
    // The picture goes into the new group's log right after it; the icon stays under it.
    final picture = _picture;
    if (id != null && picture != null) {
      await state.setGroupPicture(id, picture);
    }
    final banner = _banner;
    if (id != null && banner != null) await state.setGroupBanner(id, banner);
    if (!mounted) return;
    setState(() => _busy = false);
    if (id != null) {
      Navigator.of(context).pop();
      state.say(l10n.newGroupMade(name));
      state.goHome();
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final name = tidyName(_name.text);
    final query = _search.text.trim().toLowerCase();
    final friends = state.contacts.friends
        .where(
          (f) => query.isEmpty || f.person.name.toLowerCase().contains(query),
        )
        .toList();
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Flexible(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            l10n.newGroupTitle,
                            style: hx(c.fg, size: 20, height: 1.3),
                          ),
                          const SizedBox(height: 4),
                          Text(l10n.newGroupSubtitle, style: tx(c.fg2)),
                        ],
                      ),
                    ),
                    FgButton(
                      icon: Ic.close,
                      kind: Kind.ghost,
                      height: 32,
                      tooltip: l10n.commonClose,
                      onTap: () => Navigator.of(context).pop(),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                IdentityPreview(
                  banner: _banner == null
                      ? const EmptyBanner()
                      : BannerPicture(key: ValueKey(_banner), bytes: _banner!),
                  picture: GroupBadge(
                    icon: _icon,
                    picture: _picture,
                    size: IdentityPreview.pictureSize,
                    radius: IdentityPreview.pictureSize / 2,
                    iconSize: 32,
                  ),
                  title: name.isEmpty ? l10n.newGroupNameExample : name,
                  titleMuted: name.isEmpty,
                  below: Padding(
                    padding: const EdgeInsets.only(top: 2),
                    child: Text(
                      l10n.homeMembers(_chosen.length + 1),
                      style: tx(c.fg3, size: 13),
                    ),
                  ),
                  onPicture: _pick,
                  pictureTooltip: l10n.newGroupChooseFromComputer,
                  pictureBusy: _picking,
                  bannerActions: [
                    OverPill(
                      icon: Ic.image,
                      text: _banner == null
                          ? l10n.groupBannerChoose
                          : l10n.groupBannerAnother,
                      busy: _pickingBanner,
                      onTap: _pickBanner,
                    ),
                    if (_banner != null)
                      OverPill(
                        icon: Ic.close,
                        tooltip: l10n.groupBannerNone,
                        onTap: () => setState(() => _banner = null),
                      ),
                  ],
                ),
                const SizedBox(height: 22),
                Text(
                  l10n.newGroupName,
                  style: tx(c.fg2, size: 13, weight: w500),
                ),
                const SizedBox(height: 8),
                FgField(
                  controller: _name,
                  autofocus: true,
                  maxLength: 60,
                  placeholder: l10n.newGroupNameExample,
                  label: l10n.newGroupName,
                ),
                NameNote(_name.text),
                const SizedBox(height: 20),
                Text(
                  l10n.newGroupIconOrPicture,
                  style: tx(c.fg2, size: 13, weight: w500),
                ),
                const SizedBox(height: 8),
                GroupIconRow(
                  // A picture from the computer shows instead of any icon.
                  selected: _picture == null ? _icon : null,
                  onPick: (i) => setState(() {
                    _icon = i;
                    _picture = null;
                  }),
                ),
                const SizedBox(height: 22),
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        l10n.newGroupMembers,
                        style: tx(c.fg2, size: 13, weight: w500),
                      ),
                    ),
                    Text(
                      l10n.newGroupSelected(_chosen.length),
                      style: tx(c.fg3, size: 13),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Container(
                  clipBehavior: Clip.antiAlias,
                  // Inside the outline, as when it was drawn under what is inside.
                  padding: const EdgeInsets.all(1),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                  ),
                  // Over what is inside, which would cover it at the rounded corners.
                  foregroundDecoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: c.border),
                  ),
                  child: Column(
                    children: [
                      Container(
                        decoration: BoxDecoration(
                          border: Border(bottom: BorderSide(color: c.border)),
                        ),
                        child: FgField(
                          controller: _search,
                          borderless: true,
                          placeholder: l10n.newGroupSearchPlaceholder,
                          leading: Padding(
                            padding: const EdgeInsets.only(left: 2),
                            child: FgIcon(Ic.search, size: 14, color: c.fg3),
                          ),
                          label: l10n.newGroupSearchLabel,
                        ),
                      ),
                      ConstrainedBox(
                        constraints: const BoxConstraints(maxHeight: 264),
                        child: state.contacts.friends.isEmpty
                            ? Padding(
                                padding: const EdgeInsets.all(16),
                                child: Text(
                                  l10n.newGroupNoFriends,
                                  style: tx(c.fg3, size: 13),
                                ),
                              )
                            : ListView(
                                shrinkWrap: true,
                                children: [
                                  for (final f in friends)
                                    Pressable(
                                      onTap: () => setState(() {
                                        if (!_chosen.remove(f.person.id)) {
                                          _chosen.add(f.person.id);
                                        }
                                      }),
                                      builder: (context, hovered) {
                                        final on = _chosen.contains(
                                          f.person.id,
                                        );
                                        return Container(
                                          height: 44,
                                          color: hovered ? c.hover : null,
                                          padding: const EdgeInsets.symmetric(
                                            horizontal: 12,
                                          ),
                                          child: Row(
                                            children: [
                                              FgCheck(on),
                                              const SizedBox(width: 10),
                                              Avatar(
                                                avatar: f.person.avatar,
                                                size: 24,
                                              ),
                                              const SizedBox(width: 10),
                                              Expanded(
                                                child: Text(
                                                  f.person.name,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                  style: tx(
                                                    c.fg,
                                                    weight: on
                                                        ? w500
                                                        : FontWeight.w400,
                                                  ),
                                                ),
                                              ),
                                              const SizedBox(width: 12),
                                              Text(
                                                f.person.short,
                                                style: tx(
                                                  c.fg3,
                                                  size: 12,
                                                  monospace: true,
                                                ),
                                              ),
                                            ],
                                          ),
                                        );
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
        ),
        Footer(
          children: [
            Expanded(
              child: Text(
                l10n.newGroupKeptOnDevices,
                style: tx(c.fg3, size: 13),
              ),
            ),
            const SizedBox(width: 10),
            FgButton(
              text: l10n.commonCancel,
              onTap: () => Navigator.of(context).pop(),
            ),
            const SizedBox(width: 10),
            FgButton(
              text: l10n.newGroupCreate,
              kind: Kind.primary,
              busy: _busy,
              onTap:
                  tidyName(_name.text).isEmpty ||
                      nameProblem(_name.text) != null
                  ? null
                  : _create,
            ),
          ],
        ),
      ],
    );
  }
}
