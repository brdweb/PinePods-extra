// Resolves a podroll remoteItem to a Podcast Index feed.
//
// Prefers a direct byguid / byfeedurl call when this build was given the same Podcast Index
// key and secret the search provider already uses (PINDEX_KEY / PINDEX_SECRET). Otherwise it
// uses the server's search proxy, which is the lookup the rest of the app already makes.

import 'dart:convert';

import 'package:crypto/crypto.dart';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/podcasting2.dart';

typedef IndexFetch = Future<Map<String, dynamic>?> Function(Uri uri, Map<String, String> headers);

/// SHA-1 of `key + secret + unix seconds`, matching podcast_search's Podcast Index auth header.
String podcastIndexAuth(String key, String secret, int unixSeconds) {
  return sha1.convert(utf8.encode('$key$secret$unixSeconds')).toString();
}

Future<IndexHit?> lookupPodcastIndex({
  required String key,
  required String secret,
  String? guid,
  String? feedUrl,
  String userAgent = 'Pinepods/0.9.0',
  DateTime Function()? now,
  IndexFetch? request,
}) async {
  if (key.isEmpty || secret.isEmpty) return null;
  final unix = (now ?? DateTime.now)().millisecondsSinceEpoch ~/ 1000;
  final headers = <String, String>{
    'User-Agent': userAgent,
    'X-Auth-Date': '$unix',
    'X-Auth-Key': key,
    'Authorization': podcastIndexAuth(key, secret, unix),
  };

  Future<IndexHit?> once(Uri uri) async {
    final Map<String, dynamic>? body;
    if (request != null) {
      body = await request(uri, headers);
    } else {
      final response = await http.get(uri, headers: headers).timeout(const Duration(seconds: 15));
      if (response.statusCode != 200) return null;
      final decoded = jsonDecode(response.body);
      body = decoded is Map<String, dynamic> ? decoded : null;
    }
    if (body == null) return null;
    return hitFromPodcastIndexBody(body);
  }

  final trimmedGuid = guid?.trim() ?? '';
  if (trimmedGuid.isNotEmpty) {
    final hit = await once(Uri.https('api.podcastindex.org', '/api/1.0/podcasts/byguid', {'guid': trimmedGuid}));
    if (hit != null && (hit.guid.isEmpty || hit.guid.toLowerCase() == trimmedGuid.toLowerCase())) {
      return hit;
    }
  }
  final url = feedUrl?.trim() ?? '';
  if (url.isNotEmpty) {
    return once(Uri.https('api.podcastindex.org', '/api/1.0/podcasts/byfeedurl', {'url': url}));
  }
  return null;
}

Future<IndexHit?> resolvePodroll({
  required PodrollItem item,
  required Future<Map<String, dynamic>> Function(String query) search,
  String indexKey = podcastIndexKey,
  String indexSecret = podcastIndexSecret,
  IndexFetch? request,
  DateTime Function()? now,
  String userAgent = 'Pinepods/0.9.0',
}) async {
  if (indexKey.isNotEmpty && indexSecret.isNotEmpty) {
    final direct = await lookupPodcastIndex(
      key: indexKey,
      secret: indexSecret,
      guid: item.feedGuid,
      feedUrl: item.feedUrl,
      request: request,
      now: now,
      userAgent: userAgent,
    );
    if (direct != null) return direct;
  }
  final query = lookupQuery(feedGuid: item.feedGuid, feedUrl: item.feedUrl);
  if (query.isEmpty) return null;
  try {
    final json = await search(query);
    return pickIndexHit(hitsFromSearchJson(json), feedGuid: item.feedGuid, feedUrl: item.feedUrl);
  } catch (_) {
    return null;
  }
}
