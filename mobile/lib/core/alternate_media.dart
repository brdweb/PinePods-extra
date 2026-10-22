import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// A media version derived from the RSS enclosure and `podcast:alternateEnclosure`.
///
/// Mirrors `rust-api/src/services/alternate_media.rs`. IPFS and torrent sources are
/// listed and never played. HLS/DASH are playable and not saved as a single file.
class MediaSource {
  final String uri;
  final String? contentType;
  final String scheme;
  final bool playable;
  final bool downloadable;
  final String? unsupportedReason;

  const MediaSource({
    required this.uri,
    this.contentType,
    required this.scheme,
    required this.playable,
    required this.downloadable,
    this.unsupportedReason,
  });
}

class MediaIntegrity {
  final String kind;
  final String value;
  final bool verifiable;

  const MediaIntegrity({required this.kind, required this.value, required this.verifiable});
}

class MediaOption {
  final String id;
  final String? title;
  final String mimeType;
  final int? bitrate;
  final int? height;
  final String? lang;
  final String? codecs;
  final bool isDefault;
  final String kind;
  final List<MediaSource> sources;
  final MediaIntegrity? integrity;
  final bool playable;
  final bool downloadable;

  const MediaOption({
    required this.id,
    this.title,
    required this.mimeType,
    this.bitrate,
    this.height,
    this.lang,
    this.codecs,
    required this.isDefault,
    required this.kind,
    required this.sources,
    this.integrity,
    required this.playable,
    required this.downloadable,
  });
}

class PlaybackStep {
  final String optionId;
  final int sourceIndex;
  final String uri;
  final String mimeType;
  final String kind;
  final bool isVideo;

  const PlaybackStep({
    required this.optionId,
    required this.sourceIndex,
    required this.uri,
    required this.mimeType,
    required this.kind,
    required this.isVideo,
  });
}

class DownloadStep {
  final String optionId;
  final String uri;
  final String mimeType;
  final MediaIntegrity? integrity;

  const DownloadStep({
    required this.optionId,
    required this.uri,
    required this.mimeType,
    this.integrity,
  });
}

class MediaPlan {
  final List<MediaOption> options;
  final String selectedId;
  final List<PlaybackStep> playback;
  final List<DownloadStep> download;

  const MediaPlan({
    required this.options,
    required this.selectedId,
    required this.playback,
    required this.download,
  });

  MediaOption? get selected {
    for (final option in options) {
      if (option.id == selectedId) return option;
    }
    return options.isEmpty ? null : options.first;
  }
}

/// In-memory choice for the episode currently being played, so Music Assistant
/// and the now-playing screen can see it without another lookup.
class MediaChoiceSession {
  static int? episodeId;
  static String? optionId;
  static MediaPlan? plan;

  /// Integrity to check when a local download of [episodeId] finishes.
  static final Map<int, MediaIntegrity> pendingIntegrity = {};

  static void remember(int episodeId, MediaPlan plan) {
    MediaChoiceSession.episodeId = episodeId;
    MediaChoiceSession.optionId = plan.selectedId;
    MediaChoiceSession.plan = plan;
  }

  static void clear() {
    episodeId = null;
    optionId = null;
    plan = null;
  }
}

const _prefPrefix = 'pinepods.mediaOption.';

Future<String?> preferredMediaOption(int episodeId) async {
  try {
    final prefs = await SharedPreferences.getInstance();
    final value = prefs.getString('$_prefPrefix$episodeId');
    if (value == null || value.trim().isEmpty) return null;
    return value;
  } catch (_) {
    // Tests and a first launch before the plugin is ready still play the auto-selected file.
    return null;
  }
}

Future<void> storePreferredMediaOption(int episodeId, String optionId) async {
  try {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('$_prefPrefix$episodeId', optionId);
  } catch (_) {}
}

MediaPlan planFromTags({
  required String primaryUri,
  bool isVideo = false,
  Map<String, dynamic>? tags,
  String? preferOptionId,
  bool musicAssistant = false,
}) {
  final raw = tags?['alternate_enclosures'];
  final alternates = raw is List ? raw.whereType<Map>().map((e) => e.cast<String, dynamic>()).toList() : const <Map<String, dynamic>>[];
  return planMedia(
    primaryUri: primaryUri,
    isVideo: isVideo,
    alternates: alternates,
    preferOptionId: preferOptionId,
    musicAssistant: musicAssistant,
  );
}

