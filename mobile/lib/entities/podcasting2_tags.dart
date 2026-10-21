// Podcasting 2.0 tags from `tags` on the episode and podcast endpoints.
//
// See `docs/podcasting2-api.md`. Missing keys stay empty so older responses still parse.

class ItemTags {
  final String? guid;
  final List<Soundbite> soundbites;
  final List<P2Location> locations;
  final P2Season? season;
  final P2EpisodeNumber? episode;
  final P2License? license;
  final String? medium;
  final List<P2Image> image;
  final ImageSet? images;
  final List<ContentLink> contentLinks;
  final List<SocialInteract> socialInteracts;
  final List<FundingLink> funding;
  final P2Chat? chat;
  final List<ValueBlock> value;
  final List<RemoteItem> remoteItems;

  const ItemTags({
    this.guid,
    this.soundbites = const [],
    this.locations = const [],
    this.season,
    this.episode,
    this.license,
    this.medium,
    this.image = const [],
    this.images,
    this.contentLinks = const [],
    this.socialInteracts = const [],
    this.funding = const [],
    this.chat,
    this.value = const [],
    this.remoteItems = const [],
  });

  factory ItemTags.fromJson(Map<String, dynamic>? json) {
    if (json == null) return const ItemTags();
    return ItemTags(
      guid: json['guid'] as String?,
      soundbites: _list(json['soundbites'], Soundbite.fromJson),
      locations: _list(json['locations'], P2Location.fromJson),
      season: json['season'] is Map<String, dynamic>
          ? P2Season.fromJson(json['season'] as Map<String, dynamic>)
          : null,
      episode: json['episode'] is Map<String, dynamic>
          ? P2EpisodeNumber.fromJson(json['episode'] as Map<String, dynamic>)
          : null,
      license: json['license'] is Map<String, dynamic>
          ? P2License.fromJson(json['license'] as Map<String, dynamic>)
          : null,
      medium: json['medium'] as String?,
      image: _list(json['image'], P2Image.fromJson),
      images: json['images'] is Map<String, dynamic>
          ? ImageSet.fromJson(json['images'] as Map<String, dynamic>)
          : null,
      contentLinks: _list(json['content_links'], ContentLink.fromJson),
      socialInteracts: _list(json['social_interacts'], SocialInteract.fromJson),
      funding: _list(json['funding'], FundingLink.fromJson),
      chat: json['chat'] is Map<String, dynamic>
          ? P2Chat.fromJson(json['chat'] as Map<String, dynamic>)
          : null,
      value: _list(json['value'], ValueBlock.fromJson),
      remoteItems: _list(json['remote_items'], RemoteItem.fromJson),
    );
  }

  bool get isEmpty =>
      soundbites.isEmpty &&
      locations.isEmpty &&
      season == null &&
      episode == null &&
      license == null &&
      (medium == null || medium!.trim().isEmpty) &&
      contentLinks.isEmpty &&
      socialInteracts.isEmpty &&
      funding.isEmpty &&
      chat == null &&
      value.every((block) => block.timeSplits.isEmpty) &&
      remoteItems.isEmpty;
}

class ChannelTags {
  final String? guid;
  final String? medium;
  final List<P2Location> locations;
  final List<Trailer> trailers;
  final P2License? license;
  final List<P2Image> image;
  final ImageSet? images;
  final List<SocialInteract> socialInteracts;
  final UpdateFrequency? updateFrequency;
  final RemoteItem? publisher;
  final P2Chat? chat;
  final List<RemoteItem> remoteItems;

  const ChannelTags({
    this.guid,
    this.medium,
    this.locations = const [],
    this.trailers = const [],
    this.license,
    this.image = const [],
    this.images,
    this.socialInteracts = const [],
    this.updateFrequency,
    this.publisher,
    this.chat,
    this.remoteItems = const [],
  });

