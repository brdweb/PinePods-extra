import 'dart:convert';

import 'package:crypto/crypto.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/alternate_media.dart';

Map<String, dynamic> alt(String mime, bool isDefault, List<String> uris, {String? integrityType, String? integrityValue, String? height}) {
  return {
    'mime_type': mime,
    'default': isDefault,
    'title': 'Low',
    'bitrate': '64000',
    if (height != null) 'height': height,
    'sources': uris.map((u) => {'uri': u}).toList(),
    if (integrityType != null) 'integrity': {'type': integrityType, 'value': integrityValue},
  };
}

void main() {
  test('default alternate beats the rss enclosure and hides ipfs from playback', () {
    final plan = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: [
        alt('audio/mpeg', true, ['https://mirror.example/ep.mp3', 'ipfs://bafy']),
      ],
    );
    expect(plan.selectedId, 'alt-0');
    expect(plan.playback.first.uri, 'https://mirror.example/ep.mp3');
    expect(plan.playback.every((s) => s.uri.startsWith('https://')), isTrue);
    expect(plan.options[1].sources[1].playable, isFalse);
    expect(plan.options[1].sources[1].unsupportedReason, 'ipfs');
    expect(mediaOptionLabel(plan.options[1]), contains('IPFS'));
  });

  test('rss enclosure wins when nothing is marked default', () {
    final plan = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: [alt('audio/mpeg', false, ['https://mirror.example/low.mp3'])],
    );
    expect(plan.selectedId, 'primary');
    expect(plan.playback[0].uri, 'https://cdn.example/ep.mp3');
    expect(plan.playback[1].uri, 'https://mirror.example/low.mp3');
  });

  test('torrent is shown and not played', () {
    final plan = planMedia(
      primaryUri: 'magnet:?xt=urn:btih:abc',
      primaryMime: 'application/x-bittorrent',
      alternates: [
        alt('audio/mpeg', false, ['https://cdn.example/a.mp3', 'https://cdn.example/a.torrent']),
      ],
    );
    expect(plan.options.first.playable, isFalse);
    expect(plan.options.first.sources.first.unsupportedReason, 'torrent');
    expect(plan.selectedId, 'alt-0');
    expect(plan.playback, hasLength(1));
    expect(plan.playback.first.uri, 'https://cdn.example/a.mp3');
    expect(plan.options[1].sources[1].unsupportedReason, 'torrent');
  });

  test('explicit choice limits the download to that version', () {
    final plan = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: [
        alt('audio/mpeg', true, ['https://mirror.example/def.mp3']),
        alt('audio/mp4', false, ['https://cdn.example/hq.m4a', 'https://backup.example/hq.m4a']),
      ],
      preferOptionId: 'alt-1',
    );
    expect(plan.selectedId, 'alt-1');
    expect(plan.download, hasLength(2));
    expect(plan.download.every((d) => d.optionId == 'alt-1'), isTrue);
    expect(plan.playback.first.optionId, 'alt-1');
    expect(plan.playback.any((s) => s.optionId == 'primary'), isTrue);
  });

  test('hls cannot be saved as a file and music assistant prefers progressive audio', () {
    final alternates = [alt('application/vnd.apple.mpegurl', true, ['https://cdn.example/live.m3u8'])];
    final chosen = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: alternates,
      preferOptionId: 'alt-0',
    );
    expect(chosen.options[1].playable, isTrue);
    expect(chosen.options[1].downloadable, isFalse);
    expect(chosen.download, isEmpty);
    expect(chosen.playback.first.kind, 'hls');

    final ma = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: alternates,
      musicAssistant: true,
    );
    expect(ma.selectedId, 'primary');
    expect(ma.playback.first.uri, 'https://cdn.example/ep.mp3');
  });

  test('auto download falls back to another progressive source', () {
    final plan = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: [alt('audio/mpeg', true, ['https://mirror.example/ep.mp3'])],
    );
    expect(plan.download[0].uri, 'https://mirror.example/ep.mp3');
    expect(plan.download[1].uri, 'https://cdn.example/ep.mp3');
  });

  test('video height marks hls as video', () {
    final plan = planMedia(
      primaryUri: 'https://cdn.example/ep.mp3',
      primaryMime: 'audio/mpeg',
      alternates: [
        alt('application/vnd.apple.mpegurl', true, ['https://cdn.example/v.m3u8'], height: '720'),
      ],
    );
    expect(plan.playback.first.isVideo, isTrue);
  });

  test('sri matches sha384 and rejects a different payload', () {
    final bytes = utf8.encode('pinepods-enclosure');
    final token = 'sha384-${base64.encode(sha384.convert(bytes).bytes)}';
    expect(sriMatches(bytes, 'sri', token), isTrue);
    expect(sriMatches(utf8.encode('other-bytes'), 'sri', token), isFalse);
  });

  test('sri uses the strongest algorithm present', () {
    final bytes = utf8.encode('abc');
    final sha256Token = 'sha256-${base64.encode(sha256.convert(bytes).bytes)}';
    final wrong512 = 'sha512-${base64.encode(sha512.convert(utf8.encode('not-the-file')).bytes)}';
    expect(sriMatches(bytes, 'SRI', '$sha256Token $wrong512'), isFalse);
    final ok512 = 'sha512-${base64.encode(sha512.convert(bytes).bytes)}';
    expect(sriMatches(bytes, 'sri', '$sha256Token $ok512'), isTrue);
  });

  test('tags json from the podcasting 2.0 API feeds the planner', () {
    final plan = planFromTags(
      primaryUri: 'https://cdn.example/ep.mp3',
      tags: {
        'alternate_enclosures': [
          alt('audio/mpeg', true, ['https://mirror.example/ep.mp3']),
        ],
      },
    );
    expect(plan.selectedId, 'alt-0');
    expect(plan.playback.first.uri, 'https://mirror.example/ep.mp3');
  });
}