MediaPlan planMedia({
  required String primaryUri,
  bool isVideo = false,
  String? primaryMime,
  required List<Map<String, dynamic>> alternates,
  String? preferOptionId,
  bool musicAssistant = false,
}) {
  final options = <MediaOption>[_primaryOption(primaryUri, primaryMime, isVideo)];
  for (var i = 0; i < alternates.length; i++) {
    options.add(_alternateOption(i, alternates[i]));
  }
  final explicit = (preferOptionId ?? '').trim();
  final known = explicit.isNotEmpty && options.any((o) => o.id == explicit) ? explicit : null;
  final selected = _selectId(options, known, musicAssistant);
  return MediaPlan(
    options: options,
    selectedId: selected,
    playback: _playbackChain(options, selected),
    download: _downloadChain(options, selected, known != null),
  );
}

String mediaOptionLabel(
  MediaOption option, {
  String original = 'Original',
  String audio = 'Audio',
  String video = 'Video',
  String hls = 'HLS stream',
  String dash = 'DASH stream',
  String other = 'Other',
  String ipfs = 'IPFS — shown only',
  String torrent = 'Torrent — shown only',
  String unavailable = 'Cannot play on this device',
}) {
  final parts = <String>[];
  if (option.id == 'primary') {
    parts.add(original);
  } else if (option.title != null && option.title!.trim().isNotEmpty) {
    parts.add(option.title!.trim());
  } else {
    parts.add(switch (option.kind) {
      'video' => video,
      'hls' => hls,
      'dash' => dash,
      'audio' => audio,
      _ => other,
    });
  }
  final bitrate = option.bitrate;
  if (bitrate != null && bitrate > 0) {
    final kbps = bitrate >= 1000 ? (bitrate / 1000).round() : bitrate;
    parts.add('$kbps kbps');
  }
  if (option.codecs != null && option.codecs!.isNotEmpty) parts.add(option.codecs!);
  if (option.lang != null && option.lang!.isNotEmpty) parts.add(option.lang!);
  String? reason;
  for (final source in option.sources) {
    final unsupported = source.unsupportedReason;
    if (unsupported == 'ipfs' || unsupported == 'torrent') {
      reason = unsupported;
      break;
    }
  }
  if (!option.playable || reason != null) {
    parts.add(switch (reason) {
      'ipfs' => ipfs,
      'torrent' => torrent,
      _ => unavailable,
    });
  }
  return parts.join(' · ');
}

/// `true` match, `false` mismatch, `null` when the hash type is not one we can check.
bool? sriMatches(List<int> bytes, String kind, String value) {
  final tokens = _expectedDigests(kind, value);
  if (tokens == null) return null;
  final (algo, digests) = tokens;
  final actual = _digest(algo, bytes);
  return digests.any((d) => _bytesEqual(d, actual));
}

Future<bool?> fileMatchesSri(File file, MediaIntegrity integrity) async {
  if (!integrity.verifiable) return null;
  final tokens = _expectedDigests(integrity.kind, integrity.value);
  if (tokens == null) return null;
  final (algo, digests) = tokens;
  final Hash hasher = switch (algo) {
    'sha256' => sha256,
    'sha384' => sha384,
    'sha512' => sha512,
    _ => sha256,
  };
  final actual = await hasher.bind(file.openRead()).first;
  return digests.any((d) => _bytesEqual(d, actual.bytes));
}

MediaOption _primaryOption(String uri, String? mime, bool isVideo) {
  var mimeType = (mime ?? '').trim();
  if (mimeType.isEmpty && isVideo) mimeType = 'video/mp4';
  final source = _classify(uri, mimeType.isEmpty ? null : mimeType);
  var kind = _kindOf(mimeType, uri);
  if (isVideo && source.playable && kind == 'audio') kind = 'video';
  final playable = source.playable && kind != 'other';
  final downloadable = source.downloadable && (kind == 'audio' || kind == 'video');
  if (mimeType.isEmpty) {
    mimeType = switch (kind) {
      'video' => 'video/mp4',
      'hls' => 'application/vnd.apple.mpegurl',
      'dash' => 'application/dash+xml',
      _ => 'audio/mpeg',
    };
  }
  return MediaOption(
    id: 'primary',
    mimeType: mimeType,
    isDefault: false,
    kind: kind,
    sources: [source],
    playable: playable,
    downloadable: downloadable,
  );
}

