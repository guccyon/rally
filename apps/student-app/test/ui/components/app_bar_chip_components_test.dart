import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/components/rally_app_bar.dart';
import 'package:rally_student_app/ui/components/rally_choice_chips.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

/// アプリバー（app_bar）とチップ（chip）の部品（components.yaml の 0.9.0）。
void main() {
  Future<void> pumpAppBar(WidgetTester tester, RallyAppBar appBar) =>
      tester.pumpWidget(
        MaterialApp(
          theme: buildRallyTheme(),
          home: Scaffold(appBar: appBar, body: const SizedBox()),
        ),
      );

  group('アプリバー', () {
    testWidgets('高さ 56px で、戻るは「戻る」と読み、押すと呼ばれる', (tester) async {
      var back = 0;
      await pumpAppBar(
        tester,
        RallyAppBar(title: '自分の予約', onBack: () => back++),
      );
      expect(tester.getSize(find.byType(RallyAppBar)).height, 56);
      expect(find.bySemanticsLabel('戻る'), findsOneWidget);
      expect(find.byTooltip('戻る'), findsOneWidget);
      await tester.tap(find.byTooltip('戻る'));
      expect(back, 1);
      // 押せる範囲は 48px
      expect(tester.getSize(find.byType(IconButton)), const Size(48, 48));
    });

    testWidgets('top は戻るを出さず、題を見出しとして読む', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpAppBar(tester, const RallyAppBar(title: 'ホーム'));
      expect(find.byTooltip('戻る'), findsNothing);
      expect(
        tester.getSemantics(find.text('ホーム')),
        matchesSemantics(label: 'ホーム', isHeader: true),
      );
      handle.dispose();
    });

    testWidgets('入口は items の語の tooltip と、未読の数つきの読み上げ', (tester) async {
      var opened = <RallyAppBarEntry>[];
      await pumpAppBar(
        tester,
        RallyAppBar(
          title: 'ホーム',
          actions: [
            for (final entry in RallyAppBarEntry.values.skip(1))
              RallyAppBarAction(
                entry,
                onPressed: () => opened = [...opened, entry],
                unreadCount: entry == RallyAppBarEntry.chat ? 3 : 0,
                hasUnread: entry == RallyAppBarEntry.notifications,
              ),
          ],
        ),
      );
      for (final entry in RallyAppBarEntry.values.skip(1)) {
        expect(find.byTooltip(entry.label), findsOneWidget);
      }
      expect(find.bySemanticsLabel('チャット、未読 3 件'), findsOneWidget);
      expect(find.bySemanticsLabel('お知らせ、未読あり'), findsOneWidget);
      expect(find.bySemanticsLabel('自分の予約'), findsOneWidget);
      await tester.tap(find.byTooltip('チャット'));
      expect(opened, [RallyAppBarEntry.chat]);
    });

    testWidgets('送信中は戻ると入口を text_muted にして押せず、端末の戻るも止める', (tester) async {
      var back = 0;
      await pumpAppBar(
        tester,
        RallyAppBar(
          title: 'ログイン',
          onBack: () => back++,
          busy: true,
          actions: [
            RallyAppBarAction(
              RallyAppBarEntry.profile,
              onPressed: () => back++,
            ),
          ],
        ),
      );
      await tester.tap(find.byTooltip('戻る'));
      await tester.tap(find.byTooltip('プロフィール'));
      expect(back, 0);
      for (final button in tester.widgetList<IconButton>(
        find.byType(IconButton),
      )) {
        expect(button.onPressed, isNull);
      }
      expect(
        IconTheme.of(tester.element(find.byIcon(Icons.arrow_back_rounded)))
            .color,
        RallyColors.textMuted,
      );
      expect(tester.widget<PopScope>(find.byType(PopScope)).canPop, isFalse);
      // 300ms を超えるまで（loading でないあいだ）は bar を出さない
      expect(find.byType(LinearProgressIndicator), findsNothing);
    });

    testWidgets('送信中が長いときは下に 4px の loading の bar を出す', (tester) async {
      await pumpAppBar(
        tester,
        RallyAppBar(title: 'ログイン', onBack: () {}, busy: true, loading: true),
      );
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      expect(tester.getSize(find.byType(RallyAppBar)).height, 60);
    });

    testWidgets('送信中でなければ端末の戻るを止めない', (tester) async {
      await pumpAppBar(tester, RallyAppBar(title: '自分の予約', onBack: () {}));
      expect(tester.widget<PopScope>(find.byType(PopScope)).canPop, isTrue);
    });
  });

  group('チップ', () {
    const options = [
      RallyChipOption('tennis', 'テニス'),
      RallyChipOption('baseball', '野球'),
      RallyChipOption('soccer', 'サッカー'),
    ];

    Future<void> pumpChips(WidgetTester tester, Widget chips) =>
        tester.pumpWidget(
          MaterialApp(
            theme: buildRallyTheme(),
            home: Scaffold(body: Center(child: chips)),
          ),
        );

    Material chipMaterial(WidgetTester tester, String label) =>
        tester.widget<Material>(
          find
              .ancestor(of: find.text(label), matching: find.byType(Material))
              .first,
        );

    testWidgets('押すと 1 つを選び、選んだ chip は check と「選択中」の読み上げで示す', (tester) async {
      String? selected = 'tennis';
      await pumpChips(
        tester,
        StatefulBuilder(
          builder: (context, setState) => RallyChoiceChips<String>(
            label: '習いたいスポーツ',
            options: options,
            selected: selected,
            onSelected: (value) => setState(() => selected = value),
          ),
        ),
      );
      expect(find.byIcon(Icons.check_rounded), findsOneWidget);
      expect(find.bySemanticsLabel('テニス、選択中'), findsOneWidget);
      expect(chipMaterial(tester, 'テニス').color, RallyColors.primaryContainer);
      expect(chipMaterial(tester, '野球').color, RallyColors.background);
      // 高さ 48px 以上
      expect(
        tester.getSize(find.byType(InkWell).first).height,
        greaterThanOrEqualTo(48),
      );

      await tester.tap(find.text('野球'));
      await tester.pump();
      expect(selected, 'baseball');
      expect(find.bySemanticsLabel('野球、選択中'), findsOneWidget);
      expect(find.byIcon(Icons.check_rounded), findsOneWidget);

      // 選んだ chip をもう一度押しても外さない
      await tester.tap(find.text('野球'));
      await tester.pump();
      expect(selected, 'baseball');
    });

    testWidgets('chip は radio として読み、group にはラベルがある', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpChips(
        tester,
        RallyChoiceChips<String>(
          label: '習いたいスポーツ',
          options: options,
          selected: 'tennis',
          onSelected: (_) {},
        ),
      );
      expect(
        tester.getSemantics(find.text('テニス')),
        matchesSemantics(
          label: 'テニス、選択中',
          isInMutuallyExclusiveGroup: true,
          hasCheckedState: true,
          isChecked: true,
          isButton: true,
          hasEnabledState: true,
          isEnabled: true,
          hasTapAction: true,
          isFocusable: true,
        ),
      );
      expect(find.bySemanticsLabel('習いたいスポーツ'), findsOneWidget);
      handle.dispose();
    });

    testWidgets('送信中は押せず、全部を surface にして選んだ chip の check は残す', (tester) async {
      var pressed = 0;
      await pumpChips(
        tester,
        RallyChoiceChips<String>(
          label: '習いたいスポーツ',
          options: options,
          selected: 'tennis',
          onSelected: (_) => pressed++,
          disabled: true,
        ),
      );
      await tester.tap(find.text('野球'));
      expect(pressed, 0);
      expect(chipMaterial(tester, 'テニス').color, RallyColors.surface);
      expect(chipMaterial(tester, '野球').color, RallyColors.surface);
      expect(find.byIcon(Icons.check_rounded), findsOneWidget);
    });

    testWidgets('選ばずに送ると group の下に誤りを出し、最初の chip へフォーカスする', (tester) async {
      String? error;
      late StateSetter update;
      await pumpChips(
        tester,
        StatefulBuilder(
          builder: (context, setState) {
            update = setState;
            return RallyChoiceChips<String>(
              label: '習いたいスポーツ',
              options: options,
              selected: null,
              onSelected: (_) {},
              error: error,
            );
          },
        ),
      );
      expect(find.byIcon(Icons.check_rounded), findsNothing);
      update(() => error = RallyChoiceChips.notSelectedMessage('習いたいスポーツ'));
      await tester.pump();
      expect(find.text('習いたいスポーツが選ばれていません。1 つ選んでください。'), findsOneWidget);
      expect(Focus.of(tester.element(find.text('テニス'))).hasFocus, isTrue);
    });
  });
}
