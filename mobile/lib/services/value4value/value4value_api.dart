import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class ValueApiException implements Exception {
  ValueApiException(this.message);
  final String message;
  @override
  String toString() => message;
}

class ValueSettings {
  ValueSettings({
    required this.enabled,
    required this.hasSecret,
    required this.maxBoostSats,
    required this.streamSatsPerMinute,
    required this.dailyLimitSats,
    required this.spentTodaySats,
  });

  final bool enabled;
  final bool hasSecret;
  final int maxBoostSats;
  final int streamSatsPerMinute;
  final int dailyLimitSats;
  final int spentTodaySats;

  factory ValueSettings.fromJson(Map<String, dynamic> json) => ValueSettings(
        enabled: json['enabled'] == true,
        hasSecret: json['has_secret'] == true,
        maxBoostSats: _int(json['max_boost_sats']),
        streamSatsPerMinute: _int(json['stream_sats_per_minute']),
        dailyLimitSats: _int(json['daily_limit_sats']),
        spentTodaySats: _int(json['spent_today_sats']),
      );
}

class ValuePreview {
  ValuePreview({
    required this.payable,
    required this.suggestedSats,
    required this.recipientCount,
  });

  final bool payable;
  final int? suggestedSats;
  final int recipientCount;

  factory ValuePreview.fromJson(Map<String, dynamic> json) => ValuePreview(
        payable: json['payable'] == true,
        suggestedSats: json['suggested_sats'] == null ? null : _int(json['suggested_sats']),
        recipientCount: _int(json['recipient_count']),
      );
}

class ValueSend {
  ValueSend({required this.sent, required this.reason, required this.satsSent});

  final bool sent;
  final String reason;
  final int satsSent;

  factory ValueSend.fromJson(Map<String, dynamic> json) => ValueSend(
        sent: json['sent'] == true,
        reason: (json['reason'] as String?) ?? '',
        satsSent: _int(json['sats_sent']),
      );
}

class ValueTest {
  ValueTest({required this.ok, required this.balanceSats, required this.detail});

  final bool ok;
  final int balanceSats;
  final String detail;

  factory ValueTest.fromJson(Map<String, dynamic> json) => ValueTest(
        ok: json['ok'] == true,
        balanceSats: _int(json['balance_sats']),
        detail: (json['detail'] as String?) ?? '',
      );
}

int _int(Object? value) {
  if (value is int) return value;
  if (value is num) return value.toInt();
  return int.tryParse('$value') ?? 0;
}

/// Signed-in PinePods account, when the server, key, and user id are all set.
({String server, String apiKey, int userId})? valueSessionOf(BuildContext context) {
  final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
  final server = settings.pinepodsServer;
  final apiKey = settings.pinepodsApiKey;
  final userId = settings.pinepodsUserId;
  if (server == null || server.isEmpty || apiKey == null || apiKey.isEmpty || userId == null) {
    return null;
  }
  return (server: server, apiKey: apiKey, userId: userId);
}

/// HTTP client for `/api/data/value/*`. The wallet string is never returned.
class ValueApi {
  ValueApi({
    required String server,
    required this.apiKey,
    http.Client? client,
  })  : server = server.trim().replaceAll(RegExp(r'/$'), ''),
        _client = client ?? _shared;

  static final http.Client _shared = http.Client();

  final String server;
  final String apiKey;
  final http.Client _client;

  Map<String, String> get _headers => {
        'Api-Key': apiKey,
        'Content-Type': 'application/json',
      };

  Future<ValueSettings> getSettings(int userId) async {
    final url = Uri.parse('$server/api/data/value/settings?user_id=$userId');
    final response = await _client.get(url, headers: {'Api-Key': apiKey});
    return ValueSettings.fromJson(_object(response));
  }

  Future<ValueSettings> saveSettings({
    required int userId,
    required bool enabled,
    required String secret,
    required bool clearSecret,
    required int maxBoostSats,
    required int streamSatsPerMinute,
    required int dailyLimitSats,
  }) async {
    final url = Uri.parse('$server/api/data/value/settings');
    final response = await _client.put(
      url,
      headers: _headers,
      body: jsonEncode({
        'user_id': userId,
        'enabled': enabled,
        'secret': secret.trim().isEmpty ? null : secret.trim(),
        'clear_secret': clearSecret,
        'max_boost_sats': maxBoostSats,
        'stream_sats_per_minute': streamSatsPerMinute,
        'daily_limit_sats': dailyLimitSats,
      }),
    );
    return ValueSettings.fromJson(_object(response));
  }

  Future<ValueTest> testWallet({required int userId, String secret = ''}) async {
    final url = Uri.parse('$server/api/data/value/test');
    final response = await _client.post(
      url,
      headers: _headers,
      body: jsonEncode({
        'user_id': userId,
        'secret': secret.trim().isEmpty ? null : secret.trim(),
      }),
    );
    return ValueTest.fromJson(_object(response));
  }

  Future<ValuePreview> previewEpisode({required int userId, required int episodeId}) async {
    final url = Uri.parse('$server/api/data/value/episode?user_id=$userId&episode_id=$episodeId');
    final response = await _client.get(url, headers: {'Api-Key': apiKey});
    return ValuePreview.fromJson(_object(response));
  }

  Future<ValueSend> boost({
    required int userId,
    required int episodeId,
    required int sats,
    required String message,
    required String senderName,
    required int positionSeconds,
  }) async {
    final url = Uri.parse('$server/api/data/value/boost');
    final response = await _client.post(
      url,
      headers: _headers,
      body: jsonEncode({
        'user_id': userId,
        'episode_id': episodeId,
        'sats': sats,
        'message': message,
        'sender_name': senderName,
        'position_seconds': positionSeconds,
      }),
    );
    return ValueSend.fromJson(_object(response));
  }

  Map<String, dynamic> _object(http.Response response) {
    Map<String, dynamic> body = {};
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map<String, dynamic>) body = decoded;
    } catch (_) {}
    if (response.statusCode < 200 || response.statusCode >= 300) {
      final message = body['message'] as String?;
      throw ValueApiException(
        (message == null || message.isEmpty) ? 'Request failed (${response.statusCode})' : message,
      );
    }
    return body;
  }
}