MediaOption _alternateOption(int index, Map<String, dynamic> alt) {
  final mime = (alt['mime_type'] ?? alt['type'] ?? '').toString();
  final rawSources = alt['sources'];
  final sources = <MediaSource>[];
  if (rawSources is List) {
    for (final item in rawSources) {
      if (item is! Map) continue;
      final uri = (item['uri'] ?? '').toString();
      final contentType = item['content_type']?.toString();
      sources.add(_classify(uri, (contentType == null || contentType.isEmpty) ? mime : contentType));
    }
  }
  var kind = _kindOf(mime, sources.isEmpty ? '' : sources.first.uri);
  final upgraded = sources.any((s) => s.playable && _kindOf(s.contentType ?? mime, s.uri) == 'video');
  if (upgraded && kind == 'audio') kind = 'video';
  final playable = sources.any((s) => s.playable) && kind != 'other';
  final downloadable = sources.any((s) => s.downloadable) && (kind == 'audio' || kind == 'video');
  final integrityRaw = alt['integrity'];
  MediaIntegrity? integrity;
  if (integrityRaw is Map) {
    final kindName = (integrityRaw['type'] ?? '').toString();
    final value = (integrityRaw['value'] ?? '').toString();
    integrity = MediaIntegrity(
      kind: kindName,
      value: value,
      verifiable: _expectedDigests(kindName, value) != null,
    );
  }
  return MediaOption(
    id: 'alt-$index',
    title: _nonEmpty(alt['title']?.toString()),
    mimeType: mime,
    bitrate: _asInt(alt['bitrate']),
    height: _asInt(alt['height']),
    lang: _nonEmpty(alt['lang']?.toString()),
    codecs: _nonEmpty(alt['codecs']?.toString()),
    isDefault: alt['default'] == true,
    kind: kind,
    sources: sources,
    integrity: integrity,
    playable: playable,
    downloadable: downloadable,
  );
}

MediaSource _classify(String rawUri, String? contentType) {
  final uri = rawUri.trim();
  final scheme = uri.contains(':') ? uri.split(':').first.toLowerCase() : '';
  final mime = (contentType ?? '').trim();
  final kind = _kindOf(mime, uri);
  final torrent = _isTorrent(scheme, mime, uri);
  final ipfs = scheme == 'ipfs' || scheme == 'ipns';
  final http = scheme == 'http' || scheme == 'https';
  late final bool playable;
  late final bool downloadable;
  String? reason;
  if (uri.isEmpty) {
    playable = false;
    downloadable = false;
    reason = 'unsupported_scheme';
  } else if (ipfs) {
    playable = false;
    downloadable = false;
    reason = 'ipfs';
  } else if (torrent) {
    playable = false;
    downloadable = false;
    reason = 'torrent';
  } else if (!http) {
    playable = false;
    downloadable = false;
    reason = 'unsupported_scheme';
  } else if (kind == 'other') {
    playable = false;
    downloadable = false;
    reason = 'unsupported_type';
  } else if (kind == 'hls' || kind == 'dash') {
    playable = true;
    downloadable = false;
  } else {
    playable = true;
    downloadable = true;
  }
  return MediaSource(
    uri: uri,
    contentType: mime.isEmpty ? null : mime,
    scheme: scheme,
    playable: playable,
    downloadable: downloadable,
    unsupportedReason: reason,
  );
}

String _kindOf(String mime, String uri) {
  final m = mime.trim().toLowerCase();
  final path = uri.split(RegExp(r'[?#]')).first.toLowerCase();
  if (m.contains('mpegurl') || m.contains('vnd.apple.mpegurl') || path.endsWith('.m3u8') || path.endsWith('.m3u')) {
    return 'hls';
  }
  if (m.contains('dash') || path.endsWith('.mpd')) return 'dash';
  if (m.startsWith('video/') || _hasExt(path, const ['.mp4', '.webm', '.mkv', '.mov', '.m4v'])) return 'video';
  if (m.startsWith('audio/') || _hasExt(path, const ['.mp3', '.m4a', '.aac', '.ogg', '.opus', '.flac', '.wav', '.oga'])) {
    return 'audio';
  }
  if ((m.isEmpty || m == 'application/octet-stream') && (uri.startsWith('http://') || uri.startsWith('https://'))) {
    return 'audio';
  }
  return 'other';
}

