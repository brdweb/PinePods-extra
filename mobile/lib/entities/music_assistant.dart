// Music Assistant models and the UI decisions shared by settings, the speaker
// sheet, and the now-playing bar. Mirrors web/src/requests/music_assistant.rs.

import 'dart:convert';

class MaSettings {
  final bool enabled;
  final String? serverUrl;
  final String? externalBaseUrl;
  final bool hasToken;
  final bool connected;
  final String? serverVersion;
  final int? schemaVersion;
  final String? warning;
  final String? lastError;

  const MaSettings({
    required this.enabled,
    this.serverUrl,
    this.externalBaseUrl,
    this.hasToken = false,
    this.connected = false,
    this.serverVersion,
    this.schemaVersion,
    this.warning,
    this.lastError,
  });

  factory MaSettings.fromJson(Map<String, dynamic> json) {
    return MaSettings(
      enabled: json['enabled'] == true,
      serverUrl: json['server_url'] as String?,
      externalBaseUrl: json['external_base_url'] as String?,
      hasToken: json['has_token'] == true,
      connected: json['connected'] == true,
      serverVersion: json['server_version'] as String?,
      schemaVersion: (json['schema_version'] as num?)?.toInt(),
      warning: json['warning'] as String?,
      lastError: json['last_error'] as String?,
    );
  }
}

class MaPlayer {
  final String playerId;
  final String deviceId;
  final String name;
  final String kind;
  final bool available;
  final String state;
  final double? volumeLevel;
  final bool volumeMuted;
  final List<String> groupMembers;
  final String? syncedTo;
  final List<String> canGroupWith;
  final String queueId;

  const MaPlayer({
    required this.playerId,
    required this.deviceId,
    required this.name,
    required this.kind,
    this.available = false,
    this.state = '',
    this.volumeLevel,
    this.volumeMuted = false,
    this.groupMembers = const [],
    this.syncedTo,
    this.canGroupWith = const [],
    this.queueId = '',
  });

  bool get isGroup =>
      kind.toLowerCase() == 'group' || groupMembers.isNotEmpty;

  factory MaPlayer.fromJson(Map<String, dynamic> json) {
    return MaPlayer(
      playerId: json['player_id'] as String? ?? '',
      deviceId: json['device_id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      kind: json['kind'] as String? ?? 'player',
      available: json['available'] == true,
      state: json['state'] as String? ?? '',
      volumeLevel: json['volume_level'] is num
          ? (json['volume_level'] as num).toDouble()
          : null,
      volumeMuted: json['volume_muted'] == true,
      groupMembers: _strings(json['group_members']),
      syncedTo: json['synced_to'] as String?,
      canGroupWith: _strings(json['can_group_with']),
      queueId: json['queue_id'] as String? ?? '',
    );
  }
}

class MaPlayersResponse {
  final bool enabled;
  final bool connected;
  final List<MaPlayer> players;

  const MaPlayersResponse({
    required this.enabled,
    required this.connected,
    required this.players,
  });

  factory MaPlayersResponse.fromJson(Map<String, dynamic> json) {
    final raw = json['players'];
    return MaPlayersResponse(
      enabled: json['enabled'] == true,
      connected: json['connected'] == true,
      players: raw is List
          ? raw
              .whereType<Map>()
              .map((row) => MaPlayer.fromJson(row.cast<String, dynamic>()))
              .toList()
          : const [],
    );
  }
}

class MaTestReport {
  final bool ok;
  final String detail;
  final String? serverVersion;
  final int? schemaVersion;
  final int playerCount;
  final String? warning;

  const MaTestReport({
    required this.ok,
    required this.detail,
    this.serverVersion,
    this.schemaVersion,
    this.playerCount = 0,
    this.warning,
  });

  factory MaTestReport.fromJson(Map<String, dynamic> json) {
    return MaTestReport(
      ok: json['ok'] == true,
      detail: json['detail'] as String? ?? '',
      serverVersion: json['server_version'] as String?,
      schemaVersion: (json['schema_version'] as num?)?.toInt(),
      playerCount: (json['player_count'] as num?)?.toInt() ?? 0,
      warning: json['warning'] as String?,
    );
  }
}

class MaCommandResponse {
  final bool ok;
  final String detail;
  final List<String> warnings;

  const MaCommandResponse({
    required this.ok,
    required this.detail,
    this.warnings = const [],
  });

  factory MaCommandResponse.fromJson(Map<String, dynamic> json) {
    return MaCommandResponse(
      ok: json['ok'] == true,
      detail: json['detail'] as String? ?? '',
      warnings: _strings(json['warnings']),
    );
  }
}

List<String> _strings(dynamic raw) {
  if (raw is! List) return const [];
  return raw.map((entry) => entry.toString()).toList();
}

enum MaProblem {
  localhostRejected,
  unreachable,
  streamHint,
  notConfigured,
  authFailed,
  other,
}

class TestExplanation {
  final bool ok;
  final List<MaProblem> problems;
  final bool showStreamHint;
  final String detail;

