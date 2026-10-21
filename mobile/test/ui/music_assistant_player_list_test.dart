import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';
import 'package:pinepods_mobile/ui/podcast/music_assistant_sheet.dart';

MaPlayer speaker(String id, String name, {bool available = true, List<String> members = const []}) {
  return MaPlayer(
    playerId: id,
    deviceId: 'ma:1:$id',
    name: name,
    kind: members.isEmpty ? 'player' : 'group',
    available: available,
    state: 'idle',
    volumeLevel: 30,
    groupMembers: members,
    canGroupWith: const ['den'],
  );
}

void main() {
  testWidgets('groups selected speakers and plays an available one', (tester) async {
    String? played;
    final commands = <(String, String, Map<String, dynamic>)>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: MusicAssistantPlayerList(
              players: [
                speaker('kitchen', 'Kitchen'),
                speaker('living', 'Living Room'),
                speaker('den', 'Den', available: false),
              ],
              connected: true,
              playLabel: 'Play this episode',
              onPlay: (player) async {
                played = player.playerId;
              },
              onCommand: (playerId, action, args) async {
                commands.add((playerId, action, args));
              },
            ),
          ),
        ),
      ),
    );

    expect(find.text('Kitchen'), findsOneWidget);
    expect(find.text('Speaker · Idle'), findsWidgets);
    expect(find.textContaining('Unavailable'), findsOneWidget);

    await tester.tap(find.widgetWithText(CheckboxListTile, 'Kitchen'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Living Room'));
    await tester.pump();
    await tester.tap(find.text('Group selected'));
    await tester.pump();

    expect(commands, hasLength(1));
    expect(commands.single.$1, 'kitchen');
    expect(commands.single.$2, 'group_many');
    expect(commands.single.$3['player_ids'], ['kitchen', 'living']);

    await tester.tap(find.widgetWithText(TextButton, 'Play this episode').first);
    await tester.pump();
    expect(played, 'kitchen');
  });

  testWidgets('edits an existing group and refuses two groups', (tester) async {
    final commands = <String>[];
    Map<String, dynamic>? lastArgs;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: MusicAssistantPlayerList(
              players: [
                speaker('kitchen', 'Kitchen', members: const ['living']),
                speaker('living', 'Living Room'),
                speaker('porch', 'Porch'),
                speaker('den', 'Den', members: const ['hall']),
              ],
              connected: false,
              playLabel: 'Play current episode',
              onPlay: (_) async {},
              onCommand: (playerId, action, args) async {
                commands.add('$playerId:$action');
                lastArgs = args;
              },
            ),
          ),
        ),
      ),
    );

    expect(find.text('Not connected yet'), findsOneWidget);
    expect(find.text('Ungroup'), findsNWidgets(2));

    await tester.ensureVisible(find.widgetWithText(CheckboxListTile, 'Kitchen'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Kitchen'));
    await tester.ensureVisible(find.widgetWithText(CheckboxListTile, 'Den'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Den'));
    await tester.pump();
    await tester.ensureVisible(find.text('Group selected'));
    await tester.tap(find.text('Group selected'));
    await tester.pump();
    expect(find.text('Select only one existing group when changing members.'), findsOneWidget);
    expect(commands, isEmpty);

    await tester.ensureVisible(find.widgetWithText(CheckboxListTile, 'Den'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Den'));
    await tester.ensureVisible(find.widgetWithText(CheckboxListTile, 'Living Room'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Living Room'));
    await tester.ensureVisible(find.widgetWithText(CheckboxListTile, 'Porch'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Porch'));
    await tester.pump();
    await tester.ensureVisible(find.text('Group selected'));
    await tester.tap(find.text('Group selected'));
    await tester.pump();
    expect(commands, ['kitchen:set_members']);
    expect(lastArgs?['player_ids_to_add'], ['porch']);

    await tester.ensureVisible(find.text('Ungroup').first);
    await tester.tap(find.text('Ungroup').first);
    await tester.pump();
    expect(commands, contains('kitchen:ungroup'));
  });
}
