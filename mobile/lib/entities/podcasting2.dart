// Podcasting 2.0 channel extras served by /api/data/fetch_podcasting_2_pod_data.
// Value is display-only. Podroll items are resolved to a show by the caller.

class PodrollItem {
  final String feedGuid;
  final String? feedUrl;
  final String? title;
  final String? artworkUrl;

  const PodrollItem({
    required this.feedGuid,
    this.feedUrl,
    this.title,
    this.artworkUrl,
  });

  factory PodrollItem.fromJson(Map<String, dynamic> json) {
    return PodrollItem(
      feedGuid: _str(json['feed_guid'] ?? json['feedGuid']),
      feedUrl: _opt(json['feed_url'] ?? json['feedUrl']),
      title: _opt(json['title']),
      artworkUrl: _opt(json['artwork_url'] ?? json['artworkUrl'] ?? json['artwork'] ?? json['image']),
    );
  }

  bool get hasIdentity => feedGuid.trim().isNotEmpty || (feedUrl?.trim().isNotEmpty ?? false);
}

class ValueRecipient {
  final String name;
  final String type;
  final String address;
  final String split;

  const ValueRecipient({
    required this.name,
    required this.type,
    required this.address,
    required this.split,
  });

  factory ValueRecipient.fromJson(Map<String, dynamic> json) {
    return ValueRecipient(
      name: _str(json['name']),
      type: _str(json['type']),
      address: _str(json['address']),
      split: _str(json['split']),
    );
  }

  String get displayName => name.trim().isNotEmpty ? name.trim() : type.trim();

  String get splitLabel => formatSplit(split);
}

class ValueBlock {
  final String type;
  final String method;
  final String? suggested;
  final List<ValueRecipient> recipients;

  const ValueBlock({
    required this.type,
    required this.method,
    this.suggested,
    required this.recipients,
  });

  factory ValueBlock.fromJson(Map<String, dynamic> json) {
    final raw = json['recipients'];
    final recipients = raw is List
        ? raw.whereType<Map>().map((entry) => ValueRecipient.fromJson(Map<String, dynamic>.from(entry))).toList()
        : const <ValueRecipient>[];
    return ValueBlock(
      type: _str(json['type']),
      method: _str(json['method']),
      suggested: _opt(json['suggested']),
      recipients: recipients,
    );
  }

  /// "lightning · keysend · Suggested 5000". Protocol names stay as the feed wrote them.
  String methodLine(String suggestedLabel) {
    final parts = <String>[
      if (type.trim().isNotEmpty) type.trim(),
      if (method.trim().isNotEmpty) method.trim(),
      if (suggested != null && suggested!.trim().isNotEmpty) '$suggestedLabel ${suggested!.trim()}',
    ];
    return parts.join(' · ');
  }
}

class Podcasting2Channel {
  final List<PodrollItem> podroll;
  final List<ValueBlock> value;

  const Podcasting2Channel({required this.podroll, required this.value});

  factory Podcasting2Channel.fromJson(Map<String, dynamic> json) {
    final podroll = json['podroll'] is List
        ? (json['podroll'] as List)
            .whereType<Map>()
            .map((entry) => PodrollItem.fromJson(Map<String, dynamic>.from(entry)))
            .where((item) => item.hasIdentity)
            .toList()
        : const <PodrollItem>[];
    final value = json['value'] is List
        ? (json['value'] as List)
            .whereType<Map>()
            .map((entry) => ValueBlock.fromJson(Map<String, dynamic>.from(entry)))
            .toList()
        : const <ValueBlock>[];
    return Podcasting2Channel(podroll: podroll, value: value);
  }
}

/// One Podcast Index / search-proxy feed, reduced to what a podroll row needs.
class IndexHit {
  final String guid;
  final String title;
  final String url;
  final String originalUrl;
  final String artwork;
  final String author;
  final String description;
  final String link;
  final int id;
  final int episodeCount;
  final bool explicit;
  final Map<String, String> categories;

