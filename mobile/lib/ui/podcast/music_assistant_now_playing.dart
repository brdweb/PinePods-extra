import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/music_assistant/music_assistant_api.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';
import 'package:pinepods_mobile/ui/podcast/music_assistant_sheet.dart';

/// Mini now-playing bar for a Music Assistant player. Renders nothing unless
/// the bridge is enabled and a player has an episode while this phone is idle.
class MaNowPlayingBanner extends StatefulWidget {
  const MaNowPlayingBanner({super.key});

  @override
  State<MaNowPlayingBanner> createState() => _MaNowPlayingBannerState();
}

class _MaNowPlayingBannerState extends State<MaNowPlayingBanner> {
  Timer? _tick;

  @override
  void dispose() {
    _tick?.cancel();
    super.dispose();
  }

  NowPlayingDevice? _device(List<NowPlayingDevice> devices, bool enabled) {
    if (!enabled) return null;
    final candidates = [
      for (final device in devices)
        PlaybackCandidate(
          deviceId: device.deviceId,
          deviceType: device.deviceType,
          episodeId: device.episodeId,
          playing: device.playing,
          updatedAt: device.updatedAt,
        ),
    ];
    final chosen = choosePlayback(
      candidates,
      MusicAssistantController.instance.focusDeviceId.value,
    );
    if (chosen == null) return null;
    for (final device in devices) {
      if (device.deviceId == chosen.deviceId) return device;
    }
    return null;
  }

  void _syncTick(bool playing) {
    if (playing && _tick == null) {
      _tick = Timer.periodic(const Duration(seconds: 1), (_) {
        if (mounted) setState(() {});
      });
    } else if (!playing) {
      _tick?.cancel();
      _tick = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final service = GlobalServices.nowPlayingService;
    if (service == null) return const SizedBox.shrink();
    return ValueListenableBuilder<List<NowPlayingDevice>>(
      valueListenable: service.devices,
      builder: (context, devices, _) {
        return ValueListenableBuilder<bool>(
          valueListenable: MusicAssistantController.instance.enabled,
          builder: (context, enabled, _) {
            final device = _device(devices, enabled);
            _syncTick(device?.playing ?? false);
            if (device == null) return const SizedBox.shrink();
            final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
            final shown = displayedPosition(
              positionSec: device.positionSec,
              playing: device.playing,
              speed: device.speed,
              updatedAt: device.updatedAt,
              now: now,
            );
            final title = device.title.isEmpty ? 'Now playing' : device.title;
            final progress = device.durationSec > 0
                ? '${formatClock(shown)} / ${formatClock(device.durationSec)}'
                : formatClock(shown);
            return Material(
              color: Theme.of(context).colorScheme.surface,
              child: InkWell(
                onTap: () => _openControls(context, service, device),
                child: Container(
                  height: 66,
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  decoration: BoxDecoration(
                    border: Border(top: Divider.createBorderSide(context)),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.speaker),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              'Playing on ${device.deviceName}',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(fontSize: 11),
                            ),
                            Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
                            Text(progress, style: const TextStyle(fontSize: 11)),
                          ],
                        ),
                      ),
                      IconButton(
                        tooltip: device.playing ? 'Pause' : 'Play',
                        icon: Icon(device.playing ? Icons.pause : Icons.play_arrow),
                        onPressed: () => service.sendCommand(
                          device.deviceId,
                          device.playing ? 'pause' : 'play',
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            );
          },
        );
      },
    );
  }

  Future<void> _openControls(
    BuildContext context,
    NowPlayingService service,
    NowPlayingDevice device,
  ) async {
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (context) => _MaTransportSheet(service: service, deviceId: device.deviceId),
    );
  }
}

class _MaTransportSheet extends StatelessWidget {
  const _MaTransportSheet({required this.service, required this.deviceId});

