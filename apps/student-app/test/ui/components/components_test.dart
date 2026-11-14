import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/components/booking_card.dart';
import 'package:rally_student_app/ui/components/confirm_dialog.dart';
import 'package:rally_student_app/ui/components/empty_state.dart';
import 'package:rally_student_app/ui/components/list_row.dart';
import 'package:rally_student_app/ui/components/notice.dart';
import 'package:rally_student_app/ui/components/rally_button.dart';
import 'package:rally_student_app/ui/components/rally_card.dart';
import 'package:rally_student_app/ui/components/score_panel.dart';
import 'package:rally_student_app/ui/components/section_loading.dart';
import 'package:rally_student_app/ui/components/status_tag.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

import '../../support/pump.dart';

Future<void> pumpPart(WidgetTester tester, Widget child) => tester.pumpWidget(
  MaterialApp(
    theme: buildRallyTheme(),
    home: Scaffold(
      body: ListView(padding: const EdgeInsets.all(16), children: [child]),
    ),
  ),
);

void main() {
  group('ボタン', () {
    testWidgets('押すと 1 回だけ呼ばれ、高さは 56px で角丸は md（ピル型にしない）', (tester) async {
      var pressed = 0;
      await pumpPart(
        tester,
        RallyButton(label: '予約する', onPressed: () => pressed++),
      );
      await tester.tap(find.text('予約する'));
      expect(pressed, 1);
      final size = tester.getSize(find.byType(FilledButton));
      expect(size.height, greaterThanOrEqualTo(56));
      final shape = tester
          .widget<Material>(
            find.descendant(
              of: find.byType(FilledButton),
              matching: find.byType(Material),
            ),
          )
          .shape;
      expect(shape, isA<RoundedRectangleBorder>());
      expect(
        (shape! as RoundedRectangleBorder).borderRadius,
        BorderRadius.circular(RallyRadius.md),
      );
    });

    testWidgets('読み込み中はラベルを残したまま spinner を出し、押せない', (tester) async {
      var pressed = 0;
      await pumpPart(
        tester,
        RallyButton(label: '送る', loading: true, onPressed: () => pressed++),
      );
      expect(find.text('送る'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      await tester.tap(find.text('送る'));
      expect(pressed, 0);
    });

    testWidgets('種類ごとに面の色が変わる', (tester) async {
      Future<Color?> background(RallyButtonVariant variant) async {
        await pumpPart(
          tester,
          RallyButton(label: '動画を送る', variant: variant, onPressed: () {}),
        );
        final material = tester.widget<Material>(
          find
              .ancestor(of: find.text('動画を送る'), matching: find.byType(Material))
              .first,
        );
        return material.color;
      }

      expect(await background(RallyButtonVariant.primary), RallyColors.primary);
      expect(
        await background(RallyButtonVariant.onDark),
        RallyColors.secondary,
      );
      expect(
        await background(RallyButtonVariant.danger),
        RallyColors.danger.fg,
      );
      expect(
        await background(RallyButtonVariant.secondary),
        RallyColors.primaryContainer,
      );
    });

    testWidgets('副のボタンは塗りで線を付けない。画面の OutlinedButton も同じ見た目', (tester) async {
      await pumpPart(
        tester,
        Column(
          children: [
            RallyButton(
              label: '保存する',
              variant: RallyButtonVariant.secondary,
              onPressed: () {},
            ),
            OutlinedButton(onPressed: () {}, child: const Text('戻る')),
          ],
        ),
      );
      expect(find.byType(OutlinedButton), findsOneWidget);
      for (final label in ['保存する', '戻る']) {
        final material = tester.widget<Material>(
          find
              .ancestor(of: find.text(label), matching: find.byType(Material))
              .first,
        );
        expect(material.color, RallyColors.primaryContainer, reason: label);
        final shape = material.shape! as RoundedRectangleBorder;
        expect(shape.side, BorderSide.none, reason: label);
      }
    });
  });

  BoxDecoration dotOf(WidgetTester tester) =>
      tester
              .widget<Container>(find.byKey(const ValueKey('status-dot')))
              .decoration!
          as BoxDecoration;

  testWidgets('状態のタグは、状態の色の点と呼び名の文字で出し、面の札にしない', (tester) async {
    await pumpPart(
      tester,
      const StatusTag(label: '確定待ち', status: RallyStatus.warning),
    );
    expect(find.text('確定待ち'), findsOneWidget);
    final text = tester.widget<Text>(find.text('確定待ち'));
    expect(text.style!.color, RallyColors.text);
    expect(text.style!.fontWeight, FontWeight.w700);
    expect(dotOf(tester).color, RallyColors.warning.fg);
    expect(
      tester.getSize(find.byKey(const ValueKey('status-dot'))),
      const Size(8, 8),
    );
    // 面の色のある箱は点だけ
    final boxes = tester.widgetList<Container>(find.byType(Container));
    expect(
      boxes.where((c) => (c.decoration as BoxDecoration?)?.color != null),
      hasLength(1),
    );
  });

  testWidgets('neutral の状態のタグは border_strong の点と text_muted の文字', (
    tester,
  ) async {
    await pumpPart(
      tester,
      StatusTag(label: '終了', status: RallySlotState.closed.status),
    );
    expect(dotOf(tester).color, RallyColors.borderStrong);
    final text = tester.widget<Text>(find.text('終了'));
    expect(text.style!.color, RallyColors.textMuted);
  });

  testWidgets('急ぎの状態のタグは字も danger.fg', (tester) async {
    await pumpPart(
      tester,
      const StatusTag(
        label: '開始まで 24 時間以内',
        status: RallyStatus.danger,
        urgent: true,
      ),
    );
    final text = tester.widget<Text>(find.text('開始まで 24 時間以内'));
    expect(text.style!.color, RallyColors.danger.fg);
    expect(dotOf(tester).color, RallyColors.danger.fg);
  });

  testWidgets('スコアの面は、数字と単位と条件を 1 つの文で読み上げる', (tester) async {
    final handle = tester.ensureSemantics();
    await pumpPart(
      tester,
      const ScorePanel(
        eyebrow: '今月のビデオ通話',
        number: '2',
        unit: '回',
        note: '月 2 回のうち',
      ),
    );
    expect(find.bySemanticsLabel('今月のビデオ通話、2 回、月 2 回のうち'), findsOneWidget);
    final number = tester.widget<Text>(find.text('2'));
    expect(number.style!.fontFamily, RallyTypography.numericFamily);
    expect(number.style!.fontSize, 64);
    handle.dispose();
  });

  testWidgets('知らせの inline は白地に左の状態の色の線とアイコンで出し、読み上げに伝える', (tester) async {
    await pumpPart(
      tester,
      const Notice(
        message: '予約を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
        status: RallyStatus.danger,
      ),
    );
    expect(find.byIcon(RallyStatus.danger.icon), findsOneWidget);
    final semantics = tester.widget<Semantics>(
      find
          .ancestor(
            of: find.textContaining('予約を読み込めませんでした'),
            matching: find.byType(Semantics),
          )
          .first,
    );
    expect(semantics.properties.liveRegion, isTrue);
    final text = tester.widget<Text>(find.textContaining('予約を読み込めませんでした'));
    expect(text.style!.color, RallyColors.text);
    final box = tester.widget<Container>(
      find
          .ancestor(
            of: find.textContaining('予約を読み込めませんでした'),
            matching: find.byType(Container),
          )
          .first,
    );
    final decoration = box.decoration! as BoxDecoration;
    expect(decoration.color, RallyColors.background);
    expect(
      (decoration.border! as Border).left,
      BorderSide(color: RallyColors.danger.fg, width: 4),
    );
  });

  testWidgets('成功の知らせは濃い面と完了の印（ライムの丸）', (tester) async {
    await pumpPart(
      tester,
      const Notice(message: '予約が確定しました', status: RallyStatus.success),
    );
    final mark = tester.widget<Container>(
      find.byKey(const ValueKey('done-mark')),
    );
    expect((mark.decoration! as BoxDecoration).color, RallyColors.secondary);
    final box = tester.widget<Container>(
      find
          .ancestor(
            of: find.text('予約が確定しました'),
            matching: find.byType(Container),
          )
          .first,
    );
    expect(
      (box.decoration! as BoxDecoration).color,
      RallyColors.inverseSurface,
    );
  });

  testWidgets('空の表示は一言と次の操作を出す。screen は surface の面と primary、文は text の色', (
    tester,
  ) async {
    var tapped = 0;
    await pumpPart(
      tester,
      EmptyState(
        message: 'まだこれからの予約はありません。コーチを探して始めましょう',
        actionLabel: 'コーチを探す',
        onAction: () => tapped++,
      ),
    );
    final message = tester.widget<Text>(find.textContaining('まだこれからの予約はありません'));
    expect(message.style?.color, RallyColors.text);
    expect(_emptyDecoration(tester).color, RallyColors.surface);
    await tester.tap(find.widgetWithText(FilledButton, 'コーチを探す'));
    expect(tapped, 1);
  });

  testWidgets('欄だけの空の表示（section）は background の面と border の線と secondary', (
    tester,
  ) async {
    await pumpPart(
      tester,
      EmptyState(
        message: 'まだこれからの予約はありません。',
        actionLabel: 'コーチを探す',
        onAction: () {},
        variant: EmptyStateVariant.section,
      ),
    );
    final decoration = _emptyDecoration(tester);
    expect(decoration.color, RallyColors.background);
    expect(decoration.border, Border.all(color: RallyColors.border));
    final button = tester.widget<Material>(
      find
          .ancestor(of: find.text('コーチを探す'), matching: find.byType(Material))
          .first,
    );
    expect(button.color, RallyColors.primaryContainer);
    expect(find.byType(OutlinedButton), findsNothing);
  });

  testWidgets('欄の読み込み中は文で出す', (tester) async {
    await pumpPart(tester, const SectionLoading(message: '予約を読み込んでいます…'));
    expect(find.text('予約を読み込んでいます…'), findsOneWidget);
  });

  testWidgets('押せるカードは全体が 1 つのボタンで、題・日時・状態を 1 つの文で読む', (tester) async {
    final handle = tester.ensureSemantics();
    var tapped = 0;
    await pumpPart(
      tester,
      RallyCard(
        onTap: () => tapped++,
        semanticsLabel: '10/28（水）12:00〜12:30、テニス・コーチ: 山田、確定済み',
        children: const [
          Text('10/28（水）12:00〜12:30'),
          Text('テニス・コーチ: 山田'),
          StatusTag(label: '確定済み', status: RallyStatus.success),
        ],
      ),
    );
    await tester.tap(find.text('テニス・コーチ: 山田'));
    expect(tapped, 1);
    expect(
      find.bySemanticsLabel('10/28（水）12:00〜12:30、テニス・コーチ: 山田、確定済み'),
      findsOneWidget,
    );
    handle.dispose();
  });

  testWidgets('カードは白地に border の 1px で囲む', (tester) async {
    await pumpPart(tester, const RallyCard(children: [Text('山田 花子')]));
    final material = tester.widget<Material>(
      find
          .ancestor(of: find.text('山田 花子'), matching: find.byType(Material))
          .first,
    );
    expect(material.color, RallyColors.background);
    expect(
      (material.shape! as RoundedRectangleBorder).side,
      const BorderSide(color: RallyColors.border),
    );
  });

  testWidgets('予約のカードは日付のコマを大きく出し、次のレッスンだけコマをライムにする', (tester) async {
    Future<Color?> koma({required bool next}) async {
      await pumpPart(
        tester,
        RallyBookingCard(
          month: '10月',
          day: '28',
          weekday: '水',
          time: '12:00〜12:30',
          meta: 'テニス・コーチ: 山田',
          next: next,
        ),
      );
      return (tester
                  .widget<Container>(find.byKey(const ValueKey('date-block')))
                  .decoration!
              as BoxDecoration)
          .color;
    }

    expect(await koma(next: true), RallyColors.secondary);
    expect(await koma(next: false), RallyColors.surface);
    expect(tester.getSize(find.byKey(const ValueKey('date-block'))).width, 64);
    expect(
      tester.widget<Text>(find.text('28')).style!.fontFamily,
      RallyTypography.numericFamily,
    );
    expect(tester.widget<Text>(find.text('12:00〜12:30')).style!.fontSize, 22);
  });

  testWidgets('未読の行は「未読」のしるしを文字で出す', (tester) async {
    var tapped = 0;
    await pumpPart(
      tester,
      ListRow(
        title: 'コーチから返事が届きました',
        subtitle: '10/27 18:04',
        unread: true,
        onTap: () => tapped++,
      ),
    );
    expect(find.text('未読'), findsOneWidget);
    expect(find.byIcon(Icons.chevron_right_rounded), findsOneWidget);
    await tester.tap(find.text('コーチから返事が届きました'));
    expect(tapped, 1);
    expect(
      tester.getSize(find.byType(ListTile)).height,
      greaterThanOrEqualTo(56),
    );
  });

  group('確認のダイアログ', () {
    Future<bool?> open(WidgetTester tester, {required String tap}) async {
      bool? result;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildRallyTheme(),
          home: Builder(
            builder: (context) => Scaffold(
              body: TextButton(
                onPressed: () async => result = await showConfirmDialog(
                  context,
                  title: '予約をキャンセルしますか？',
                  body: '10/28（水）12:00〜12:30 のテニスの予約をキャンセルします。',
                  confirmLabel: 'キャンセルする',
                  danger: true,
                ),
                child: const Text('開く'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('開く'));
      await tester.pumpAndSettle();
      expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
      // cancel を左、confirm を右
      expect(
        tester.getCenter(find.text('やめる')).dx,
        lessThan(tester.getCenter(find.text('キャンセルする')).dx),
      );
      await tester.tap(find.text(tap));
      await tester.pumpAndSettle();
      return result;
    }

    testWidgets('確定すると true', (tester) async {
      expect(await open(tester, tap: 'キャンセルする'), isTrue);
    });

    testWidgets('「やめる」は false', (tester) async {
      expect(await open(tester, tap: 'やめる'), isFalse);
    });

    testWidgets('すばやく二度押ししても閉じるのは確認だけで、開いた画面は残る（#1513）', (tester) async {
      var opened = 0;
      final results = <bool>[];
      await tester.pumpWidget(
        MaterialApp(
          theme: buildRallyTheme(),
          home: Builder(
            builder: (context) => Scaffold(
              body: TextButton(
                onPressed: () async {
                  opened++;
                  results.add(
                    await showConfirmDialog(
                      context,
                      title: '予約をキャンセルしますか？',
                      body: '10/28（水）12:00〜12:30 のテニスの予約をキャンセルします。',
                      confirmLabel: 'キャンセルする',
                      danger: true,
                    ),
                  );
                },
                child: const Text('開く'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('開く'));
      await tester.pumpAndSettle();

      await doubleTapInOneFrame(tester, find.text('キャンセルする'));

      expect(tester.takeException(), isNull);
      expect(opened, 1);
      expect(results, [true]);
      expect(find.text('予約をキャンセルしますか？'), findsNothing);
      expect(find.text('開く'), findsOneWidget);
    });
  });
}

BoxDecoration _emptyDecoration(WidgetTester tester) =>
    tester
            .widget<Container>(
              find
                  .descendant(
                    of: find.byType(EmptyState),
                    matching: find.byType(Container),
                  )
                  .first,
            )
            .decoration!
        as BoxDecoration;