  const IndexHit({
    required this.guid,
    required this.title,
    required this.url,
    required this.originalUrl,
    required this.artwork,
    required this.author,
    required this.description,
    required this.link,
    required this.id,
    required this.episodeCount,
    required this.explicit,
    required this.categories,
  });

  factory IndexHit.fromJson(Map<String, dynamic> json) {
    final image = _str(json['image']);
    final artwork = _str(json['artwork']);
    final categories = <String, String>{};
    final rawCategories = json['categories'];
    if (rawCategories is Map) {
      rawCategories.forEach((key, value) {
        if (value != null) categories['$key'] = '$value';
      });
    }
    return IndexHit(
      guid: _str(json['podcastGuid'] ?? json['podcast_guid']),
      title: _str(json['title']),
      url: _str(json['url']),
      originalUrl: _str(json['originalUrl'] ?? json['original_url']),
      artwork: artwork.isNotEmpty ? artwork : image,
      author: _str(json['author']),
      description: _str(json['description']),
      link: _str(json['link']),
      id: _int(json['id']),
      episodeCount: _int(json['episodeCount'] ?? json['episode_count']),
      explicit: _bool(json['explicit']),
      categories: categories,
    );
  }
}

class ResolvedPodroll {
  final String feedGuid;
  final String feedUrl;
  final String title;
  final String author;
  final String artwork;
  final String description;
  final String link;
  final String originalUrl;
  final int indexId;
  final int episodeCount;
  final bool explicit;
  final Map<String, String> categories;
  final bool resolved;
  final bool subscribed;

  const ResolvedPodroll({
    required this.feedGuid,
    required this.feedUrl,
    required this.title,
    required this.author,
    required this.artwork,
    required this.description,
    required this.link,
    required this.originalUrl,
    required this.indexId,
    required this.episodeCount,
    required this.explicit,
    required this.categories,
    required this.resolved,
    this.subscribed = false,
  });

  bool get canOpen => feedUrl.isNotEmpty;

  String get key => feedGuid.isNotEmpty ? feedGuid : feedUrl;

  factory ResolvedPodroll.placeholder(PodrollItem item) {
    final url = item.feedUrl?.trim() ?? '';
    final embeddedTitle = item.title?.trim() ?? '';
    return ResolvedPodroll(
      feedGuid: item.feedGuid.trim(),
      feedUrl: url,
      title: podrollTitle(resolved: embeddedTitle, feedUrl: url, feedGuid: item.feedGuid),
      author: '',
      artwork: item.artworkUrl?.trim() ?? '',
      description: '',
      link: '',
      originalUrl: url,
      indexId: 0,
      episodeCount: 0,
      explicit: false,
      categories: const {},
      resolved: embeddedTitle.isNotEmpty,
    );
  }

  ResolvedPodroll apply(IndexHit hit) {
    final nextUrl = hit.url.trim().isNotEmpty ? hit.url.trim() : (hit.originalUrl.trim().isNotEmpty ? hit.originalUrl.trim() : feedUrl);
    return ResolvedPodroll(
      feedGuid: feedGuid,
      feedUrl: nextUrl,
      title: hit.title.trim().isNotEmpty ? hit.title.trim() : title,
      author: hit.author.trim().isNotEmpty ? hit.author.trim() : author,
      artwork: hit.artwork.trim().isNotEmpty ? hit.artwork.trim() : artwork,
      description: hit.description.trim().isNotEmpty ? hit.description.trim() : description,
      link: hit.link.trim().isNotEmpty ? hit.link.trim() : link,
      originalUrl: hit.originalUrl.trim().isNotEmpty ? hit.originalUrl.trim() : originalUrl,
      indexId: hit.id > 0 ? hit.id : indexId,
      episodeCount: hit.episodeCount,
      explicit: hit.explicit,
      categories: hit.categories.isNotEmpty ? hit.categories : categories,
      resolved: hit.title.trim().isNotEmpty || resolved,
      subscribed: subscribed,
    );
  }

