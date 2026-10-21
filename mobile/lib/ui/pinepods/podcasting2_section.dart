// Podroll recommendations and Value4Value recipients on a PinePods podcast page.
// Display only: opening or subscribing uses the existing podcast flow, and nothing sends a payment.

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/podcasting2.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';

class Podcasting2Section extends StatefulWidget {
  final List<PodrollItem> podroll;
  final List<ValueBlock> valueBlocks;
  final Future<IndexHit?> Function(PodrollItem item) lookup;
  final Future<bool> Function(ResolvedPodroll show)? alreadySubscribed;
  final void Function(ResolvedPodroll show) onOpen;
  final Future<bool> Function(ResolvedPodroll show) onSubscribe;

  const Podcasting2Section({
    super.key,
    required this.podroll,
    required this.valueBlocks,
    required this.lookup,
    required this.onOpen,
    required this.onSubscribe,
    this.alreadySubscribed,
  });

  @override
  State<Podcasting2Section> createState() => _Podcasting2SectionState();
}

class _Podcasting2SectionState extends State<Podcasting2Section> {
  List<ResolvedPodroll> _shows = const [];
  final Set<String> _busy = {};

  @override
  void initState() {
    super.initState();
    _shows = _placeholders(widget.podroll);
    _resolve();
  }

  @override
  void didUpdateWidget(covariant Podcasting2Section oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_samePodroll(oldWidget.podroll, widget.podroll)) {
      _shows = _placeholders(widget.podroll);
      _busy.clear();
      _resolve();
    }
  }

  List<ResolvedPodroll> _placeholders(List<PodrollItem> items) {
    return items.where((item) => item.hasIdentity).map(ResolvedPodroll.placeholder).toList();
  }

  bool _samePodroll(List<PodrollItem> left, List<PodrollItem> right) {
    if (left.length != right.length) return false;
    for (var i = 0; i < left.length; i++) {
      if (left[i].feedGuid != right[i].feedGuid || left[i].feedUrl != right[i].feedUrl) return false;
    }
    return true;
  }

  Future<void> _resolve() async {
    final items = widget.podroll.where((item) => item.hasIdentity).toList();
    final next = <ResolvedPodroll>[];
    for (final item in items) {
      ResolvedPodroll show = ResolvedPodroll.placeholder(item);
      try {
        final hit = await widget.lookup(item);
        if (hit != null) show = show.apply(hit);
      } catch (_) {}
      if (widget.alreadySubscribed != null && show.canOpen) {
        try {
          show = show.copyWith(subscribed: await widget.alreadySubscribed!(show));
        } catch (_) {}
      }
      next.add(show);
    }
    if (!mounted) return;
    setState(() => _shows = next);
  }

  Future<void> _subscribe(ResolvedPodroll show) async {
    if (!show.canOpen || show.subscribed || _busy.contains(show.key)) return;
    setState(() => _busy.add(show.key));
    var ok = false;
    try {
      ok = await widget.onSubscribe(show);
    } catch (_) {
      ok = false;
    }
    if (!mounted) return;
    setState(() {
      _busy.remove(show.key);
      if (ok) {
        _shows = [
          for (final row in _shows) row.key == show.key ? row.copyWith(subscribed: true) : row,
        ];
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    if (widget.valueBlocks.isEmpty && _shows.isEmpty) return const SizedBox.shrink();
    final l10n = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final muted = theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (widget.valueBlocks.isNotEmpty) _ValueCard(blocks: widget.valueBlocks, l10n: l10n, muted: muted),
        if (widget.valueBlocks.isNotEmpty && _shows.isNotEmpty) const SizedBox(height: 16),
        if (_shows.isNotEmpty) ...[
          Text(
            l10n?.podroll_title ?? 'Podroll',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 4),
          Text(l10n?.podroll_hint ?? 'Shows this podcast recommends', style: muted),
          const SizedBox(height: 8),
          for (final show in _shows) ...[
            _PodrollRow(
              show: show,
              busy: _busy.contains(show.key),
              l10n: l10n,
              onOpen: show.canOpen ? () => widget.onOpen(show) : null,
              onSubscribe: show.canOpen && !show.subscribed ? () => _subscribe(show) : null,
            ),
            const SizedBox(height: 8),
          ],
        ],
      ],
    );
  }
}

