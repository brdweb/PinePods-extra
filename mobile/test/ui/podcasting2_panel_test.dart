import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/podcasting2_tags.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';
import 'package:pinepods_mobile/ui/widgets/podcasting2_panel.dart';

Widget _host(Widget child) {
  return MaterialApp(
    localizationsDelegates: const [
      AppLocalizations.delegate,
      GlobalMaterialLocalizations.delegate,
      GlobalWidgetsLocalizations.delegate,
      GlobalCupertinoLocalizations.delegate,
    ],
    supportedLocales: AppLocalizations.supportedLocales,
    home: Scaffold(body: SingleChildScrollView(child: child)),
  );
}

void main() {
  testWidgets('episode panel lists clips and hides a disabled comment thread', (
    tester,
  ) async {
    ClipWindow? played;
    await tester.pumpWidget(
      _host(
        EpisodePodcasting2Panel(
          tags: const ItemTags(
            soundbites: [
              Soundbite(startTime: '73', duration: '60', title: 'Hook'),
            ],
            locations: [
              P2Location(name: 'Studio', geo: 'geo:40.7,-74.0', country: 'US'),
            ],
            socialInteracts: [
              SocialInteract(
                protocol: 'disabled',
                uri: 'https://example.com/off',
              ),
              SocialInteract(
                protocol: 'activitypub',
                uri: 'https://example.com/thread',
              ),
            ],
          ),
          onPlayClip: (window) => played = window,
        ),
      ),
    );

    expect(find.text('Soundbites'), findsOneWidget);
    expect(find.text('Open map'), findsOneWidget);
    expect(find.text('activitypub'), findsOneWidget);
    expect(find.text('disabled'), findsNothing);
    expect(find.text('https://example.com/off'), findsNothing);

    await tester.tap(find.byTooltip('Play clip'));
    expect(played?.start, 73);
    expect(played?.duration, 60);
  });

  testWidgets('show panel plays a trailer and empty tags render nothing', (
    tester,
  ) async {
    Trailer? played;
    await tester.pumpWidget(
      _host(
        ShowPodcasting2Panel(
          tags: const ChannelTags(
            trailers: [
              Trailer(title: 'Teaser', url: 'https://example.com/teaser.mp3'),
            ],
          ),
          onPlayTrailer: (trailer) => played = trailer,
        ),
      ),
    );

    expect(find.text('Trailers'), findsWidgets);
    await tester.tap(find.byTooltip('Play trailer'));
    expect(played?.url, 'https://example.com/teaser.mp3');

    await tester.pumpWidget(
      _host(const EpisodePodcasting2Panel(tags: ItemTags())),
    );
    expect(find.text('Soundbites'), findsNothing);
    expect(find.byType(SizedBox), findsWidgets);

    await tester.pumpWidget(
      _host(const ShowPodcasting2Panel(tags: ChannelTags())),
    );
    expect(find.text('Trailers'), findsNothing);
  });
}