  ResolvedPodroll copyWith({bool? subscribed}) {
    return ResolvedPodroll(
      feedGuid: feedGuid,
      feedUrl: feedUrl,
      title: title,
      author: author,
      artwork: artwork,
      description: description,
      link: link,
      originalUrl: originalUrl,
      indexId: indexId,
      episodeCount: episodeCount,
      explicit: explicit,
      categories: categories,
      resolved: resolved,
      subscribed: subscribed ?? this.subscribed,
    );
  }
}

String formatSplit(String split) {
  final trimmed = split.trim();
  if (trimmed.isEmpty) return '';
  if (trimmed.endsWith('%')) return trimmed;
  final value = num.tryParse(trimmed);
  if (value == null) return trimmed;
  if (value == value.roundToDouble()) return '${value.toInt()}%';
  return '$trimmed%';
}

String lookupQuery({required String feedGuid, String? feedUrl}) {
  final url = feedUrl?.trim() ?? '';
  if (url.isNotEmpty) return url;
  return feedGuid.trim();
}

String shortGuid(String guid) {
  final trimmed = guid.trim();
  if (trimmed.isEmpty) return '';
  if (trimmed.length <= 12) return trimmed;
  return '${trimmed.substring(0, 8)}…';
}

String podrollTitle({String? resolved, String? feedUrl, required String feedGuid}) {
  final title = resolved?.trim() ?? '';
  if (title.isNotEmpty) return title;
  final url = feedUrl?.trim() ?? '';
  if (url.isNotEmpty) {
    return Uri.tryParse(url)?.host ?? url;
  }
  return shortGuid(feedGuid);
}

/// GUID equality wins. An exact feed URL matches next. A single result is kept only when the
/// search was by feed URL, because that is how the existing Podcast Index term search answers
/// an exact feed URL. A GUID search that does not echo the GUID is not a match.
IndexHit? pickIndexHit(List<IndexHit> hits, {required String feedGuid, String? feedUrl}) {
  final guid = feedGuid.trim().toLowerCase();
  if (guid.isNotEmpty) {
    for (final hit in hits) {
      if (hit.guid.trim().toLowerCase() == guid) return hit;
    }
  }
  final url = feedUrl?.trim() ?? '';
  if (url.isEmpty) return null;
  for (final hit in hits) {
    if (_urlsMatch(hit.url, url) || _urlsMatch(hit.originalUrl, url)) return hit;
  }
  if (hits.length == 1) return hits.first;
  return null;
}

List<IndexHit> hitsFromSearchJson(Map<String, dynamic> json) {
  final feeds = json['feeds'];
  if (feeds is! List) return const [];
  return feeds.whereType<Map>().map((entry) => IndexHit.fromJson(Map<String, dynamic>.from(entry))).toList();
}

IndexHit? hitFromPodcastIndexBody(Map<String, dynamic> json) {
  final status = json['status'];
  if (status == false || status == 'false') return null;
  final feed = json['feed'];
  if (feed is Map) return IndexHit.fromJson(Map<String, dynamic>.from(feed));
  final feeds = hitsFromSearchJson(json);
  if (feeds.length == 1) return feeds.first;
  return null;
}

bool _urlsMatch(String left, String right) {
  final a = _normalizeUrl(left);
  final b = _normalizeUrl(right);
  return a.isNotEmpty && a == b;
}

String _normalizeUrl(String url) {
  var value = url.trim().toLowerCase();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  return value;
}

String _str(dynamic value) => value is String ? value : '';

String? _opt(dynamic value) {
  if (value is! String) return null;
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}

int _int(dynamic value) {
  if (value is int) return value;
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value) ?? 0;
  return 0;
}

bool _bool(dynamic value) {
  if (value is bool) return value;
  if (value is num) return value != 0;
  if (value is String) return value == 'true' || value == '1';
  return false;
}
