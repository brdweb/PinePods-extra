import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/podcasting2_tags.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';
import 'package:url_launcher/url_launcher.dart';

/// Episode-level Podcasting 2.0 details: season, clips, links, location, chat, comments.
class EpisodePodcasting2Panel extends StatelessWidget {
  final ItemTags? tags;
  final int? itunesSeason;
  final int? itunesEpisode;
  final List<P2Chapter> chapters;
  final void Function(ClipWindow window)? onPlayClip;
  final void Function(P2Chapter chapter)? onOpenChapter;

  const EpisodePodcasting2Panel({
    super.key,
    required this.tags,
    this.itunesSeason,
    this.itunesEpisode,
    this.chapters = const [],
    this.onPlayClip,
    this.onOpenChapter,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = L.of(context)!;
    final item = tags ?? const ItemTags();
    final numbers = resolveNumbers(
      season: item.season,
      episode: item.episode,
      itunesSeason: itunesSeason,
      itunesEpisode: itunesEpisode,
    );
    final badge = formatNumbersBadge(l10n.p2_season, l10n.p2_episode, numbers);
    final links = item.contentLinks
        .where((link) => safeLink(link.href) != null)
        .toList();
    final socials = item.socialInteracts
        .where((social) => social.link != null)
        .toList();
    final splits = timeSplitsOf(item.value);
    final children = <Widget>[
      if (badge != null) _Badge(badge),
      if ((item.medium ?? '').trim().isNotEmpty)
        _section(
          context,
          l10n.p2_medium,
          Text(_mediumLabel(l10n, item.medium!)),
        ),
      if (item.soundbites.isNotEmpty)
        _section(
          context,
          l10n.p2_soundbites,
          Column(
            children: [
              for (var i = 0; i < item.soundbites.length; i++)
                if (item.soundbites[i].window != null)
                  _ClipRow(
                    title: (item.soundbites[i].title ?? '').trim().isEmpty
                        ? l10n.p2_clip_n('${i + 1}')
                        : item.soundbites[i].title!,
                    meta:
                        '${formatClock(item.soundbites[i].window!.start)} · ${formatClock(item.soundbites[i].window!.duration)}',
                    tooltip: l10n.p2_play_clip,
                    onPlay: onPlayClip == null
                        ? null
                        : () => onPlayClip!(item.soundbites[i].window!),
                  ),
            ],
          ),
        ),
      if (chapters.isNotEmpty)
        _section(
          context,
          l10n.p2_chapters,
          Column(
            children: [
              for (final chapter in chapters)
                _ChapterRow(
                  chapter: chapter,
                  linkLabel: l10n.p2_chapter_link,
                  onOpen: onOpenChapter,
                ),
            ],
          ),
        ),
      if (links.isNotEmpty)
        _section(
          context,
          l10n.p2_links,
          Column(
            children: [
              for (final link in links)
                _LinkRow(
                  url: safeLink(link.href)!,
                  text: link.text.trim().isEmpty ? link.href : link.text,
                ),
            ],
          ),
        ),
      if (item.locations.isNotEmpty)
        _section(
          context,
          l10n.p2_location,
          Column(
            children: [
              for (final location in item.locations)
                _LocationRow(location: location, mapLabel: l10n.p2_open_map),
            ],
          ),
        ),
      if (item.chat?.link != null)
        _section(
          context,
          l10n.p2_chat,
          _LinkRow(url: item.chat!.link!, text: l10n.p2_join_chat),
        ),
      if (socials.isNotEmpty)
        _section(
          context,
          l10n.p2_comments,
          Column(
            children: [
              for (final social in socials)
                _LinkRow(
                  url: social.link!,
                  text: social.protocol.trim().isEmpty
                      ? l10n.p2_comment_thread
                      : social.protocol,
                ),
            ],
          ),
        ),
      if (item.license?.hasContent == true)
        _license(context, l10n, item.license!),
      if (item.funding.isNotEmpty)
        _section(
          context,
          l10n.p2_funding,
          Column(
            children: [
              for (final funding in item.funding)
                if (httpLink(funding.url) != null)
                  _LinkRow(
                    url: funding.url,
                    text: funding.description.trim().isEmpty
                        ? funding.url
                        : funding.description,
                  ),
            ],
          ),
        ),
      if (item.remoteItems.isNotEmpty)
        _section(
          context,
          l10n.p2_related,
          Column(
            children: [
              for (final item in item.remoteItems)
                _RemoteRow(
                  item: item,
                  openLabel: l10n.p2_open_feed,
                  l10n: l10n,
                ),
            ],
          ),
        ),
      if (splits.isNotEmpty)
        _section(
          context,
          l10n.p2_time_splits,
          Column(
            children: [
              for (final split in splits) _SplitRow(split: split, l10n: l10n),
            ],
          ),
        ),
    ];
    if (children.isEmpty) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }
}

/// Show-level Podcasting 2.0 details: schedule, trailers, publisher, location, chat.
class ShowPodcasting2Panel extends StatelessWidget {
  final ChannelTags? tags;
  final Iterable<String> episodeGuids;
  final void Function(Trailer trailer)? onPlayTrailer;
  final void Function(LiveItem item)? onPlayLive;

