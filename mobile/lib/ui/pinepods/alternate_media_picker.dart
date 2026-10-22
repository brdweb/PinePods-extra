import 'package:flutter/material.dart';
import 'package:pinepods_mobile/core/alternate_media.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';

/// Dropdown of RSS and alternate enclosures. Unplayable sources stay visible
/// and disabled so IPFS and torrent links are not a dead end.
class AlternateMediaPicker extends StatelessWidget {
  final MediaPlan plan;
  final ValueChanged<String>? onSelected;

  const AlternateMediaPicker({super.key, required this.plan, this.onSelected});

  @override
  Widget build(BuildContext context) {
    if (plan.options.length < 2) return const SizedBox.shrink();
    final l = AppLocalizations.of(context);
    final selected = plan.options.any((o) => o.id == plan.selectedId) ? plan.selectedId : plan.options.first.id;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l?.media_choose_version ?? 'Choose a version',
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 8),
        DropdownButton<String>(
          isExpanded: true,
          value: selected,
          items: [
            for (final option in plan.options)
              DropdownMenuItem(
                value: option.id,
                enabled: option.playable,
                child: Text(
                  mediaOptionLabel(
                    option,
                    original: l?.media_original ?? 'Original',
                    audio: l?.media_audio ?? 'Audio',
                    video: l?.media_video ?? 'Video',
                    hls: l?.media_hls ?? 'HLS stream',
                    dash: l?.media_dash ?? 'DASH stream',
                    other: l?.media_other ?? 'Other',
                    ipfs: l?.media_ipfs ?? 'IPFS — shown only',
                    torrent: l?.media_torrent ?? 'Torrent — shown only',
                    unavailable: l?.media_cannot_play ?? 'Cannot play on this device',
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
          onChanged: (id) {
            if (id == null) return;
            final option = plan.options.cast<MediaOption?>().firstWhere((o) => o?.id == id, orElse: () => null);
            if (option == null || !option.playable) return;
            onSelected?.call(id);
          },
        ),
        if (plan.selected?.integrity?.verifiable == true)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l?.media_integrity ?? 'Integrity hash is checked when the file is downloaded',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        if (plan.selected != null && plan.selected!.playable && !plan.selected!.downloadable)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l?.media_stream_only ?? 'This version is a stream and cannot be saved as a file',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
      ],
    );
  }
}
