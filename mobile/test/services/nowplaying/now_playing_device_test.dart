// A device's `capabilities` decide which remote controls the UI offers it. An
// empty list means a client from before the parameter existed, which applies the
// original action set.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';

void main() {
  test('a device that lists its actions supports only those', () {
    final d = NowPlayingDevice.fromJson({
      'device_id': 'a',
      'capabilities': ['play', 'pause', 'play_episode'],
    });
    expect(d.supports('pause'), isTrue);
    expect(d.supports('play_episode'), isTrue);
    expect(d.supports('skip_forward'), isFalse);
  });

  test('a device without the field supports the original actions', () {
    final d = NowPlayingDevice.fromJson({'device_id': 'a'});
    for (final action in NowPlayingDevice.legacyActions) {
      expect(d.supports(action), isTrue, reason: action);
    }
    expect(d.supports('set_volume'), isFalse);
  });

  test('music assistant volume and group fields parse', () {
    final d = NowPlayingDevice.fromJson({
      'device_id': 'ma:1:kitchen',
      'device_type': 'music_assistant',
      'volume_level': 20,
      'volume_muted': false,
      'ma_player_id': 'kitchen',
      'group_members': ['living'],
      'capabilities': ['play', 'pause', 'volume_set', 'group_volume'],
    });
    expect(d.volumeLevel, 20);
    expect(d.volumeMuted, isFalse);
    expect(d.maPlayerId, 'kitchen');
    expect(d.groupMembers, ['living']);
    expect(d.supports('volume_set'), isTrue);
    expect(d.supports('seek'), isFalse);
  });

  test('non-string entries are ignored', () {
    final d = NowPlayingDevice.fromJson({
      'device_id': 'a',
      'capabilities': ['seek', 3, null],
    });
    expect(d.capabilities, ['seek']);
  });
}