  final NowPlayingService service;
  final String deviceId;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ValueListenableBuilder<List<NowPlayingDevice>>(
        valueListenable: service.devices,
        builder: (context, devices, _) {
          NowPlayingDevice? device;
          for (final candidate in devices) {
            if (candidate.deviceId == deviceId) device = candidate;
          }
          if (device == null) {
            return const Padding(
              padding: EdgeInsets.all(24),
              child: Text('That speaker is offline.'),
            );
          }
          final current = device;
          final members = current.groupMembers;
          final grouped = members.isNotEmpty;
          final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
          final shown = displayedPosition(
            positionSec: current.positionSec,
            playing: current.playing,
            speed: current.speed,
            updatedAt: current.updatedAt,
            now: now,
          );
          return SingleChildScrollView(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    current.title.isEmpty ? 'Now playing' : current.title,
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  Text('Playing on ${current.deviceName}'),
                  Text('${formatClock(shown)} / ${formatClock(current.durationSec)}'),
                  if (current.durationSec > 0 && current.supports('seek'))
                    Slider(
                      value: shown.clamp(0, current.durationSec).toDouble(),
                      min: 0,
                      max: current.durationSec,
                      label: 'Seek',
                      onChanged: (_) {},
                      onChangeEnd: (value) => service.sendCommand(
                        current.deviceId,
                        'seek',
                        {'position_sec': value},
                      ),
                    ),
                  Row(
                    children: [
                      if (current.supports('skip_back'))
                        IconButton(
                          tooltip: 'Skip back 15s',
                          icon: const Icon(Icons.replay),
                          onPressed: () => service.sendCommand(
                            current.deviceId,
                            'skip_back',
                            const {'seconds': 15},
                          ),
                        ),
                      IconButton(
                        tooltip: current.playing ? 'Pause' : 'Play',
                        icon: Icon(current.playing ? Icons.pause : Icons.play_arrow),
                        onPressed: () => service.sendCommand(
                          current.deviceId,
                          current.playing ? 'pause' : 'play',
                        ),
                      ),
                      if (current.supports('skip_forward'))
                        IconButton(
                          tooltip: 'Skip forward 30s',
                          icon: const Icon(Icons.forward_30),
                          onPressed: () => service.sendCommand(
                            current.deviceId,
                            'skip_forward',
                            const {'seconds': 30},
                          ),
                        ),
                      if (current.supports('stop'))
                        IconButton(
                          tooltip: 'Stop',
                          icon: const Icon(Icons.stop),
                          onPressed: () => service.sendCommand(current.deviceId, 'stop'),
                        ),
                      if (current.supports('set_speed'))
                        DropdownButton<double>(
                          value: _nearestSpeed(current.speed),
                          items: const [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0]
                              .map((rate) => DropdownMenuItem(
                                    value: rate,
                                    child: Text('${rate.toStringAsFixed(2)}x'),
                                  ))
                              .toList(),
                          onChanged: (rate) {
                            if (rate == null) return;
                            service.sendCommand(current.deviceId, 'set_speed', {'speed': rate});
                          },
                        ),
                    ],
                  ),
                  if (current.volumeLevel != null &&
                      current.supports(grouped ? 'group_volume' : 'volume_set'))
                    _relayVolume(
                      context,
                      label: grouped ? 'Group volume' : 'Volume',
                      level: current.volumeLevel!,
                      muted: current.volumeMuted ?? false,
                      onLevel: (level) => service.sendCommand(
                        current.deviceId,
                        grouped ? 'group_volume' : 'volume_set',
                        {'volume_level': level},
                      ),
                      onMute: () => service.sendCommand(
                        current.deviceId,
                        'volume_mute',
                        {'muted': !(current.volumeMuted ?? false)},
                      ),
                    ),
                  if (grouped)
                    ValueListenableBuilder<List<MaPlayer>>(
                      valueListenable: MusicAssistantController.instance.players,
                      builder: (context, players, _) => Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text('Speakers in this group'),
                          for (final member in members)
                            if (players.where((p) => p.playerId == member && p.available).isNotEmpty)
                              _relayVolume(
                                context,
                                label: playerName(players, member),
                                level: players.firstWhere((p) => p.playerId == member).volumeLevel ?? 0,
                                muted: players.firstWhere((p) => p.playerId == member).volumeMuted,
                                onLevel: (level) {
                                  final player = players.firstWhere((p) => p.playerId == member);
                                  service.sendCommand(player.deviceId, 'volume_set', {'volume_level': level});
                                },
                                onMute: () {
                                  final player = players.firstWhere((p) => p.playerId == member);
                                  service.sendCommand(
                                    player.deviceId,
                                    'volume_mute',
                                    {'muted': !player.volumeMuted},
                                  );
                                },
                              ),
                        ],
                      ),
                    ),
                  Align(
                    alignment: Alignment.centerRight,
                    child: TextButton.icon(
                      onPressed: () {
                        final host = Navigator.of(context, rootNavigator: true).context;
                        Navigator.of(context).pop();
                        showMusicAssistantSpeakers(host);
                      },
                      icon: const Icon(Icons.speaker_group),
                      label: const Text('MA Speakers'),
                    ),
                  ),
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}

double _nearestSpeed(double speed) {
  const rates = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
  var best = rates.first;
  for (final rate in rates) {
    if ((rate - speed).abs() < (best - speed).abs()) best = rate;
  }
  return best;
}

Widget _relayVolume(
  BuildContext context, {
  required String label,
  required double level,
  required bool muted,
  required ValueChanged<double> onLevel,
  required VoidCallback onMute,
}) {
  return Row(
    children: [
      SizedBox(width: 96, child: Text(label, style: const TextStyle(fontSize: 12))),
      Expanded(
        child: Slider(
          value: level.clamp(0, 100).toDouble(),
          min: 0,
          max: 100,
          label: label,
          onChanged: (_) {},
          onChangeEnd: onLevel,
        ),
      ),
      IconButton(
        tooltip: muted ? 'Unmute' : 'Mute',
        onPressed: onMute,
        icon: Icon(muted ? Icons.volume_off : Icons.volume_up),
      ),
    ],
  );
}
