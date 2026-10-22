import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';

MaPlayer player(String id, {String kind = 'player', List<String> members = const []}) {
  return MaPlayer(
    playerId: id,
    deviceId: 'ma:1:$id',
    name: id,
    kind: kind,
    available: true,
    volumeLevel: 20,
    groupMembers: members,
  );
}

PlaybackCandidate candidate(String id, int episode, bool playing, int updated) {
  return PlaybackCandidate(
    deviceId: id,
    deviceType: 'music_assistant',
    episodeId: episode,
    playing: playing,
    updatedAt: updated,
  );
}

void main() {
  test('error bodies use message and plain text stays', () {
    const body =
        '{"error":"Bad request","message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost","status_code":400}';
    expect(messageFromErrorBody(body), contains('not localhost'));
    expect(messageFromErrorBody('timed out connecting'), 'timed out connecting');
  });

  test('localhost external urls are rejected', () {
    expect(externalBaseIsRejected('http://localhost:8040'), isTrue);
    expect(externalBaseIsRejected('http://127.0.0.1:8040'), isTrue);
    expect(externalBaseIsRejected('http://[::1]:8040'), isTrue);
    expect(externalBaseIsRejected('http://app.localhost'), isTrue);
    expect(externalBaseIsRejected('http://169.254.1.1'), isTrue);
    expect(externalBaseIsRejected('http://[fe80::1]/'), isTrue);
    expect(externalBaseIsRejected('http://192.168.1.10:8040'), isFalse);
    expect(externalBaseIsRejected('https://pods.example.com'), isFalse);
    expect(externalBaseIsRejected(''), isFalse);
  });

  test('known failures classify', () {
    expect(
      classifyMaText(
        'external base URL must be a LAN or public host Music Assistant can reach, not localhost',
      ),
      MaProblem.localhostRejected,
    );
    expect(classifyMaText('could not connect: connection refused'), MaProblem.unreachable);
    expect(classifyMaText('timed out connecting to Music Assistant'), MaProblem.unreachable);
    expect(classifyMaText('Music Assistant cannot fetch the stream URL'), MaProblem.streamHint);
    expect(
      classifyMaText('a Music Assistant token is required to enable the connection'),
      MaProblem.notConfigured,
    );
    expect(classifyMaText('Invalid or expired token'), MaProblem.authFailed);
  });

  test('explainTest surfaces localhost and unreachable together', () {
    final explained = explainTest(
      externalBase: 'http://localhost:8040',
      probe: const MaTestReport(
        ok: false,
        detail: 'could not connect: connection refused',
      ),
    );
    expect(explained.ok, isFalse);
    expect(explained.problems, contains(MaProblem.localhostRejected));
    expect(explained.problems, contains(MaProblem.unreachable));
    expect(explained.showStreamHint, isTrue);

    final serverErr = explainTest(
      externalBase: 'http://192.168.1.10:8040',
      errorText:
          '{"message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost"}',
    );
    expect(serverErr.problems, contains(MaProblem.localhostRejected));
    expect(serverErr.showStreamHint, isTrue);
  });

  test('a successful probe still shows the stream hint', () {
    final explained = explainTest(
      externalBase: 'http://192.168.1.10:8040',
      probe: const MaTestReport(
        ok: true,
        detail: 'connected, 2 player(s)',
        serverVersion: '2.10.3',
        schemaVersion: 80,
        playerCount: 2,
      ),
    );
    expect(explained.ok, isTrue);
    expect(explained.problems, isEmpty);
    expect(explained.showStreamHint, isTrue);
    expect(explained.detail, contains('2 player'));
  });

  test('planGroup creates a group and edits an existing one', () {
    final players = [
      player('kitchen'),
      player('living'),
      player('den'),
    ];
    final created = planGroup(players, ['kitchen', 'living']);
    expect(created.error, isNull);
    expect(created.plan!.action, 'group_many');
    expect(created.plan!.playerId, 'kitchen');
    expect(created.plan!.args['target_player'], 'kitchen');
    expect(created.plan!.args['player_ids'], ['kitchen', 'living']);

    final grouped = [
      player('kitchen', members: ['living']),
      player('living'),
      player('den'),
    ];
    final edited = planGroup(grouped, ['kitchen', 'living', 'den']);
    expect(edited.plan!.action, 'set_members');
    expect(edited.plan!.playerId, 'kitchen');
    expect(edited.plan!.args['player_ids_to_add'], ['den']);
    expect(edited.plan!.args['player_ids_to_remove'], isEmpty);
    expect(planGroup(grouped, ['kitchen', 'living']).error, GroupPlanError.unchanged);
    expect(planGroup(players, ['kitchen']).error, GroupPlanError.needTwo);

    final twoGroups = [
      player('kitchen', members: ['living']),
      player('den', kind: 'group', members: ['porch']),
    ];
    expect(
      planGroup(twoGroups, ['kitchen', 'den']).error,
      GroupPlanError.multipleGroups,
    );
  });

  test('choosePlayback prefers focus, then a playing speaker', () {
    final devices = [
      candidate('ma:1:kitchen', 4, false, 10),
      candidate('ma:1:living', 9, true, 5),
      const PlaybackCandidate(
        deviceId: 'web',
        deviceType: 'web',
        episodeId: 3,
        playing: true,
        updatedAt: 99,
      ),
      candidate('ma:1:idle', 0, false, 100),
    ];
    expect(choosePlayback(devices, 'ma:1:kitchen')!.deviceId, 'ma:1:kitchen');
    expect(choosePlayback(devices, null)!.deviceId, 'ma:1:living');
    expect(
      choosePlayback([
        candidate('ma:1:kitchen', 4, false, 10),
        candidate('ma:1:den', 8, false, 20),
      ], null)!
          .deviceId,
      'ma:1:den',
    );
    expect(choosePlayback(const [], null), isNull);
    expect(
      showMaNowPlaying(enabled: true, localPlaying: false, devices: devices),
      isTrue,
    );
    expect(
      showMaNowPlaying(enabled: true, localPlaying: true, devices: devices),
      isFalse,
    );
    expect(
      showMaNowPlaying(enabled: false, localPlaying: false, devices: devices),
      isFalse,
    );
  });

  test('transport stays when sonos reports idle', () {
    expect(transportShown('playing', true, false), isTrue);
    expect(transportShown('paused', true, false), isFalse);
    expect(transportShown('idle', true, false), isFalse);
    expect(transportShown('idle', true, true), isTrue);
    expect(transportShown('', true, false), isFalse);
    expect(transportShown('idle', false, false), isNull);
    expect(transportShown('unknown', false, true), isTrue);
    expect(includeInOtherDevices('music_assistant'), isFalse);
    expect(includeInOtherDevices('Music_Assistant'), isFalse);
    expect(includeInOtherDevices('Web'), isTrue);
    expect(includeInOtherDevices('iOS'), isTrue);
    expect(includeInOtherDevices('Android'), isTrue);
    expect(includeInOtherDevices('Firewood'), isTrue);

    final member = MaPlayer(
      playerId: 'bed',
      deviceId: 'ma:1:bed',
      name: 'Bed',
      kind: 'player',
      state: 'idle',
      syncedTo: 'living',
    );
    final leader = MaPlayer(
      playerId: 'living',
      deviceId: 'ma:1:living',
      name: 'Living',
      kind: 'player',
      state: 'playing',
    );
    expect(playbackStateFor(member, [member, leader]), 'playing');
    expect(playbackStateFor(leader, [member, leader]), 'playing');
    expect(transportPlayerId(member), 'living');
    expect(transportPlayerId(leader), 'living');
  });

  test('displayed position follows speed and freezes when stale', () {
    expect(
      displayedPosition(positionSec: 10, playing: true, speed: 2, updatedAt: 100, now: 105),
      20,
    );
    expect(
      displayedPosition(positionSec: 10, playing: false, speed: 2, updatedAt: 100, now: 105),
      10,
    );
    expect(
      displayedPosition(positionSec: 10, playing: true, speed: 1, updatedAt: 100, now: 140),
      10,
    );
    expect(formatClock(65), '01:05');
    expect(formatClock(3661), '1:01:01');
  });

  test('player json parses groups', () {
    final parsed = MaPlayersResponse.fromJson({
      'enabled': true,
      'connected': true,
      'players': [
        {
          'player_id': 'kitchen',
          'device_id': 'ma:1:kitchen',
          'name': 'Kitchen',
          'kind': 'group',
          'available': true,
          'state': 'playing',
          'volume_level': 20,
          'volume_muted': false,
          'group_members': ['living'],
          'can_group_with': ['den'],
          'queue_id': 'kitchen',
        }
      ],
    });
    expect(parsed.players.single.isGroup, isTrue);
    expect(parsed.players.single.groupMembers, ['living']);
    expect(playerName(parsed.players, 'kitchen'), 'Kitchen');
    expect(stateLabel('buffering'), 'Playing');
  });
}
