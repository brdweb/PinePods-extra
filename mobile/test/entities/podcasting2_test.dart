import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/podcasting2.dart';
import 'package:pinepods_mobile/services/podcast/podroll_lookup.dart';

void main() {
  test('parses the channel payload the server already returns', () {
    final channel = Podcasting2Channel.fromJson({
      'people': [],
      'podroll': [
        {'feed_guid': 'g-1'},
        {'feedGuid': 'g-2', 'feedUrl': 'https://example.com/rss', 'title': 'Friend'},
      ],
      'funding': [],
      'value': [
        {
          'type': 'lightning',
          'method': 'keysend',
          'suggested': '5000',
          'recipients': [
            {'name': 'Alice', 'type': 'node', 'address': '03ab', 'split': '90'},
            {'name': '', 'type': 'node', 'address': '02cd', 'split': '10%'},
          ],
        },
      ],
    });

    expect(channel.podroll, hasLength(2));
    expect(channel.podroll[0].feedUrl, isNull);
    expect(channel.podroll[1].feedUrl, 'https://example.com/rss');
    expect(channel.podroll[1].title, 'Friend');
    expect(channel.value.single.methodLine('Suggested'), 'lightning · keysend · Suggested 5000');
    expect(channel.value.single.recipients[0].splitLabel, '90%');
    expect(channel.value.single.recipients[1].displayName, 'node');
    expect(channel.value.single.recipients[1].splitLabel, '10%');
  });

  test('formatSplit appends a percent once', () {
    expect(formatSplit('90'), '90%');
    expect(formatSplit(' 33.5 '), '33.5%');
    expect(formatSplit('90%'), '90%');
    expect(formatSplit(''), '');
    expect(formatSplit('node'), 'node');
  });

  test('lookup query prefers a feed URL', () {
    expect(lookupQuery(feedGuid: 'guid', feedUrl: ' https://example.com/rss '), 'https://example.com/rss');
    expect(lookupQuery(feedGuid: ' guid ', feedUrl: ' '), 'guid');
  });

  test('pick prefers a guid match and otherwise a feed URL', () {
    final wrong = _hit(guid: 'other', url: 'https://example.com/rss', title: 'Wrong');
    final right = _hit(guid: 'GUID-1', url: 'https://other.example/rss', title: 'Right');
    expect(
      pickIndexHit([wrong, right], feedGuid: 'guid-1', feedUrl: 'https://example.com/rss')?.title,
      'Right',
    );

    final byUrl = _hit(guid: '', url: 'https://example.com/rss/', title: 'By URL');
    expect(
      pickIndexHit([byUrl], feedGuid: 'missing', feedUrl: 'https://example.com/rss')?.title,
      'By URL',
    );

    final loose = [
      _hit(guid: '', url: 'https://a.example/rss', title: 'A'),
      _hit(guid: '', url: 'https://b.example/rss', title: 'B'),
    ];
    expect(pickIndexHit(loose, feedGuid: 'missing-guid'), isNull);
  });

  test('titles fall back from name to host to a short guid', () {
    expect(podrollTitle(resolved: 'Show', feedUrl: 'https://ex.com/rss', feedGuid: 'g'), 'Show');
    expect(podrollTitle(feedUrl: 'https://feeds.ex.com/rss', feedGuid: 'g'), 'feeds.ex.com');
    expect(podrollTitle(feedGuid: '1234567890abcdef'), '12345678…');
  });

  test('direct Podcast Index lookup signs the request the way podcast_search does', () async {
    Uri? seen;
    Map<String, String>? headers;
    final hit = await lookupPodcastIndex(
      key: 'key',
      secret: 'secret',
      guid: 'abc',
      now: () => DateTime.fromMillisecondsSinceEpoch(1000 * 1000),
      request: (uri, sent) async {
        seen = uri;
        headers = sent;
        return {
          'status': 'true',
          'feed': {
            'id': 9,
            'title': 'Show',
            'url': 'https://ex/rss',
            'podcastGuid': 'abc',
            'artwork': 'https://ex/art.jpg',
            'author': 'Ada',
          },
        };
      },
    );

    expect(hit?.title, 'Show');
    expect(hit?.artwork, 'https://ex/art.jpg');
    expect(seen.toString(), contains('/podcasts/byguid'));
    expect(seen!.queryParameters['guid'], 'abc');
    expect(headers?['X-Auth-Key'], 'key');
    expect(headers?['X-Auth-Date'], '1000');
    expect(headers?['Authorization'], podcastIndexAuth('key', 'secret', 1000));
  });

  test('resolve uses the search proxy when Podcast Index keys are absent', () async {
    final hit = await resolvePodroll(
      item: const PodrollItem(feedGuid: 'g-1', feedUrl: 'https://example.com/rss'),
      indexKey: '',
      indexSecret: '',
      search: (query) async {
        expect(query, 'https://example.com/rss');
        return {
          'feeds': [
            {'id': 4, 'title': 'From search', 'url': 'https://example.com/rss', 'podcastGuid': 'g-1'},
          ],
        };
      },
    );
    expect(hit?.title, 'From search');
    expect(hit?.id, 4);
  });

  test('resolve prefers a direct guid lookup when credentials are available', () async {
    var searched = false;
    final hit = await resolvePodroll(
      item: const PodrollItem(feedGuid: 'abc'),
      indexKey: 'key',
      indexSecret: 'secret',
      search: (_) async {
        searched = true;
        return {'feeds': []};
      },
      request: (uri, _) async {
        expect(uri.path, contains('byguid'));
        return {
          'feed': {'id': 3, 'title': 'Direct', 'url': 'https://direct.example/rss', 'podcastGuid': 'abc'},
        };
      },
    );
    expect(hit?.title, 'Direct');
    expect(searched, isFalse);
  });
}

IndexHit _hit({required String guid, required String url, required String title}) {
  return IndexHit(
    guid: guid,
    title: title,
    url: url,
    originalUrl: url,
    artwork: '',
    author: '',
    description: '',
    link: '',
    id: 1,
    episodeCount: 2,
    explicit: false,
    categories: const {},
  );
}