bool _hasExt(String path, List<String> exts) => exts.any(path.endsWith);

bool _isTorrent(String scheme, String mime, String uri) {
  final path = uri.toLowerCase().split(RegExp(r'[?#]')).first;
  return scheme == 'magnet' ||
      scheme == 'torrent' ||
      mime.toLowerCase() == 'application/x-bittorrent' ||
      path.endsWith('.torrent');
}

String _selectId(List<MediaOption> options, String? explicit, bool musicAssistant) {
  if (explicit != null) {
    for (final option in options) {
      if (option.id == explicit && option.playable) return option.id;
    }
  }
  if (musicAssistant) {
    MediaOption? bestDefault;
    for (final option in options) {
      if (option.id != 'primary' && option.isDefault && option.downloadable) {
        if (bestDefault == null || _audioRank(option) > _audioRank(bestDefault)) bestDefault = option;
      }
    }
    if (bestDefault != null) return bestDefault.id;
    for (final option in options) {
      if (option.id == 'primary' && option.downloadable) return option.id;
    }
    MediaOption? best;
    for (final option in options) {
      if (option.downloadable && (best == null || _audioRank(option) > _audioRank(best))) best = option;
    }
    if (best != null) return best.id;
  }
  for (final option in options) {
    if (option.id != 'primary' && option.isDefault && option.playable) return option.id;
  }
  for (final option in options) {
    if (option.id == 'primary' && option.playable) return option.id;
  }
  for (final kind in const ['audio', 'video', 'hls', 'dash']) {
    for (final option in options) {
      if (option.playable && option.kind == kind) return option.id;
    }
  }
  return options.isEmpty ? 'primary' : options.first.id;
}

int _audioRank(MediaOption option) {
  final kind = switch (option.kind) {
    'audio' => 2000000000,
    'video' => 1000000000,
    _ => 0,
  };
  final bitrate = option.bitrate ?? 0;
  return kind + (bitrate > 1000000000 ? 1000000000 : bitrate);
}

List<PlaybackStep> _playbackChain(List<MediaOption> options, String selectedId) {
  final order = <MediaOption>[];
  for (final option in options) {
    if (option.id == selectedId) order.add(option);
  }
  final rest = options.where((o) => o.id != selectedId && o.playable).toList()
    ..sort((a, b) => _fallbackRank(a).compareTo(_fallbackRank(b)));
  order.addAll(rest);
  final seen = <String>{};
  final steps = <PlaybackStep>[];
  for (final option in order) {
    for (var i = 0; i < option.sources.length; i++) {
      final source = option.sources[i];
      if (!source.playable || !seen.add(source.uri)) continue;
      final mime = (source.contentType == null || source.contentType!.isEmpty) ? option.mimeType : source.contentType!;
      var kind = _kindOf(mime, source.uri);
      if (kind == 'audio' && option.kind == 'video') kind = 'video';
      if (kind == 'other') kind = option.kind;
      steps.add(PlaybackStep(
        optionId: option.id,
        sourceIndex: i,
        uri: source.uri,
        mimeType: mime,
        kind: kind,
        isVideo: kind == 'video' || ((option.height ?? 0) > 0 && kind != 'audio'),
      ));
    }
  }
  return steps;
}

int _fallbackRank(MediaOption option) {
  return switch (option.kind) {
    'audio' when option.downloadable => 0,
    'video' when option.downloadable => 1,
    'audio' => 2,
    'video' => 3,
    'hls' => 4,
    'dash' => 5,
    _ => 6,
  };
}