  factory ChannelTags.fromJson(Map<String, dynamic>? json) {
    if (json == null) return const ChannelTags();
    return ChannelTags(
      guid: json['guid'] as String?,
      medium: json['medium'] as String?,
      locations: _list(json['locations'], P2Location.fromJson),
      trailers: _list(json['trailers'], Trailer.fromJson),
      license: json['license'] is Map<String, dynamic>
          ? P2License.fromJson(json['license'] as Map<String, dynamic>)
          : null,
      image: _list(json['image'], P2Image.fromJson),
      images: json['images'] is Map<String, dynamic>
          ? ImageSet.fromJson(json['images'] as Map<String, dynamic>)
          : null,
      socialInteracts: _list(json['social_interacts'], SocialInteract.fromJson),
      updateFrequency: json['update_frequency'] is Map<String, dynamic>
          ? UpdateFrequency.fromJson(
              json['update_frequency'] as Map<String, dynamic>,
            )
          : null,
      publisher: json['publisher'] is Map<String, dynamic>
          ? RemoteItem.fromJson(json['publisher'] as Map<String, dynamic>)
          : null,
      chat: json['chat'] is Map<String, dynamic>
          ? P2Chat.fromJson(json['chat'] as Map<String, dynamic>)
          : null,
      remoteItems: _list(json['remote_items'], RemoteItem.fromJson),
    );
  }

  bool get isEmpty =>
      (medium == null || medium!.trim().isEmpty) &&
      locations.isEmpty &&
      trailers.isEmpty &&
      license == null &&
      socialInteracts.isEmpty &&
      updateFrequency == null &&
      publisher == null &&
      chat == null &&
      remoteItems.isEmpty;
}

class P2Chapter {
  final String title;
  final double startTime;
  final String? imageUrl;
  final String? url;

  const P2Chapter({
    required this.title,
    required this.startTime,
    this.imageUrl,
    this.url,
  });

  factory P2Chapter.fromJson(Map<String, dynamic> json) {
    return P2Chapter(
      title: (json['title'] as String?) ?? '',
      startTime: _asDouble(json['startTime'] ?? json['start_time']) ?? 0,
      imageUrl: (json['img'] ?? json['image']) as String?,
      url: httpLink(json['url'] as String?),
    );
  }
}

class Soundbite {
  final String startTime;
  final String duration;
  final String? title;

  const Soundbite({
    required this.startTime,
    required this.duration,
    this.title,
  });

  factory Soundbite.fromJson(Map<String, dynamic> json) => Soundbite(
    startTime: '${json['start_time'] ?? ''}',
    duration: '${json['duration'] ?? ''}',
    title: json['title'] as String?,
  );

  ClipWindow? get window => soundbiteWindow(startTime, duration);
}

class ClipWindow {
  final double start;
  final double end;
  const ClipWindow(this.start, this.end);
  double get duration => end - start;
}

class P2Location {
  final String name;
  final String? rel;
  final String? geo;
  final String? osm;
  final String? country;

  const P2Location({
    required this.name,
    this.rel,
    this.geo,
    this.osm,
    this.country,
  });

  factory P2Location.fromJson(Map<String, dynamic> json) => P2Location(
    name: (json['name'] as String?) ?? '',
    rel: json['rel'] as String?,
    geo: json['geo'] as String?,
    osm: json['osm'] as String?,
    country: json['country'] as String?,
  );

  String? get mapUrl => locationMapUrl(this);
}

class P2Season {
  final String number;
  final String? name;
  const P2Season({required this.number, this.name});
  factory P2Season.fromJson(Map<String, dynamic> json) => P2Season(
    number: '${json['number'] ?? ''}',
    name: json['name'] as String?,
  );
}

class P2EpisodeNumber {
  final String number;
  final String? display;
  const P2EpisodeNumber({required this.number, this.display});
  factory P2EpisodeNumber.fromJson(Map<String, dynamic> json) =>
      P2EpisodeNumber(
        number: '${json['number'] ?? ''}',
        display: json['display'] as String?,
      );
}

