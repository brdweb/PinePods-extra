import 'package:flutter/material.dart';
import 'package:pinepods_mobile/core/alternate_media.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/music_assistant/music_assistant_api.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';
import 'package:provider/provider.dart';

class MaEpisodeTarget {
  final int episodeId;
  final bool isYoutube;
  final double? positionSec;

  const MaEpisodeTarget({
    required this.episodeId,
    required this.isYoutube,
    this.positionSec,
  });
}

/// Speaker list, grouping, volume, and play. [onCommand] performs one MA
/// command; tests pass a fake, the app sends it over HTTP.
class MusicAssistantPlayerList extends StatefulWidget {
  const MusicAssistantPlayerList({
    super.key,
    required this.players,
    required this.connected,
    required this.playLabel,
    required this.onPlay,
    required this.onCommand,
    this.loadedPlayerIds = const {},
    this.playingPlayerIds = const {},
  });

  final List<MaPlayer> players;
  final bool connected;
  final String playLabel;
  final Future<void> Function(MaPlayer player) onPlay;
  final Future<void> Function(String playerId, String action, Map<String, dynamic> args) onCommand;

  /// Player ids whose relay snapshot still has an episode. Idle Sonos rows
  /// keep play/pause/stop while one of these is set.
  final Set<String> loadedPlayerIds;

  /// Player ids the relay says are playing. Overrides a stale idle state.
  final Set<String> playingPlayerIds;

  @override
  State<MusicAssistantPlayerList> createState() => _MusicAssistantPlayerListState();
}

class _MusicAssistantPlayerListState extends State<MusicAssistantPlayerList> {
  final Set<String> _selected = {};
  String? _message;

  void _toggle(String id) {
    setState(() {
      if (!_selected.add(id)) _selected.remove(id);
    });
  }