  const ShowPodcasting2Panel({
    super.key,
    required this.tags,
    this.episodeGuids = const [],
    this.onPlayTrailer,
    this.onPlayLive,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = L.of(context)!;
    final show = tags;
    if (show == null || show.isEmpty) return const SizedBox.shrink();
    final socials = show.socialInteracts
        .where((social) => social.link != null)
        .toList();
    final groups = _trailerGroups(show.trailers);
    final showHeaders = groups.any((group) => group.$1 != null);
    final liveItems = visibleLiveItems(show.liveItems, episodeGuids);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (liveItems.isNotEmpty)
          _section(
            context,
            l10n.p2_live_items,
            Column(
              children: [
                for (final item in liveItems)
                  _ClipRow(
                    title: (item.title ?? '').trim().isEmpty
                        ? _liveStatusLabel(l10n, item.status)
                        : item.title!.trim(),
                    meta: _liveMeta(l10n, item),
                    tooltip: normalizeLiveStatus(item.status) == 'ended'
                        ? l10n.p2_play_recording
                        : l10n.p2_play_live,
                    onPlay: item.playableUrl == null || onPlayLive == null
                        ? null
                        : () => onPlayLive!(item),
                  ),
              ],
            ),
          ),
        if ((show.medium ?? '').trim().isNotEmpty)
          _section(
            context,
            l10n.p2_medium,
            Text(_mediumLabel(l10n, show.medium!)),
          ),
        if (show.updateFrequency != null)
          _schedule(context, l10n, show.updateFrequency!),
        if (show.trailers.isNotEmpty)
          _section(
            context,
            l10n.p2_trailers,
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final group in groups) ...[
                  if (showHeaders)
                    Padding(
                      padding: const EdgeInsets.only(top: 8, bottom: 4),
                      child: Text(
                        group.$1 == null
                            ? l10n.p2_other_episodes
                            : l10n.p2_season_group(group.$1!),
                        style: Theme.of(context).textTheme.labelLarge,
                      ),
                    ),
                  for (final trailer in group.$2)
                    _ClipRow(
                      title: trailer.title.trim().isEmpty
                          ? l10n.p2_trailer
                          : trailer.title,
                      meta: l10n.p2_trailer,
                      tooltip: l10n.p2_play_trailer,
                      onPlay: onPlayTrailer == null
                          ? null
                          : () => onPlayTrailer!(trailer),
                    ),
                ],
              ],
            ),
          ),
        if (show.publisher != null)
          _section(
            context,
            l10n.p2_publisher,
            _RemoteRow(
              item: show.publisher!,
              openLabel: l10n.p2_open_feed,
              l10n: l10n,
            ),
          ),
        if (show.locations.isNotEmpty)
          _section(
            context,
            l10n.p2_location,
            Column(
              children: [
                for (final location in show.locations)
                  _LocationRow(location: location, mapLabel: l10n.p2_open_map),
              ],
            ),
          ),
        if (show.chat?.link != null)
          _section(
            context,
            l10n.p2_chat,
            _LinkRow(url: show.chat!.link!, text: l10n.p2_join_chat),
          ),
        if (socials.isNotEmpty)
          _section(
            context,
            l10n.p2_comments,
            Column(
              children: [
                for (final social in socials)
                  _LinkRow(
                    url: social.link!,
                    text: social.protocol.trim().isEmpty
                        ? l10n.p2_comment_thread
                        : social.protocol,
                  ),
              ],
            ),
          ),
        if (show.license?.hasContent == true)
          _license(context, l10n, show.license!),
        if (show.remoteItems.isNotEmpty)
          _section(
            context,
            l10n.p2_related,
            Column(
              children: [
                for (final item in show.remoteItems)
                  _RemoteRow(
                    item: item,
                    openLabel: l10n.p2_open_feed,
                    l10n: l10n,
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

List<(String?, List<Trailer>)> _trailerGroups(List<Trailer> trailers) {
  final groups = <(String?, List<Trailer>)>[];
  for (final trailer in trailers) {
    final season = trailer.season?.trim();
    final key = (season == null || season.isEmpty) ? null : season;
    final existing = groups.indexWhere((group) => group.$1 == key);
    if (existing >= 0) {
      groups[existing].$2.add(trailer);
    } else {
      groups.add((key, [trailer]));
    }
  }
  return groups;
}

String _liveStatusLabel(AppLocalizations l10n, String status) {
  switch (normalizeLiveStatus(status)) {
    case 'live':
      return l10n.p2_live_now;
    case 'ended':
      return l10n.p2_ended;
    default:
      return l10n.p2_upcoming;
  }
}

String _liveMeta(AppLocalizations l10n, LiveItem item) {
  final when = item.start.trim();
  final status = _liveStatusLabel(l10n, item.status);
  if (when.isEmpty) return status;
  return '$status · ${l10n.p2_starts(when)}';
}

String _mediumLabel(AppLocalizations l10n, String medium) {
  return switch (medium.trim().toLowerCase()) {
    'podcast' => l10n.p2_medium_podcast,
    'music' => l10n.p2_medium_music,
    'video' => l10n.p2_medium_video,
    'film' => l10n.p2_medium_film,
    'audiobook' => l10n.p2_medium_audiobook,
    'newsletter' => l10n.p2_medium_newsletter,
    'blog' => l10n.p2_medium_blog,
    'publisher' => l10n.p2_medium_publisher,
    _ => medium.trim(),
  };
}

String _cadenceLabel(AppLocalizations l10n, Cadence cadence) {
  final (one, many) = switch (cadence.unit) {
    CadenceUnit.hourly => (l10n.p2_unit_hour, l10n.p2_unit_hours),
    CadenceUnit.daily => (l10n.p2_unit_day, l10n.p2_unit_days),
    CadenceUnit.weekly => (l10n.p2_unit_week, l10n.p2_unit_weeks),
    CadenceUnit.monthly => (l10n.p2_unit_month, l10n.p2_unit_months),
    CadenceUnit.yearly => (l10n.p2_unit_year, l10n.p2_unit_years),
  };
  final unit = cadence.interval == 1 ? one : many;
  if (cadence.interval == 1) return l10n.p2_every_one(unit);
  return l10n.p2_every_n('${cadence.interval}', unit);
}

Widget _schedule(
  BuildContext context,
  AppLocalizations l10n,
  UpdateFrequency frequency,
) {
  final cadence = frequency.cadence;
  final text = frequency.text.trim();
  final complete = frequency.complete == true;
  if (text.isEmpty && cadence == null && !complete)
    return const SizedBox.shrink();
  return _section(
    context,
    l10n.p2_schedule,
    Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (text.isNotEmpty) Text(text),
        if (cadence != null) Text(_cadenceLabel(l10n, cadence)),
        if (complete) Text(l10n.p2_schedule_complete),
      ],
    ),
  );
}

Widget _license(
  BuildContext context,
  AppLocalizations l10n,
  P2License license,
) {
  return _section(
    context,
    l10n.p2_license,
    Wrap(
      spacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        if (license.identifier.trim().isNotEmpty) Text(license.identifier),
        if (license.url != null)
          _LinkRow(url: license.url!, text: l10n.p2_open_license),
      ],
    ),
  );
}

Widget _section(BuildContext context, String title, Widget body) {
  return Padding(
    padding: const EdgeInsets.only(top: 16),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        body,
      ],
    ),
  );
}