class P2License {
  final String identifier;
  final String? url;
  const P2License({required this.identifier, this.url});
  factory P2License.fromJson(Map<String, dynamic> json) => P2License(
    identifier: (json['identifier'] as String?) ?? '',
    url: httpLink(json['url'] as String?),
  );
  bool get hasContent =>
      identifier.trim().isNotEmpty || (url != null && url!.isNotEmpty);
}

class P2Image {
  final String href;
  final String? alt;
  final String? purpose;
  const P2Image({required this.href, this.alt, this.purpose});
  factory P2Image.fromJson(Map<String, dynamic> json) => P2Image(
    href: (json['href'] as String?) ?? '',
    alt: json['alt'] as String?,
    purpose: json['purpose'] as String?,
  );
}

class ImageSource {
  final String url;
  final String? descriptor;
  const ImageSource({required this.url, this.descriptor});
  factory ImageSource.fromJson(Map<String, dynamic> json) => ImageSource(
    url: (json['url'] as String?) ?? '',
    descriptor: json['descriptor'] as String?,
  );
}

class ImageSet {
  final String srcset;
  final List<ImageSource> sources;
  const ImageSet({required this.srcset, this.sources = const []});
  factory ImageSet.fromJson(Map<String, dynamic> json) => ImageSet(
    srcset: (json['srcset'] as String?) ?? '',
    sources: _list(json['sources'], ImageSource.fromJson),
  );
}

class ContentLink {
  final String href;
  final String text;
  const ContentLink({required this.href, required this.text});
  factory ContentLink.fromJson(Map<String, dynamic> json) => ContentLink(
    href: (json['href'] as String?) ?? '',
    text: (json['text'] as String?) ?? '',
  );
}

class SocialInteract {
  final String protocol;
  final String? uri;
  final String? accountId;
  final String? accountUrl;
  final String? priority;
  const SocialInteract({
    required this.protocol,
    this.uri,
    this.accountId,
    this.accountUrl,
    this.priority,
  });
  factory SocialInteract.fromJson(Map<String, dynamic> json) => SocialInteract(
    protocol: (json['protocol'] as String?) ?? '',
    uri: json['uri'] as String?,
    accountId: json['account_id'] as String?,
    accountUrl: json['account_url'] as String?,
    priority: json['priority']?.toString(),
  );
  String? get link => socialUrl(this);
}

class FundingLink {
  final String url;
  final String description;
  const FundingLink({required this.url, required this.description});
  factory FundingLink.fromJson(Map<String, dynamic> json) => FundingLink(
    url: (json['url'] as String?) ?? '',
    description: (json['description'] as String?) ?? '',
  );
}

class P2Chat {
  final String server;
  final String protocol;
  final String? accountId;
  final String? space;
  const P2Chat({
    required this.server,
    required this.protocol,
    this.accountId,
    this.space,
  });
  factory P2Chat.fromJson(Map<String, dynamic> json) => P2Chat(
    server: (json['server'] as String?) ?? '',
    protocol: (json['protocol'] as String?) ?? '',
    accountId: json['account_id'] as String?,
    space: json['space'] as String?,
  );
  String? get link => chatUrl(this);
}

class RemoteItem {
  final String feedGuid;
  final String? feedUrl;
  final String? itemGuid;
  final String? medium;
  final String? title;
  const RemoteItem({
    required this.feedGuid,
    this.feedUrl,
    this.itemGuid,
    this.medium,
    this.title,
  });
  factory RemoteItem.fromJson(Map<String, dynamic> json) => RemoteItem(
    feedGuid: (json['feed_guid'] as String?) ?? '',
    feedUrl: httpLink(json['feed_url'] as String?),
    itemGuid: json['item_guid'] as String?,
    medium: json['medium'] as String?,
    title: json['title'] as String?,
  );
  String get label {
    final named = title?.trim();
    if (named != null && named.isNotEmpty) return named;
    final kind = medium?.trim();
    if (kind != null && kind.isNotEmpty) return kind;
    return feedGuid;
  }
}

