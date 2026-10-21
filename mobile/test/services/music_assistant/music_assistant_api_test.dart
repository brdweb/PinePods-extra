import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/music_assistant/music_assistant_api.dart';

void main() {
  test('settings, test, players, and command use the Music Assistant routes', () async {
    final calls = <http.Request>[];
    final client = MockClient((request) async {
      calls.add(request);
      if (request.method == 'GET' && request.url.path.endsWith('/settings')) {
        return http.Response(
          jsonEncode({
            'enabled': true,
            'server_url': 'http://192.168.1.20:8095',
            'has_token': true,
            'connected': true,
            'server_version': '2.10.3',
          }),
          200,
        );
      }
      if (request.method == 'PUT') {
        return http.Response(
          jsonEncode({'enabled': true, 'has_token': true, 'connected': false}),
          200,
        );
      }
      if (request.url.path.endsWith('/test')) {
        return http.Response(
          jsonEncode({'ok': true, 'detail': 'connected, 1 player(s)', 'player_count': 1}),
          200,
        );
      }
      if (request.method == 'GET' && request.url.path.endsWith('/players')) {
        return http.Response(
          jsonEncode({'enabled': true, 'connected': true, 'players': []}),
          200,
        );
      }
      return http.Response(
        jsonEncode({'ok': true, 'detail': 'queued', 'warnings': ['seek was late']}),
        200,
      );
    });
    final api = MusicAssistantApi(
      server: 'https://pods.example.com/',
      apiKey: 'secret',
      client: client,
    );

    final settings = await api.getSettings(7);
    expect(settings.enabled, isTrue);
    expect(settings.serverVersion, '2.10.3');
    expect(calls.single.headers['Api-Key'], 'secret');
    expect(calls.single.url.path, '/api/data/music_assistant/settings');
    expect(calls.single.url.queryParameters['user_id'], '7');

    await api.saveSettings(
      userId: 7,
      enabled: true,
      serverUrl: ' http://192.168.1.20:8095 ',
      token: '   ',
    );
    final saved = jsonDecode(calls[1].body) as Map<String, dynamic>;
    expect(saved['server_url'], 'http://192.168.1.20:8095');
    expect(saved['token'], isNull);
    expect(saved['clear_token'], isFalse);

    final report = await api.testConnection(
      userId: 7,
      serverUrl: 'http://192.168.1.20:8095',
      externalBaseUrl: 'http://192.168.1.10:8040',
    );
    expect(report.ok, isTrue);
    expect(report.playerCount, 1);
    final probed = jsonDecode(calls[2].body) as Map<String, dynamic>;
    expect(probed['external_base_url'], 'http://192.168.1.10:8040');

    final players = await api.listPlayers(7);
    expect(players.players, isEmpty);

    final command = await api.command(
      userId: 7,
      playerId: 'kitchen',
      action: 'play_episode',
      args: {'episode_id': 4, 'is_youtube': false},
    );
    expect(command.warnings, ['seek was late']);
    final sent = jsonDecode(calls[4].body) as Map<String, dynamic>;
    expect(sent['action'], 'play_episode');
    expect(sent['player_id'], 'kitchen');
    expect(sent['args']['episode_id'], 4);
  });

  test('an error response uses the JSON message', () async {
    final client = MockClient((request) async {
      return http.Response(
        jsonEncode({
          'error': 'Bad request',
          'message': 'Bad request: external base URL must be a LAN or public host, not localhost',
          'status_code': 400,
        }),
        400,
      );
    });
    final api = MusicAssistantApi(
      server: 'https://pods.example.com',
      apiKey: 'secret',
      client: client,
    );
    expect(
      () => api.testConnection(userId: 7, externalBaseUrl: 'http://localhost:8040'),
      throwsA(
        isA<MaApiException>().having(
          (err) => err.message,
          'message',
          contains('not localhost'),
        ),
      ),
    );
  });
}
