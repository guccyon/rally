import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/components/description_list.dart';
import 'package:rally_student_app/ui/components/status_tag.dart';
import 'package:rally_student_app/ui/components/timeline.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

/// 詳細の部品（components.yaml の description_list・timeline。0.8.0、#1319）。
Future<void> pumpPart(
  WidgetTester tester,
  Widget child, {
  Size size = const Size(360, 800),
}) {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  return tester.pumpWidget(
    MaterialApp(
      theme: buildRallyTheme(),
      home: Scaffold(
        body: ListView(
          padding: const EdgeInsets.all(16),
          children: [child],
        ),
      ),
    ),
  );
}

Color? _dot(WidgetTester tester, int index) =>
    (tester
                .widgetList<Container>(find.byKey(const ValueKey('timeline-dot')))
                .elementAt(index)
                .decoration
            as BoxDecoration?)
        ?.color;

void main() {
  group('ラベルと値の並び', () {
    const items = [
      DescriptionItem('レッスンメニュー', text: 'フォアハンドの基礎（30 分）'),
      DescriptionItem('種類'),
      DescriptionItem(
        '今の状態',
        value: StatusTag(label: '確定済み', status: RallyStatus.success),
      ),
    ];

    testWidgets('値がない項目も「—」で残し、ラベルは small 700 text_muted', (tester) async {
      await pumpPart(tester, const DescriptionList(items: items));
      expect(find.text('—'), findsOneWidget);
      expect(find.text('確定済み'), findsOneWidget);
      final term = tester.widget<Text>(find.text('レッスンメニュー'));
      expect(term.style?.fontWeight, FontWeight.w700);
      expect(term.style?.color, RallyColors.textMuted);
    });

    testWidgets('画面の幅 600px 以上の inline は term 160px の横並び、600px 未満は上下に積む', (
      tester,
    ) async {
      await pumpPart(
        tester,
        const DescriptionList(items: items),
        size: const Size(800, 800),
      );
      final wideTerm = tester.getTopLeft(find.text('レッスンメニュー'));
      final wideValue = tester.getTopLeft(find.text('フォアハンドの基礎（30 分）'));
      expect(wideValue.dx - wideTerm.dx, DescriptionList.termWidth + 16);

      await pumpPart(tester, const DescriptionList(items: items));
      final narrowTerm = tester.getTopLeft(find.text('レッスンメニュー'));
      final narrowValue = tester.getTopLeft(find.text('フォアハンドの基礎（30 分）'));
      expect(narrowValue.dx, narrowTerm.dx);
      expect(narrowValue.dy, greaterThan(narrowTerm.dy));
    });

    testWidgets('項目ごとの読み込み中と失敗は、その値だけを置き換える', (tester) async {
      await pumpPart(
        tester,
        const DescriptionList(
          items: [
            DescriptionItem('表示名', text: '鈴木 一郎'),
            DescriptionItem('スポーツ', loading: true),
            DescriptionItem('残りの回数', error: '回数を読めませんでした。'),
          ],
        ),
      );
      expect(find.text('鈴木 一郎'), findsOneWidget);
      expect(find.text('読み込み中…'), findsOneWidget);
      final error = tester.widget<Text>(find.text('回数を読めませんでした。'));
      expect(error.style?.color, RallyColors.danger.fg);
    });

    testWidgets('全体の読み込み中と失敗は項目を出さない', (tester) async {
      await pumpPart(
        tester,
        const DescriptionList(items: items, loading: '予約を読み込んでいます…'),
      );
      expect(find.text('予約を読み込んでいます…'), findsOneWidget);
      expect(find.text('レッスンメニュー'), findsNothing);

      await pumpPart(
        tester,
        const DescriptionList(items: items, error: '予約を読み込めませんでした。'),
      );
      expect(find.text('予約を読み込めませんでした。'), findsOneWidget);
      expect(find.text('レッスンメニュー'), findsNothing);
    });
  });

  group('経緯の並び', () {
    testWidgets('渡した順に並べ、点は booking_state の対応の色、出来事を 1 つの文で読む', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpPart(
        tester,
        Timeline(
          semanticsLabel: '状態が変わった記録',
          empty: '状態が変わった記録はありません',
          events: [
            TimelineEvent(
              time: '10/20（火）9:12',
              label: '確定待ち',
              status: RallyBookingState.pending.status,
            ),
            TimelineEvent(
              time: '10/21（水）8:30',
              label: '確定済み',
              status: RallyBookingState.confirmed.status,
              meta: '本部: 鈴木',
            ),
            TimelineEvent(
              time: '10/27（火）18:04',
              label: 'キャンセル済み',
              status: RallyBookingState.cancelled.status,
            ),
          ],
        ),
      );
      expect(_dot(tester, 0), RallyColors.warning.fg);
      expect(_dot(tester, 1), RallyColors.success.fg);
      expect(_dot(tester, 2), RallyColors.borderStrong);
      expect(
        tester.getTopLeft(find.text('確定待ち')).dy,
        lessThan(tester.getTopLeft(find.text('確定済み')).dy),
      );
      expect(
        find.bySemanticsLabel('10/21（水）8:30、確定済み、本部: 鈴木'),
        findsOneWidget,
      );
      handle.dispose();
    });

    testWidgets('状態の対応のない出来事は border_strong の点と body 700', (tester) async {
      await pumpPart(
        tester,
        const Timeline(
          semanticsLabel: 'お知らせの経緯',
          empty: '経緯はありません',
          events: [TimelineEvent(time: '11/2（月）10:25', label: '取り下げた')],
        ),
      );
      expect(_dot(tester, 0), RallyColors.borderStrong);
      final label = tester.widget<Text>(find.text('取り下げた'));
      expect(label.style?.fontSize, RallyTypography.body.fontSize);
      expect(label.style?.fontWeight, FontWeight.w700);
    });

    testWidgets('0 件・読み込み中・失敗は点を出さない', (tester) async {
      await pumpPart(
        tester,
        const Timeline(
          semanticsLabel: '状態が変わった記録',
          empty: '状態が変わった記録はありません',
          events: [],
        ),
      );
      expect(find.text('状態が変わった記録はありません'), findsOneWidget);
      expect(find.byKey(const ValueKey('timeline-dot')), findsNothing);

      await pumpPart(
        tester,
        const Timeline(
          semanticsLabel: '状態が変わった記録',
          empty: '状態が変わった記録はありません',
          events: [],
          error: '状態が変わった記録を読めませんでした。',
        ),
      );
      expect(find.text('状態が変わった記録を読めませんでした。'), findsOneWidget);
    });
  });
}