class ValueRecipient {
  final String name;
  const ValueRecipient({required this.name});
  factory ValueRecipient.fromJson(Map<String, dynamic> json) =>
      ValueRecipient(name: (json['name'] as String?) ?? '');
}

class ValueTimeSplit {
  final String startTime;
  final String duration;
  final String? remotePercentage;
  final List<ValueRecipient> recipients;
  final RemoteItem? remoteItem;
  const ValueTimeSplit({
    required this.startTime,
    required this.duration,
    this.remotePercentage,
    this.recipients = const [],
    this.remoteItem,
  });
  factory ValueTimeSplit.fromJson(Map<String, dynamic> json) => ValueTimeSplit(
    startTime: '${json['start_time'] ?? ''}',
    duration: '${json['duration'] ?? ''}',
    remotePercentage: json['remote_percentage']?.toString(),
    recipients: _list(json['recipients'], ValueRecipient.fromJson),
    remoteItem: json['remote_item'] is Map<String, dynamic>
        ? RemoteItem.fromJson(json['remote_item'] as Map<String, dynamic>)
        : null,
  );
}

class ValueBlock {
  final List<ValueTimeSplit> timeSplits;
  const ValueBlock({this.timeSplits = const []});
  factory ValueBlock.fromJson(Map<String, dynamic> json) => ValueBlock(
    timeSplits: _list(json['time_splits'], ValueTimeSplit.fromJson),
  );
}

class Trailer {
  final String title;
  final String url;
  final String? mimeType;
  final String? season;
  const Trailer({
    required this.title,
    required this.url,
    this.mimeType,
    this.season,
  });
  factory Trailer.fromJson(Map<String, dynamic> json) => Trailer(
    title: (json['title'] as String?) ?? '',
    url: (json['url'] as String?) ?? '',
    mimeType: json['mime_type'] as String?,
    season: json['season'] as String?,
  );
  bool get isVideo => (mimeType ?? '').toLowerCase().startsWith('video/');
}

class UpdateFrequency {
  final String text;
  final bool? complete;
  final String? rrule;
  const UpdateFrequency({required this.text, this.complete, this.rrule});
  factory UpdateFrequency.fromJson(Map<String, dynamic> json) =>
      UpdateFrequency(
        text: (json['text'] as String?) ?? '',
        complete: json['complete'] as bool?,
        rrule: json['rrule'] as String?,
      );
  Cadence? get cadence => rrule == null ? null : cadenceOf(rrule!);
}

enum CadenceUnit { hourly, daily, weekly, monthly, yearly }

class Cadence {
  final CadenceUnit unit;
  final int interval;
  const Cadence(this.unit, this.interval);
}

class ResolvedNumbers {
  final int? seasonNumber;
  final String? seasonName;
  final int? episodeNumber;
  final String? episodeText;
  final bool fromPodcasting2;

  const ResolvedNumbers({
    this.seasonNumber,
    this.seasonName,
    this.episodeNumber,
    this.episodeText,
    this.fromPodcasting2 = false,
  });

  bool get isEmpty =>
      seasonNumber == null &&
      (seasonName == null || seasonName!.isEmpty) &&
      (episodeText == null || episodeText!.isEmpty);
}

class ArtworkChoice {
  final String src;
  final String? srcset;
  final String? alt;
  const ArtworkChoice({required this.src, this.srcset, this.alt});
}

List<T> _list<T>(dynamic raw, T Function(Map<String, dynamic>) parse) {
  if (raw is! List) return const [];
  return raw.whereType<Map<String, dynamic>>().map(parse).toList();
}

double? _asDouble(dynamic raw) {
  if (raw is num) return raw.toDouble();
  if (raw is String) return double.tryParse(raw.trim());
  return null;
}