class _ValueCard extends StatelessWidget {
  final List<ValueBlock> blocks;
  final AppLocalizations? l10n;
  final TextStyle? muted;

  const _ValueCard({required this.blocks, required this.l10n, required this.muted});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final suggested = _suggestedLabel(l10n);
    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.volunteer_activism_outlined, size: 20, color: theme.colorScheme.primary),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    l10n?.value_for_value_title ?? 'Value for Value',
                    style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Text(l10n?.value_for_value_supported ?? 'This show supports Value4Value', style: muted),
            for (final block in blocks) ...[
              if (block.methodLine(suggested).isNotEmpty) ...[
                const SizedBox(height: 8),
                Text(block.methodLine(suggested), style: muted),
              ],
              if (block.recipients.isNotEmpty) ...[
                const SizedBox(height: 8),
                Text(
                  l10n?.value_recipients ?? 'Recipients',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                    letterSpacing: 0.4,
                  ),
                ),
                for (final recipient in block.recipients)
                  Padding(
                    padding: const EdgeInsets.only(top: 6),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(recipient.displayName, style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                              if (recipient.address.trim().isNotEmpty)
                                Text(
                                  recipient.address.trim(),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: muted,
                                ),
                            ],
                          ),
                        ),
                        if (recipient.splitLabel.isNotEmpty)
                          Text(
                            recipient.splitLabel,
                            style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700),
                          ),
                      ],
                    ),
                  ),
              ],
            ],
            const SizedBox(height: 8),
            Text(
              l10n?.value_display_only ?? 'Display only. PinePods does not send payments.',
              style: muted,
            ),
          ],
        ),
      ),
    );
  }

  String _suggestedLabel(AppLocalizations? l10n) {
    if (l10n == null) return 'Suggested';
    return l10n.value_suggested('\u0000').replaceAll('\u0000', '').trim();
  }
}

class _PodrollRow extends StatelessWidget {
  final ResolvedPodroll show;
  final bool busy;
  final AppLocalizations? l10n;
  final VoidCallback? onOpen;
  final VoidCallback? onSubscribe;

  const _PodrollRow({
    required this.show,
    required this.busy,
    required this.l10n,
    required this.onOpen,
    required this.onSubscribe,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant);
    final subscribeLabel = busy
        ? (l10n?.podroll_subscribing ?? 'Subscribing…')
        : show.subscribed
            ? (l10n?.podroll_subscribed ?? 'Subscribed')
            : (l10n?.podroll_subscribe ?? 'Subscribe');

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ClipRRect(
          borderRadius: BorderRadius.circular(8),
          child: show.artwork.isEmpty
              ? Container(
                  width: 64,
                  height: 64,
                  color: theme.colorScheme.surfaceContainerHighest,
                  child: Icon(Icons.podcasts, color: theme.colorScheme.onSurfaceVariant),
                )
              : PodcastImage(url: show.artwork, width: 64, height: 64, fit: BoxFit.cover),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                show.title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600),
              ),
              if (show.author.isNotEmpty) Text(show.author, maxLines: 1, overflow: TextOverflow.ellipsis, style: muted),
              if (!show.resolved) Text(l10n?.podroll_unresolved ?? "Couldn't look this show up", style: muted),
              const SizedBox(height: 4),
              Wrap(
                spacing: 8,
                children: [
                  TextButton(
                    onPressed: onOpen,
                    child: Text(l10n?.podroll_open ?? 'Open'),
                  ),
                  FilledButton.tonal(
                    onPressed: busy || show.subscribed ? null : onSubscribe,
                    child: Text(subscribeLabel),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