class _Badge extends StatelessWidget {
  final String text;
  const _Badge(this.text);
  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Text(
        text,
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
          color: Theme.of(context).colorScheme.primary,
        ),
      ),
    );
  }
}

class _ClipRow extends StatelessWidget {
  final String title;
  final String meta;
  final String tooltip;
  final VoidCallback? onPlay;
  const _ClipRow({
    required this.title,
    required this.meta,
    required this.tooltip,
    this.onPlay,
  });
  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: IconButton(
        tooltip: tooltip,
        onPressed: onPlay,
        icon: const Icon(Icons.play_arrow),
      ),
      title: Text(title),
      subtitle: Text(meta),
    );
  }
}

class _ChapterRow extends StatelessWidget {
  final P2Chapter chapter;
  final String linkLabel;
  final void Function(P2Chapter chapter)? onOpen;
  const _ChapterRow({
    required this.chapter,
    required this.linkLabel,
    this.onOpen,
  });
  @override
  Widget build(BuildContext context) {
    final href = httpLink(chapter.url);
    return ListTile(
      contentPadding: EdgeInsets.zero,
      onTap: onOpen == null ? null : () => onOpen!(chapter),
      leading: chapter.imageUrl != null && chapter.imageUrl!.isNotEmpty
          ? ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: Image.network(
                chapter.imageUrl!,
                width: 48,
                height: 48,
                fit: BoxFit.cover,
                errorBuilder: (_, _, _) =>
                    const Icon(Icons.image_not_supported),
              ),
            )
          : null,
      title: Text(chapter.title),
      subtitle: Text(formatClock(chapter.startTime)),
      trailing: href == null
          ? null
          : IconButton(
              tooltip: linkLabel,
              onPressed: () => _open(href),
              icon: const Icon(Icons.open_in_new),
            ),
    );
  }
}