int? _whole(String? raw) {
  if (raw == null) return null;
  final text = raw.trim();
  if (text.isEmpty) return null;
  final asInt = int.tryParse(text);
  if (asInt != null) return asInt > 0 ? asInt : null;
  final asDouble = double.tryParse(text);
  if (asDouble == null || !asDouble.isFinite || asDouble <= 0) return null;
  return asDouble.floor();
}

ResolvedNumbers resolveNumbers({
  P2Season? season,
  P2EpisodeNumber? episode,
  int? itunesSeason,
  int? itunesEpisode,
}) {
  final seasonNumber =
      _whole(season?.number) ?? ((itunesSeason ?? 0) > 0 ? itunesSeason : null);
  final seasonName = season?.name?.trim();
  int? episodeNumber;
  String? episodeText;
  var fromP2 = season != null;
  if (episode != null) {
    fromP2 = true;
    episodeNumber =
        _whole(episode.number) ??
        ((itunesEpisode ?? 0) > 0 ? itunesEpisode : null);
    final display = episode.display?.trim();
    if (display != null && display.isNotEmpty) {
      episodeText = display;
    } else if (episodeNumber != null) {
      episodeText = '$episodeNumber';
    } else if (episode.number.trim().isNotEmpty) {
      episodeText = episode.number.trim();
    }
  } else if ((itunesEpisode ?? 0) > 0) {
    episodeNumber = itunesEpisode;
    episodeText = '$itunesEpisode';
  }
  return ResolvedNumbers(
    seasonNumber: seasonNumber,
    seasonName: (seasonName == null || seasonName.isEmpty) ? null : seasonName,
    episodeNumber: episodeNumber,
    episodeText: episodeText,
    fromPodcasting2: fromP2,
  );
}

/// Numbered episodes first, then season, then episode. Unnumbered sort last.
(int, int, int) seasonSortKey(ResolvedNumbers numbers) {
  final unnumbered =
      numbers.seasonNumber == null && numbers.episodeNumber == null ? 1 : 0;
  return (
    unnumbered,
    numbers.seasonNumber ?? 1 << 62,
    numbers.episodeNumber ?? 1 << 62,
  );
}

bool shouldGroupBySeason(Iterable<ResolvedNumbers> numbers) {
  return numbers.any(
    (n) =>
        n.seasonNumber != null ||
        (n.seasonName != null && n.seasonName!.isNotEmpty),
  );
}

String? formatNumbersBadge(
  String seasonWord,
  String episodeWord,
  ResolvedNumbers numbers,
) {
  if (numbers.isEmpty) return null;
  final parts = <String>[];
  if (numbers.seasonNumber != null) {
    if (numbers.seasonName != null) {
      parts.add('$seasonWord ${numbers.seasonNumber}: ${numbers.seasonName}');
    } else {
      parts.add('$seasonWord ${numbers.seasonNumber}');
    }
  } else if (numbers.seasonName != null) {
    parts.add(numbers.seasonName!);
  }
  final text = numbers.episodeText;
  if (text != null && text.isNotEmpty) {
    final numeric = RegExp(r'^\d+$').hasMatch(text);
    parts.add(numeric ? '$episodeWord $text' : text);
  }
  if (parts.isEmpty) return null;
  return parts.join(' · ');
}

ClipWindow? soundbiteWindow(String startTime, String duration) {
  final start = double.tryParse(startTime.trim());
  if (start == null || start < 0) return null;
  final length = double.tryParse(duration.trim()) ?? 0;
  if (length < 0) return null;
  return ClipWindow(start, start + length);
}

String formatClock(double seconds) {
  if (seconds.isNaN || seconds.isInfinite || seconds < 0) return '0:00';
  final total = seconds.round();
  final hours = total ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  final secs = total % 60;
  if (hours > 0) {
    return '$hours:${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
  }
  return '$minutes:${secs.toString().padLeft(2, '0')}';
}

