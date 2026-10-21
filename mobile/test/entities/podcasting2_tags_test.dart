import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/podcasting2_tags.dart';

void main() {
  test('podcasting 2 numbers win and itunes fills gaps', () {
    final both = resolveNumbers(
      season: const P2Season(number: '2', name: 'City'),
      episode: const P2EpisodeNumber(number: '4', display: null),
      itunesSeason: 9,
      itunesEpisode: 8,
    );
    expect(both.seasonNumber, 2);
    expect(both.episodeNumber, 4);
    expect(both.fromPodcasting2, isTrue);

    final itunes = resolveNumbers(itunesSeason: 3, itunesEpisode: 7);
    expect(itunes.seasonNumber, 3);
    expect(itunes.episodeText, '7');
    expect(itunes.fromPodcasting2, isFalse);
    expect(
      formatNumbersBadge('Season', 'Episode', itunes),
      'Season 3 · Episode 7',
    );
  });

  test('unnumbered episodes sort after numbered seasons', () {
    final numbered = seasonSortKey(
      resolveNumbers(itunesSeason: 1, itunesEpisode: 2),
    );
    final later = seasonSortKey(
      resolveNumbers(itunesSeason: 1, itunesEpisode: 10),
    );
    final unnumbered = seasonSortKey(const ResolvedNumbers());
    expect(numbered.$1, 0);
    expect(numbered.$3 < later.$3, isTrue);
    expect(later.$1 < unnumbered.$1, isTrue);
    expect(
      shouldGroupBySeason([
        resolveNumbers(itunesSeason: 1, itunesEpisode: 1),
        const ResolvedNumbers(),
      ]),
      isTrue,
    );
  });

  test('soundbite window and clock formatting', () {
    final window = soundbiteWindow('73.5', '60');
    expect(window, isNotNull);
    expect(window!.start, 73.5);
    expect(window.end, 133.5);
    expect(formatClock(73), '1:13');
    expect(formatClock(3661), '1:01:01');
    expect(soundbiteWindow('nope', '1'), isNull);
  });

  test('map links prefer geo and fall back to an OSM id', () {
    expect(
      locationMapUrl(
        const P2Location(name: 'Studio', geo: 'geo:40.7128,-74.006'),
      ),
      'https://www.openstreetmap.org/?mlat=40.7128&mlon=-74.006#map=14/40.7128/-74.006',
    );
    expect(
      locationMapUrl(const P2Location(name: 'Way', osm: 'W123')),
      'https://www.openstreetmap.org/way/123',
    );
    expect(locationMapUrl(const P2Location(name: 'Nowhere')), isNull);
  });

  test('chat and social links skip dangerous and disabled targets', () {
    expect(
      chatUrl(
        const P2Chat(
          server: 'irc.example.com',
          protocol: 'irc',
          space: '#pods',
        ),
      ),
      'irc://irc.example.com/pods',
    );
    expect(
      socialUrl(
        const SocialInteract(
          protocol: 'disabled',
          uri: 'https://example.com/hidden',
        ),
      ),
      isNull,
    );
    expect(
      socialUrl(
        const SocialInteract(
          protocol: 'activitypub',
          uri: 'https://example.com/thread',
        ),
      ),
      'https://example.com/thread',
    );
    expect(safeLink('javascript:alert(1)'), isNull);
    expect(safeLink('ipfs://bafy'), 'ipfs://bafy');
    expect(httpLink('ipfs://bafy'), isNull);
  });

  test('artwork prefers cover art and drops unsafe srcset urls', () {
    final choice = artworkChoice(
      const [
        P2Image(href: 'https://cdn.example/social.jpg', purpose: 'social'),
        P2Image(
          href: 'https://cdn.example/cover.jpg',
          purpose: 'artwork',
          alt: 'Cover',
        ),
      ],
      const ImageSet(
        srcset:
            'javascript:alert(1) 100w, https://cdn.example/a.jpg 200w, https://cdn.example/b.jpg 800w',
      ),
      'https://fallback.example/art.jpg',
    );
    expect(choice.src, 'https://cdn.example/cover.jpg');
    expect(
      choice.srcset,
      'https://cdn.example/a.jpg 200w, https://cdn.example/b.jpg 800w',
    );
    expect(choice.alt, 'Cover');

    final srcsetOnly = artworkChoice(
      const [],
      const ImageSet(
        srcset:
            'https://cdn.example/small.jpg 100w, https://cdn.example/large.jpg 900w',
      ),
      'https://fallback.example/art.jpg',
    );
    expect(srcsetOnly.src, 'https://cdn.example/large.jpg');
  });

  test('schedule cadence and time splits', () {
    expect(cadenceOf('FREQ=WEEKLY;INTERVAL=2')?.unit, CadenceUnit.weekly);
    expect(cadenceOf('FREQ=WEEKLY;INTERVAL=2')?.interval, 2);
    expect(cadenceOf('FREQ=DAILY')?.unit, CadenceUnit.daily);
    expect(cadenceOf('FREQ=DAILY')?.interval, 1);
    final splits = timeSplitsOf(const [
      ValueBlock(
        timeSplits: [
          ValueTimeSplit(
            startTime: '10',
            duration: '30',
            remotePercentage: '40',
          ),
        ],
      ),
    ]);
    expect(splits, hasLength(1));
    expect(splits.first.remotePercentage, '40');
  });

  test(
    'sample tags and chapters parse, and a legacy payload without tags is empty',
    () {
      final item = ItemTags.fromJson({
        'season': {'number': '1', 'name': 'One'},
        'episode': {'number': '2'},
        'medium': 'podcast',
        'soundbites': [
          {'start_time': '73', 'duration': '60', 'title': 'Hook'},
        ],
        'locations': [
          {'name': 'Studio', 'geo': '40.7,-74.0', 'country': 'US'},
        ],
        'license': {
          'identifier': 'CC-BY',
          'url': 'https://example.com/license',
        },
        'content_links': [
          {'href': 'https://example.com/notes', 'text': 'Notes'},
        ],
        'social_interacts': [
          {'protocol': 'disabled', 'uri': 'https://example.com/off'},
          {'protocol': 'activitypub', 'uri': 'https://example.com/thread'},
        ],
      });
      expect(item.season?.number, '1');
      expect(item.soundbites.single.window?.start, 73);
      expect(item.socialInteracts[0].link, isNull);
      expect(item.socialInteracts[1].link, 'https://example.com/thread');
      expect(item.isEmpty, isFalse);

      final chapters = chaptersFromPodcasting2({
        'chapters': [
          {
            'title': 'Start',
            'startTime': 0,
            'img': 'https://cdn.example/c.jpg',
            'url': 'https://example.com/c',
          },
          {'title': 'Skip', 'startTime': 12, 'url': 'javascript:alert(1)'},
        ],
      });
      expect(chapters.first.imageUrl, 'https://cdn.example/c.jpg');
      expect(chapters.first.url, 'https://example.com/c');
      expect(chapters[1].url, isNull);

      expect(ItemTags.fromJson(null).isEmpty, isTrue);
      expect(ChannelTags.fromJson(null).isEmpty, isTrue);
    },
  );
}