  const TestExplanation({
    required this.ok,
    required this.problems,
    required this.showStreamHint,
    required this.detail,
  });
}

String messageFromErrorBody(String body) {
  final trimmed = body.trim();
  try {
    final decoded = jsonDecode(trimmed);
    if (decoded is Map) {
      final message = decoded['message'];
      if (message is String && message.trim().isNotEmpty) {
        return message.trim();
      }
    }
  } catch (_) {}
  return trimmed;
}

String externalHost(String raw) {
  final trimmed = raw.trim();
  final afterScheme = trimmed.contains('://')
      ? trimmed.split('://').skip(1).join('://')
      : trimmed;
  final hostport = afterScheme.split(RegExp(r'[/?#]')).first;
  String host;
  if (hostport.startsWith('[')) {
    final end = hostport.indexOf(']');
    host = end >= 0 ? hostport.substring(1, end) : hostport;
  } else {
    host = hostport.split(':').first;
  }
  return host.trim().toLowerCase();
}

bool externalBaseIsRejected(String raw) {
  final host = externalHost(raw);
  if (host.isEmpty) return false;
  if (host == 'localhost' ||
      host == 'localhost.localdomain' ||
      host.endsWith('.localhost') ||
      host == 'metadata.google.internal' ||
      host.endsWith('.metadata.google.internal')) {
    return true;
  }
  if (host == '127.0.0.1' ||
      host == '::1' ||
      host == '0.0.0.0' ||
      host == '::') {
    return true;
  }
  if (host.startsWith('169.254.')) return true;
  final v6 = host.split('%').first;
  return v6.startsWith('fe80:');
}

MaProblem classifyMaText(String text) {
  final lower = text.toLowerCase();
  if (lower.contains('localhost') ||
      lower.contains('loopback') ||
      lower.contains('link-local') ||
      lower.contains('must be a lan') ||
      lower.contains('music assistant can reach')) {
    return MaProblem.localhostRejected;
  }
  if (lower.contains('could not connect') ||
      lower.contains('timed out') ||
      lower.contains('connection refused') ||
      lower.contains('unreachable') ||
      lower.contains('os error') ||
      lower.contains('dns error') ||
      lower.contains('network is unreachable')) {
    return MaProblem.unreachable;
  }
  if (lower.contains('stream') &&
      (lower.contains('reach') ||
          lower.contains('fetch') ||
          lower.contains('external'))) {
    return MaProblem.streamHint;
  }
  if (lower.contains('not configured') || lower.contains('is required')) {
    return MaProblem.notConfigured;
  }
  if (lower.contains('invalid or expired token') ||
      lower.contains('authentication') ||
      lower.contains('unauthorized') ||
      (lower.contains('token') &&
          (lower.contains('invalid') || lower.contains('expired')))) {
    return MaProblem.authFailed;
  }
  return MaProblem.other;
}

void _pushUnique(List<MaProblem> problems, MaProblem problem) {
  if (!problems.contains(problem)) problems.add(problem);
}

/// [probe] is a successful HTTP body. Pass [errorText] when the call failed.
TestExplanation explainTest({
  required String externalBase,
  MaTestReport? probe,
  String? errorText,
}) {
  final problems = <MaProblem>[];
  final details = <String>[];
  if (externalBaseIsRejected(externalBase)) {
    _pushUnique(problems, MaProblem.localhostRejected);
    _pushUnique(problems, MaProblem.streamHint);
  }
  if (probe != null) {
    final warning = probe.warning;
    if (warning != null && warning.isNotEmpty) details.add(warning);
    if (probe.detail.isNotEmpty) details.add(probe.detail);
    if (!probe.ok) _pushUnique(problems, classifyMaText(probe.detail));
    final blocking = problems.any((p) => p != MaProblem.streamHint);
    return TestExplanation(
      ok: probe.ok && !blocking,
      problems: problems,
      showStreamHint: true,
      detail: details.join(' — '),
    );
  }
  final message = messageFromErrorBody(errorText ?? '');
  final problem = classifyMaText(message);
  _pushUnique(problems, problem);
  if (problem == MaProblem.localhostRejected || problem == MaProblem.streamHint) {
    _pushUnique(problems, MaProblem.streamHint);
  }
  if (message.isNotEmpty) details.add(message);
  return TestExplanation(
    ok: false,
    problems: problems,
    showStreamHint: problems.contains(MaProblem.streamHint) ||
        problems.contains(MaProblem.localhostRejected),
    detail: details.join(' — '),
  );
}

enum GroupPlanError { needTwo, multipleGroups, unchanged }

class GroupPlan {
  final String action;
  final String playerId;
  final Map<String, dynamic> args;

  const GroupPlan({
    required this.action,
    required this.playerId,
    required this.args,
  });