String? locationMapUrl(P2Location location) {
  final geo = _parseGeo(location.geo);
  if (geo != null) {
    return 'https://www.openstreetmap.org/?mlat=${geo.$1}&mlon=${geo.$2}#map=14/${geo.$1}/${geo.$2}';
  }
  return _osmUrl(location.osm);
}

(String, String)? _parseGeo(String? raw) {
  if (raw == null) return null;
  var text = raw.trim();
  if (text.toLowerCase().startsWith('geo:')) text = text.substring(4);
  final parts = text.split(',');
  if (parts.length < 2) return null;
  final lat = double.tryParse(parts[0].trim());
  final lon = double.tryParse(parts[1].trim());
  if (lat == null ||
      lon == null ||
      lat < -90 ||
      lat > 90 ||
      lon < -180 ||
      lon > 180)
    return null;
  return (_trimCoord(lat), _trimCoord(lon));
}

String _trimCoord(double n) {
  var text = n.toStringAsFixed(6);
  text = text.replaceFirst(RegExp(r'0+$'), '');
  text = text.replaceFirst(RegExp(r'\.$'), '');
  return text;
}

String? _osmUrl(String? raw) {
  if (raw == null || raw.trim().length < 2) return null;
  final text = raw.trim();
  final kind = text[0].toUpperCase();
  final id = int.tryParse(text.substring(1).trim());
  if (id == null) return null;
  final path = switch (kind) {
    'N' => 'node',
    'W' => 'way',
    'R' => 'relation',
    _ => null,
  };
  if (path == null) return null;
  return 'https://www.openstreetmap.org/$path/$id';
}

bool _dangerous(String url) {
  final lower = url.trim().toLowerCase();
  return lower.startsWith('javascript:') ||
      lower.startsWith('data:') ||
      lower.startsWith('vbscript:');
}

String? safeLink(String? url) {
  if (url == null) return null;
  final text = url.trim();
  if (text.isEmpty || _dangerous(text) || !text.contains(':')) return null;
  return text;
}

String? httpLink(String? url) {
  if (url == null) return null;
  final text = url.trim();
  if (text.startsWith('https://') || text.startsWith('http://')) return text;
  return null;
}

String? chatUrl(P2Chat chat) {
  final server = chat.server.trim();
  final protocol = chat.protocol.trim().toLowerCase();
  final space = (chat.space ?? '').trim();
  if (_dangerous(server)) return null;
  if (server.startsWith('http://') || server.startsWith('https://'))
    return server;
  switch (protocol) {
    case 'irc':
      if (server.isEmpty) return null;
      final room = space.startsWith('#') ? space.substring(1) : space;
      return room.isEmpty ? 'irc://$server' : 'irc://$server/$room';
    case 'matrix':
      String room;
      if (space.startsWith('#') || space.startsWith('!')) {
        room = space.contains(':') || server.isEmpty ? space : '$space:$server';
      } else if (space.isNotEmpty && server.isNotEmpty) {
        room = '#$space:$server';
      } else if ((chat.accountId ?? '').trim().isNotEmpty) {
        room = chat.accountId!.trim();
      } else if (server.isNotEmpty) {
        return 'https://$server';
      } else {
        return null;
      }
      return 'https://matrix.to/#/${Uri.encodeComponent(room)}';
    case 'xmpp':
      final room = space.isNotEmpty ? space : (chat.accountId ?? '').trim();
      if (room.isEmpty) return null;
      return space.isEmpty ? 'xmpp:$room' : 'xmpp:$room?join';
    case 'nostr':
      final id = (chat.accountId ?? '').trim();
      if (id.isEmpty) return null;
      return id.startsWith('npub') ? 'https://njump.me/$id' : 'nostr:$id';
    default:
      return server.isEmpty ? null : 'https://$server';
  }
}

String? socialUrl(SocialInteract social) {
  if (social.protocol.toLowerCase() == 'disabled') return null;
  for (final candidate in [social.uri, social.accountUrl]) {
    final url = safeLink(candidate);
    if (url != null) return url;
  }
  return null;
}