  void _group() {
    final ids = widget.players
        .where((player) => _selected.contains(player.playerId))
        .map((player) => player.playerId)
        .toList();
    final result = planGroup(widget.players, ids);
    if (result.error != null || result.plan == null) {
      setState(() => _message = switch (result.error) {
            GroupPlanError.needTwo => 'Select at least two speakers to group.',
            GroupPlanError.multipleGroups =>
              'Select only one existing group when changing members.',
            GroupPlanError.unchanged => 'That group already has these speakers.',
            _ => 'Could not group those speakers.',
          });
      return;
    }
    final plan = result.plan!;
    setState(() {
      _selected.clear();
      _message = null;
    });
    widget.onCommand(plan.playerId, plan.action, plan.args);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (!widget.connected)
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: Text('Not connected yet'),
          ),
        if (widget.players.isEmpty)
          const Padding(
            padding: EdgeInsets.all(24),
            child: Text('No speakers yet', textAlign: TextAlign.center),
          ),
        const Padding(
          padding: EdgeInsets.fromLTRB(16, 8, 16, 4),
          child: Text(
            'Select speakers, then group them. Play sends the current episode, or the episode you picked.',
            style: TextStyle(fontSize: 12),
          ),
        ),
        if (_message != null)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text(_message!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
          ),
        for (final player in widget.players) _playerTile(context, player),
        Padding(
          padding: const EdgeInsets.all(16),
          child: FilledButton(
            onPressed: _selected.length < 2 ? null : _group,
            child: const Text('Group selected'),
          ),
        ),
      ],
    );
  }

  Widget _playerTile(BuildContext context, MaPlayer player) {
    final grouped = player.isGroup;
    final playback = playbackStateFor(player, widget.players);
    final state = stateLabel(playback) ?? playback;
    final kind = grouped ? 'Group' : 'Speaker';
    final leaderId = transportPlayerId(player);
    final episodeLoaded = widget.loadedPlayerIds.contains(player.playerId) ||
        widget.loadedPlayerIds.contains(leaderId);
    final relayPlaying = widget.playingPlayerIds.contains(player.playerId) ||
        widget.playingPlayerIds.contains(leaderId);
    final shown = player.available
        ? transportShown(playback, episodeLoaded, relayPlaying)
        : null;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CheckboxListTile(
            value: _selected.contains(player.playerId),
            onChanged: (_) => _toggle(player.playerId),
            secondary: Icon(grouped ? Icons.speaker_group : Icons.speaker),
            title: Text(player.name.isEmpty ? player.playerId : player.name),
            subtitle: Text(
              player.available ? '$kind · $state' : '$kind · $state · Unavailable',
            ),
          ),
          if (player.syncedTo != null)
            Padding(
              padding: const EdgeInsets.only(left: 16),
              child: Text('Synced to ${playerName(widget.players, player.syncedTo!)}'),
            ),
          if (grouped)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Wrap(
                spacing: 6,
                children: [
                  for (final member in player.groupMembers)
                    Chip(
                      label: Text(playerName(widget.players, member)),
                      onDeleted: () => widget.onCommand(player.playerId, 'set_members', {
                        'player_ids_to_remove': [member],
                      }),
                    ),
                ],
              ),
            ),
          if (grouped && player.canGroupWith.isNotEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: DropdownButton<String>(
                hint: const Text('Add speaker'),
                items: [
                  for (final id in player.canGroupWith)
                    if (!player.groupMembers.contains(id))
                      DropdownMenuItem(
                        value: id,
                        child: Text(playerName(widget.players, id)),
                      ),
                ],
                onChanged: (id) {
                  if (id == null) return;
                  widget.onCommand(player.playerId, 'set_members', {
                    'player_ids_to_add': [id],
                  });
                },
              ),
            ),
          if (player.available && player.volumeLevel != null)
            _VolumeRow(
              label: grouped ? 'Group volume' : 'Volume',
              level: player.volumeLevel!,
              muted: player.volumeMuted,
              onChanged: (level) => widget.onCommand(
                player.playerId,
                grouped ? 'group_volume' : 'volume_set',
                {'volume_level': level},
              ),
              onMute: () => widget.onCommand(
                player.playerId,
                'volume_mute',
                {'muted': !player.volumeMuted},
              ),
            ),
          Padding(
            padding: const EdgeInsets.only(left: 8),
            child: Row(
              children: [
                if (shown != null) ...[
                  IconButton(
                    tooltip: shown ? 'Pause' : 'Play',
                    onPressed: () => widget.onCommand(
                      leaderId,
                      shown ? 'pause' : 'play',
                      const {},
                    ),
                    icon: Icon(shown ? Icons.pause : Icons.play_arrow),
                  ),
                  IconButton(
                    tooltip: 'Stop',
                    onPressed: () => widget.onCommand(leaderId, 'stop', const {}),
                    icon: const Icon(Icons.stop),
                  ),
                ],
                TextButton.icon(
                  onPressed: player.available ? () => widget.onPlay(player) : null,
                  icon: const Icon(Icons.play_arrow),
                  label: Text(widget.playLabel),
                ),
                if (grouped)
                  TextButton(
                    onPressed: () => widget.onCommand(player.playerId, 'ungroup', const {}),
                    child: const Text('Ungroup'),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _VolumeRow extends StatefulWidget {
  const _VolumeRow({
    required this.label,
    required this.level,
    required this.muted,
    required this.onChanged,
    required this.onMute,
  });

  final String label;
  final double level;
  final bool muted;
  final ValueChanged<double> onChanged;
  final VoidCallback onMute;

  @override
  State<_VolumeRow> createState() => _VolumeRowState();
}

class _VolumeRowState extends State<_VolumeRow> {
  double? _drag;

  @override
  Widget build(BuildContext context) {
    final shown = _drag ?? widget.level;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          SizedBox(width: 88, child: Text(widget.label, style: const TextStyle(fontSize: 12))),
          Expanded(
            child: Slider(
              value: shown.clamp(0, 100).toDouble(),
              min: 0,
              max: 100,
              label: widget.label,
              onChanged: (value) => setState(() => _drag = value),
              onChangeEnd: (value) {
                setState(() => _drag = null);
                widget.onChanged(value);
              },
            ),
          ),
          IconButton(
            tooltip: widget.muted ? 'Unmute' : 'Mute',
            onPressed: widget.onMute,
            icon: Icon(widget.muted ? Icons.volume_off : Icons.volume_up),
          ),
        ],
      ),
    );
  }
}

Widget _speakerList({
  required BuildContext context,
  required List<MaPlayer> players,
  required bool connected,
  required MaEpisodeTarget? episode,
  required MaEpisodeTarget? target,
  required MusicAssistantApi api,
  required ({String server, String apiKey, int userId}) session,
  required ScaffoldMessengerState? messenger,
  required Set<String> loaded,
  required Set<String> playing,
}) {
  return MusicAssistantPlayerList(
    players: players,
    connected: connected,
    loadedPlayerIds: loaded,
    playingPlayerIds: playing,
    playLabel: episode != null ? 'Play this episode' : 'Play current episode',
    onPlay: (player) async {
      final chosen = target;
      if (chosen == null) {
        messenger?.showSnackBar(
          const SnackBar(content: Text('Nothing is playing here to send.')),
        );
        return;
      }
      final args = <String, dynamic>{
        'episode_id': chosen.episodeId,
        'is_youtube': chosen.isYoutube,
      };
      if (MediaChoiceSession.episodeId == chosen.episodeId &&
          MediaChoiceSession.optionId != null) {
        args['media_option_id'] = MediaChoiceSession.optionId;
      }
      if (chosen.positionSec != null) {
        args['position_sec'] = chosen.positionSec;
      }
      try {
        final result = await api.command(
          userId: session.userId,
          playerId: player.playerId,
          action: 'play_episode',
          args: args,
        );
        MusicAssistantController.instance.focusDeviceId.value = player.deviceId;
        if (context.mounted) {
          final audio = Provider.of<AudioPlayerService>(context, listen: false);
          await audio.pause();
        }
        if (result.warnings.isNotEmpty) {
          messenger?.showSnackBar(SnackBar(content: Text(result.warnings.join(' '))));
        }
        if (context.mounted) Navigator.of(context).pop();
      } catch (err) {
        final explained = explainTest(externalBase: '', errorText: err.toString());
        messenger?.showSnackBar(SnackBar(content: Text(explained.detail)));
      }
    },
    onCommand: (playerId, action, args) async {
      try {
        final result = await api.command(
          userId: session.userId,
          playerId: playerId,
          action: action,
          args: args,
        );
        if (result.warnings.isNotEmpty) {
          messenger?.showSnackBar(SnackBar(content: Text(result.warnings.join(' '))));
        }
        final refreshed = await api.listPlayers(session.userId);
        MusicAssistantController.instance.applyPlayers(refreshed);
      } catch (err) {
        messenger?.showSnackBar(SnackBar(content: Text('$err')));
      }
    },
  );
}

/// Opens the speaker picker. Hidden by callers unless the bridge is enabled.
Future<void> showMusicAssistantSpeakers(
  BuildContext context, {
  MaEpisodeTarget? episode,
}) async {
  final session = maSessionOf(context);
  if (session == null) return;
  final api = MusicAssistantApi(server: session.server, apiKey: session.apiKey);
  MaPlayersResponse list;
  try {
    list = await api.listPlayers(session.userId);
  } catch (err) {
    if (context.mounted) {
      ScaffoldMessenger.maybeOf(context)?.showSnackBar(SnackBar(content: Text('$err')));
    }
    return;
  }
  MusicAssistantController.instance.applyPlayers(list);
  if (!context.mounted || !list.enabled) return;
  final messenger = ScaffoldMessenger.maybeOf(context);
  final target = episode ?? _currentTarget(context);
  await showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).scaffoldBackgroundColor,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
    ),
    builder: (context) {
      return SafeArea(
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
                child: Row(
                  children: [
                    const Icon(Icons.speaker_group),
                    const SizedBox(width: 8),
                    Text('MA Speakers', style: Theme.of(context).textTheme.titleLarge),
                  ],
                ),
              ),
              ValueListenableBuilder<List<MaPlayer>>(
                valueListenable: MusicAssistantController.instance.players,
                builder: (context, players, _) {
                  final service = GlobalServices.nowPlayingService;
                  if (service == null) {
                    return _speakerList(
                      context: context,
                      players: players,
                      connected: list.connected,
                      episode: episode,
                      target: target,
                      api: api,
                      session: session,
                      messenger: messenger,
                      loaded: const {},
                      playing: const {},
                    );
                  }
                  return ValueListenableBuilder<List<NowPlayingDevice>>(
                    valueListenable: service.devices,
                    builder: (context, devices, _) {
                      final loaded = <String>{};
                      final playingIds = <String>{};
                      for (final device in devices) {
                        if (!isMusicAssistant(device.deviceType) || device.episodeId == 0) {
                          continue;
                        }
                        final id = device.maPlayerId;
                        if (id == null || id.isEmpty) continue;
                        loaded.add(id);
                        if (device.playing) playingIds.add(id);
                      }
                      return _speakerList(
                        context: context,
                        players: players,
                        connected: list.connected,
                        episode: episode,
                        target: target,
                        api: api,
                        session: session,
                        messenger: messenger,
                        loaded: loaded,
                        playing: playingIds,
                      );
                    },
                  );
                },
              ),
            ],
          ),
        ),
      );
    },
  );
}

MaEpisodeTarget? _currentTarget(BuildContext context) {
  final service = GlobalServices.nowPlayingService;
  final episodeId = service?.reportedEpisodeId;
  if (episodeId == null) return null;
  return MaEpisodeTarget(
    episodeId: episodeId,
    isYoutube: service!.reportedIsYoutube,
    positionSec: service.reportedPositionSec,
  );
}