  @override
  bool operator ==(Object other) =>
      other is GroupPlan &&
      other.action == action &&
      other.playerId == playerId &&
      _mapEquals(other.args, args);

  @override
  int get hashCode => Object.hash(action, playerId, args.length);
}

bool _mapEquals(Map<String, dynamic> a, Map<String, dynamic> b) {
  if (a.length != b.length) return false;
  for (final key in a.keys) {
    final left = a[key];
    final right = b[key];
    if (left is List && right is List) {
      if (left.length != right.length) return false;
      for (var i = 0; i < left.length; i++) {
        if (left[i] != right[i]) return false;
      }
    } else if (left != right) {
      return false;
    }
  }
  return true;
}

class GroupPlanResult {
  final GroupPlan? plan;
  final GroupPlanError? error;
  const GroupPlanResult.ok(this.plan) : error = null;
  const GroupPlanResult.err(this.error) : plan = null;
}

GroupPlanResult planGroup(List<MaPlayer> players, List<String> selected) {
  if (selected.length < 2) {
    return const GroupPlanResult.err(GroupPlanError.needTwo);
  }
  final groups = players
      .where((p) => selected.contains(p.playerId) && p.isGroup)
      .toList();
  if (groups.length > 1) {
    return const GroupPlanResult.err(GroupPlanError.multipleGroups);
  }
  if (groups.length == 1) {
    final group = groups.first;
    final desired = selected.where((id) => id != group.playerId).toList();
    final add = desired.where((id) => !group.groupMembers.contains(id)).toList();
    final remove =
        group.groupMembers.where((id) => !desired.contains(id)).toList();
    if (add.isEmpty && remove.isEmpty) {
      return const GroupPlanResult.err(GroupPlanError.unchanged);
    }
    return GroupPlanResult.ok(GroupPlan(
      action: 'set_members',
      playerId: group.playerId,
      args: {
        'player_ids_to_add': add,
        'player_ids_to_remove': remove,
      },
    ));
  }
  return GroupPlanResult.ok(GroupPlan(
    action: 'group_many',
    playerId: selected.first,
    args: {
      'player_ids': selected,
      'target_player': selected.first,
    },
  ));
}

class PlaybackCandidate {
  final String deviceId;
  final String deviceType;
  final int episodeId;
  final bool playing;
  final int updatedAt;

  const PlaybackCandidate({
    required this.deviceId,
    required this.deviceType,
    required this.episodeId,
    required this.playing,
    required this.updatedAt,
  });
}

bool isMusicAssistant(String deviceType) =>
    deviceType.toLowerCase() == 'music_assistant';

PlaybackCandidate? choosePlayback(
  List<PlaybackCandidate> devices,
  String? focusDeviceId,
) {
  final matched = devices
      .where((d) => isMusicAssistant(d.deviceType) && d.episodeId != 0)
      .toList();
  if (matched.isEmpty) return null;
  if (focusDeviceId != null) {
    for (final device in matched) {
      if (device.deviceId == focusDeviceId) return device;
    }
  }
  for (final device in matched) {
    if (device.playing) return device;
  }
  matched.sort((a, b) => a.updatedAt.compareTo(b.updatedAt));
  return matched.last;
}

bool showMaNowPlaying({
  required bool enabled,
  required bool localPlaying,
  required List<PlaybackCandidate> devices,
  String? focusDeviceId,
}) {
  return enabled &&
      !localPlaying &&
      choosePlayback(devices, focusDeviceId) != null;
}

double displayedPosition({
  required double positionSec,
  required bool playing,
  required double speed,
  required int updatedAt,
  required int now,
}) {
  final base = !positionSec.isFinite || positionSec < 0 ? 0.0 : positionSec;
  if (!playing || updatedAt <= 0) return base;
  final elapsed = (now - updatedAt).toDouble();
  if (elapsed < 0 || elapsed > 30) return base;
  final rate = speed.isFinite && speed > 0 ? speed : 1.0;
  return base + elapsed * rate;
}

String formatClock(double secs) {
  if (secs.isNaN || secs.isInfinite || secs < 0) return '00:00';
  final total = secs.floor();
  final h = total ~/ 3600;
  final m = (total % 3600) ~/ 60;
  final s = total % 60;
  final mm = m.toString().padLeft(2, '0');
  final ss = s.toString().padLeft(2, '0');
  return h > 0 ? '$h:$mm:$ss' : '$mm:$ss';
}

String playerName(List<MaPlayer> players, String playerId) {
  for (final player in players) {
    if (player.playerId == playerId && player.name.isNotEmpty) return player.name;
  }
  return playerId;
}

String? stateLabel(String state) {
  final lower = state.toLowerCase();
  if (lower == 'playing' || lower == 'buffering') return 'Playing';
  if (lower == 'paused') return 'Paused';
  if (lower.isEmpty || lower == 'idle' || lower == 'off' || lower == 'standby') {
    return 'Idle';
  }
  return null;
}
