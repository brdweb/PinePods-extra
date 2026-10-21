import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/podcasting2.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';
import 'package:pinepods_mobile/ui/pinepods/podcasting2_section.dart';

void main() {
  testWidgets('shows value recipients and a resolved podroll the user can open or subscribe', (tester) async {
    ResolvedPodroll? opened;
    ResolvedPodroll? subscribed;

    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Podcasting2Section(
            valueBlocks: const [
              ValueBlock(
                type: 'lightning',
                method: 'keysend',
                suggested: '5000',
                recipients: [
                  ValueRecipient(name: 'Alice', type: 'node', address: '03ab', split: '90'),
                  ValueRecipient(name: 'App', type: 'node', address: '02cd', split: '10'),
                ],
              ),
            ],
            podroll: const [PodrollItem(feedGuid: 'g-1', feedUrl: 'https://example.com/rss')],
            lookup: (_) async => const IndexHit(
              guid: 'g-1',
              title: 'Recommended Show',
              url: 'https://example.com/rss',
              originalUrl: 'https://example.com/rss',
              artwork: '',
              author: 'Ada',
              description: '',
              link: 'https://example.com',
              id: 8,
              episodeCount: 12,
              explicit: false,
              categories: {},
            ),
            onOpen: (show) => opened = show,
            onSubscribe: (show) async {
              subscribed = show;
              return true;
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Value for Value'), findsOneWidget);
    expect(find.text('This show supports Value4Value'), findsOneWidget);
    expect(find.text('Alice'), findsOneWidget);
    expect(find.text('90%'), findsOneWidget);
    expect(find.text('10%'), findsOneWidget);
    expect(find.text('Display only. PinePods does not send payments.'), findsOneWidget);
    expect(find.text('Recommended Show'), findsOneWidget);
    expect(find.text('Ada'), findsOneWidget);

    await tester.tap(find.text('Open'));
    await tester.pump();
    expect(opened?.title, 'Recommended Show');
    expect(opened?.feedUrl, 'https://example.com/rss');

    await tester.tap(find.text('Subscribe'));
    await tester.pumpAndSettle();
    expect(subscribed?.indexId, 8);
    expect(find.text('Subscribed'), findsOneWidget);
  });

  testWidgets('keeps an unresolved guid visible without an open or subscribe action', (tester) async {
    var opened = false;
    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Podcasting2Section(
            valueBlocks: const [],
            podroll: const [PodrollItem(feedGuid: '1234567890abcdef')],
            lookup: (_) async => null,
            onOpen: (_) => opened = true,
            onSubscribe: (_) async => true,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('12345678…'), findsOneWidget);
    expect(find.text("Couldn't look this show up"), findsOneWidget);
    await tester.tap(find.text('Open'));
    await tester.pump();
    expect(opened, isFalse);
  });
}