ArtworkChoice artworkChoice(
  List<P2Image> images,
  ImageSet? set,
  String fallback,
) {
  bool http(String href) => httpLink(href) != null;
  P2Image? preferred;
  P2Image? first;
  for (final image in images) {
    if (!http(image.href)) continue;
    first ??= image;
    final purpose = (image.purpose ?? '').toLowerCase();
    if (purpose == 'artwork' || purpose == 'cover') {
      preferred = image;
      break;
    }
  }
  final chosen = preferred ?? first;
  final srcset = _srcsetOf(set);
  var src = chosen?.href.trim();
  src ??= srcset == null ? null : _largestSrcsetUrl(srcset);
  return ArtworkChoice(
    src: (src == null || src.isEmpty) ? fallback : src,
    srcset: srcset,
    alt: chosen?.alt,
  );
}

String? _srcsetOf(ImageSet? set) {
  if (set == null) return null;
  if (set.sources.isNotEmpty) {
    final parts = <String>[];
    for (final source in set.sources) {
      if (httpLink(source.url) == null) continue;
      final descriptor = source.descriptor?.trim();
      parts.add(
        descriptor == null || descriptor.isEmpty
            ? source.url.trim()
            : '${source.url.trim()} $descriptor',
      );
    }
    return parts.isEmpty ? null : parts.join(', ');
  }
  if (set.srcset.trim().isEmpty) return null;
  final parts = <String>[];
  for (final part in set.srcset.split(',')) {
    final trimmed = part.trim();
    final url = trimmed.split(RegExp(r'\s+')).first;
    if (httpLink(url) != null) parts.add(trimmed);
  }
  return parts.isEmpty ? null : parts.join(', ');
}

String? _largestSrcsetUrl(String srcset) {
  String? bestUrl;
  var bestWidth = -1;
  for (final part in srcset.split(',')) {
    final bits = part.trim().split(RegExp(r'\s+'));
    if (bits.isEmpty || bits.first.isEmpty) continue;
    final width = bits.length > 1
        ? int.tryParse(bits[1].replaceAll(RegExp(r'[wW]$'), '')) ?? 0
        : 0;
    if (width >= bestWidth) {
      bestWidth = width;
      bestUrl = bits.first;
    }
  }
  return bestUrl;
}

Cadence? cadenceOf(String rrule) {
  String? freq;
  var interval = 1;
  for (final part in rrule.split(';')) {
    final bits = part.split('=');
    if (bits.length != 2) continue;
    final key = bits[0].trim().toUpperCase();
    final value = bits[1].trim();
    if (key == 'FREQ') freq = value.toUpperCase();
    if (key == 'INTERVAL') {
      final parsed = int.tryParse(value);
      if (parsed != null && parsed > 0) interval = parsed;
    }
  }
  return switch (freq) {
    'HOURLY' => Cadence(CadenceUnit.hourly, interval),
    'DAILY' => Cadence(CadenceUnit.daily, interval),
    'WEEKLY' => Cadence(CadenceUnit.weekly, interval),
    'MONTHLY' => Cadence(CadenceUnit.monthly, interval),
    'YEARLY' => Cadence(CadenceUnit.yearly, interval),
    _ => null,
  };
}

List<ValueTimeSplit> timeSplitsOf(List<ValueBlock> blocks) {
  return [for (final block in blocks) ...block.timeSplits];
}

/// Chapters parsed from the top-level `chapters` array, with image and link when present.
List<P2Chapter> chaptersFromPodcasting2(Map<String, dynamic>? data) {
  final raw = data?['chapters'];
  if (raw is! List) return const [];
  return raw
      .whereType<Map<String, dynamic>>()
      .map(P2Chapter.fromJson)
      .where((c) => c.title.isNotEmpty || c.startTime >= 0)
      .toList();
}
