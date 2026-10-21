import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';
import 'package:provider/provider.dart';
import 'package:flutter/widgets.dart';

class MaApiException implements Exception {
  MaApiException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

/// HTTP client for `/api/data/music_assistant/*`. The token never comes back
/// from the server; [saveSettings] omits a blank token so the stored one stays.
class MusicAssistantApi {
  MusicAssistantApi({
    required String server,
    required this.apiKey,
    http.Client? client,
    Duration? timeout,
  })  : server = server.trim().replaceAll(RegExp(r'/$'), ''),
        _client = client ?? _shared,
        _timeout = timeout ?? const Duration(seconds: 15);

  static final http.Client _shared = http.Client();

  final String server;
  final String apiKey;
  final http.Client _client;
  final Duration _timeout;

  Map<String, String> get _headers => {
        'Api-Key': apiKey,
        'Content-Type': 'application/json',
      };

  Future<MaSettings> getSettings(int userId) async {
    final url = Uri.parse(
      '$server/api/data/music_assistant/settings?user_id=$userId',
    );
    final response = await _client.get(url, headers: {'Api-Key': apiKey}).timeout(_timeout);
    return MaSettings.fromJson(_object(response));
  }

  Future<MaSettings> saveSettings({
    required int userId,
    required bool enabled,
    String serverUrl = '',
    String externalBaseUrl = '',
    String token = '',
    bool clearToken = false,
  }) async {
    final url = Uri.parse('$server/api/data/music_assistant/settings');
    final response = await _client
        .put(
          url,
          headers: _headers,
          body: jsonEncode({
            'user_id': userId,
            'enabled': enabled,
            'server_url': _nonempty(serverUrl),
            'external_base_url': _nonempty(externalBaseUrl),
            'token': _nonempty(token),
            'clear_token': clearToken,
          }),
        )
        .timeout(_timeout);
    return MaSettings.fromJson(_object(response));
  }

  Future<MaTestReport> testConnection({
    required int userId,
    String serverUrl = '',
    String externalBaseUrl = '',
    String token = '',
  }) async {
    final url = Uri.parse('$server/api/data/music_assistant/test');
    final response = await _client
        .post(
          url,
          headers: _headers,
          body: jsonEncode({
            'user_id': userId,
            'server_url': _nonempty(serverUrl),
            'external_base_url': _nonempty(externalBaseUrl),
            'token': _nonempty(token),
          }),
        )
        .timeout(_timeout);
    return MaTestReport.fromJson(_object(response));
  }

  Future<MaPlayersResponse> listPlayers(int userId) async {
    final url = Uri.parse(
      '$server/api/data/music_assistant/players?user_id=$userId',
    );
    final response = await _client.get(url, headers: {'Api-Key': apiKey}).timeout(_timeout);
    return MaPlayersResponse.fromJson(_object(response));
  }

  Future<MaCommandResponse> command({
    required int userId,
    required String playerId,
    required String action,
    Map<String, dynamic> args = const {},
  }) async {
    final url = Uri.parse('$server/api/data/music_assistant/command');
    final response = await _client
        .post(
          url,
          headers: _headers,
          body: jsonEncode({
            'user_id': userId,
            'player_id': playerId,
            'action': action,
            'args': args,
          }),
        )
        .timeout(_timeout);
    return MaCommandResponse.fromJson(_object(response));
  }

  Map<String, dynamic> _object(http.Response response) {
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw MaApiException(
        messageFromErrorBody(response.body),
        statusCode: response.statusCode,
      );
    }
    final decoded = jsonDecode(response.body);
    if (decoded is Map) return decoded.cast<String, dynamic>();
    throw MaApiException('Unexpected Music Assistant response');
  }
}

String? _nonempty(String value) {
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}

/// Last fetched bridge state. Speaker UI listens to [enabled].
class MusicAssistantController {
  MusicAssistantController._();

  static final instance = MusicAssistantController._();

  final ValueNotifier<bool> enabled = ValueNotifier<bool>(false);
  final ValueNotifier<bool> connected = ValueNotifier<bool>(false);
  final ValueNotifier<List<MaPlayer>> players = ValueNotifier<List<MaPlayer>>(const []);
  final ValueNotifier<String?> focusDeviceId = ValueNotifier<String?>(null);

  DateTime? _lastFetch;

  void applySettings(MaSettings view) {
    enabled.value = view.enabled;
    connected.value = view.connected;
    if (!view.enabled) {
      players.value = const [];
    }
  }

  void applyPlayers(MaPlayersResponse list) {
    enabled.value = list.enabled;
    connected.value = list.connected;
    players.value = list.enabled ? list.players : const [];
  }

  /// Loads settings (and players when the bridge is on). Skips repeat calls
  /// inside [minInterval] unless [force] is set.
  Future<void> refresh(
    MusicAssistantApi api,
    int userId, {
    bool force = false,
    Duration minInterval = const Duration(seconds: 8),
  }) async {
    final last = _lastFetch;
    if (!force && last != null && DateTime.now().difference(last) < minInterval) {
      return;
    }
    _lastFetch = DateTime.now();
    final view = await api.getSettings(userId);
    applySettings(view);
    if (view.enabled) {
      applyPlayers(await api.listPlayers(userId));
    }
  }
}

/// Credentials from the signed-in PinePods account, when all three are present.
({String server, String apiKey, int userId})? maSessionOf(BuildContext context) {
  final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
  final server = settings.pinepodsServer;
  final apiKey = settings.pinepodsApiKey;
  final userId = settings.pinepodsUserId;
  if (server == null || server.isEmpty || apiKey == null || apiKey.isEmpty || userId == null) {
    return null;
  }
  return (server: server, apiKey: apiKey, userId: userId);
}