class _LinkRow extends StatelessWidget {
  final String url;
  final String text;
  const _LinkRow({required this.url, required this.text});
  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerLeft,
      child: TextButton(onPressed: () => _open(url), child: Text(text)),
    );
  }
}

class _LocationRow extends StatelessWidget {
  final P2Location location;
  final String mapLabel;
  const _LocationRow({required this.location, required this.mapLabel});
  @override
  Widget build(BuildContext context) {
    final detail = location.country == null || location.country!.isEmpty
        ? location.name
        : '${location.name} · ${location.country}';
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(detail),
      trailing: location.mapUrl == null
          ? null
          : TextButton(
              onPressed: () => _open(location.mapUrl!),
              child: Text(mapLabel),
            ),
    );
  }
}

class _RemoteRow extends StatelessWidget {
  final RemoteItem item;
  final String openLabel;
  final AppLocalizations l10n;
  const _RemoteRow({
    required this.item,
    required this.openLabel,
    required this.l10n,
  });
  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(item.label),
      subtitle: item.medium == null
          ? null
          : Text(_mediumLabel(l10n, item.medium!)),
      trailing: item.feedUrl == null
          ? null
          : TextButton(
              onPressed: () => _open(item.feedUrl!),
              child: Text(openLabel),
            ),
    );
  }
}

class _SplitRow extends StatelessWidget {
  final ValueTimeSplit split;
  final AppLocalizations l10n;
  const _SplitRow({required this.split, required this.l10n});
  @override
  Widget build(BuildContext context) {
    final names = split.recipients
        .map((r) => r.name)
        .where((name) => name.isNotEmpty)
        .join(', ');
    final remote = split.remoteItem?.label;
    final start = double.tryParse(split.startTime);
    final duration = double.tryParse(split.duration);
    final when =
        '${start == null ? split.startTime : formatClock(start)} · ${duration == null ? split.duration : formatClock(duration)}';
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(when),
      subtitle: Text(
        [
          if (names.isNotEmpty) names,
          if (remote != null && remote.isNotEmpty) remote,
          if ((split.remotePercentage ?? '').isNotEmpty)
            l10n.p2_remote_percent(split.remotePercentage!),
        ].join(' · '),
      ),
    );
  }
}

Future<void> _open(String url) async {
  final uri = Uri.tryParse(url);
  if (uri == null) return;
  await launchUrl(uri, mode: LaunchMode.externalApplication);
}