List<DownloadStep> _downloadChain(List<MediaOption> options, String selectedId, bool explicit) {
  final order = <MediaOption>[];
  for (final option in options) {
    if (option.id == selectedId) order.add(option);
  }
  if (!explicit) {
    final rest = options.where((o) => o.id != selectedId && o.downloadable).toList()
      ..sort((a, b) => _fallbackRank(a).compareTo(_fallbackRank(b)));
    order.addAll(rest);
  }
  final seen = <String>{};
  final steps = <DownloadStep>[];
  for (final option in order) {
    for (final source in option.sources) {
      if (!source.downloadable || !seen.add(source.uri)) continue;
      steps.add(DownloadStep(
        optionId: option.id,
        uri: source.uri,
        mimeType: (source.contentType == null || source.contentType!.isEmpty) ? option.mimeType : source.contentType!,
        integrity: option.integrity,
      ));
    }
  }
  return steps;
}

String? _nonEmpty(String? value) {
  if (value == null || value.trim().isEmpty) return null;
  return value;
}

int? _asInt(dynamic value) {
  if (value == null) return null;
  if (value is int) return value;
  if (value is num) return value.round();
  return int.tryParse(value.toString()) ?? double.tryParse(value.toString())?.round();
}

(String, List<List<int>>)? _expectedDigests(String kind, String value) {
  final normalized = kind.trim().toLowerCase();
  List<(String, List<int>)> tokens;
  if (normalized == 'sri' || normalized == 'subresourceintegrity' || normalized.isEmpty) {
    tokens = _sriTokens(value);
  } else if (normalized == 'sha256' || normalized == 'sha384' || normalized == 'sha512') {
    tokens = _bareDigest(normalized, value);
  } else {
    return null;
  }
  if (tokens.isEmpty) return null;
  var strongest = 0;
  for (final token in tokens) {
    final rank = _algoRank(token.$1);
    if (rank > strongest) strongest = rank;
  }
  final algo = tokens.firstWhere((t) => _algoRank(t.$1) == strongest).$1;
  final digests = tokens.where((t) => t.$1 == algo).map((t) => t.$2).toList();
  return (algo, digests);
}

int _algoRank(String algo) => switch (algo) {
      'sha256' => 1,
      'sha384' => 2,
      'sha512' => 3,
      _ => 0,
    };

int _algoLen(String algo) => switch (algo) {
      'sha256' => 32,
      'sha384' => 48,
      'sha512' => 64,
      _ => 0,
    };

List<(String, List<int>)> _sriTokens(String value) {
  final out = <(String, List<int>)>[];
  for (final token in value.split(RegExp(r'\s+'))) {
    final dash = token.indexOf('-');
    if (dash <= 0) continue;
    final name = token.substring(0, dash).toLowerCase();
    if (_algoRank(name) == 0) continue;
    final bytes = _b64(token.substring(dash + 1));
    if (bytes != null && bytes.length == _algoLen(name)) out.add((name, bytes));
  }
  return out;
}

List<(String, List<int>)> _bareDigest(String algo, String value) {
  final trimmed = value.trim();
  final dash = trimmed.indexOf('-');
  if (dash > 0 && trimmed.substring(0, dash).toLowerCase() == algo) {
    final bytes = _b64(trimmed.substring(dash + 1));
    if (bytes != null && bytes.length == _algoLen(algo)) return [(algo, bytes)];
  }
  final b64 = _b64(trimmed);
  if (b64 != null && b64.length == _algoLen(algo)) return [(algo, b64)];
  final hex = _hex(trimmed);
  if (hex != null && hex.length == _algoLen(algo)) return [(algo, hex)];
  return const [];
}

List<int>? _b64(String payload) {
  try {
    return base64.decode(payload);
  } catch (_) {
    try {
      return base64Url.decode(payload);
    } catch (_) {
      return null;
    }
  }
}

List<int>? _hex(String value) {
  final trimmed = value.trim();
  if (trimmed.length.isOdd || !RegExp(r'^[0-9a-fA-F]+$').hasMatch(trimmed)) return null;
  final out = <int>[];
  for (var i = 0; i < trimmed.length; i += 2) {
    out.add(int.parse(trimmed.substring(i, i + 2), radix: 16));
  }
  return out;
}

List<int> _digest(String algo, List<int> bytes) {
  final data = bytes is! List<int> ? bytes : bytes;
  return switch (algo) {
    'sha256' => sha256.convert(data).bytes,
    'sha384' => sha384.convert(data).bytes,
    'sha512' => sha512.convert(data).bytes,
    _ => sha256.convert(data).bytes,
  };
}

bool _bytesEqual(List<int> a, List<int> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}
