import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
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
    testWidgets('押すと 1 回だけ呼ばれ、高さは 48px 以上で角は丸い', (tester) async {
      var pressed = 0;
      await pumpPart(
        tester,
        RallyButton(label: '予約する', onPressed: () => pressed++),
      );
      await tester.tap(find.text('予約する'));
      expect(pressed, 1);
      final size = tester.getSize(find.byType(FilledButton));
      expect(size.height, greaterThanOrEqualTo(48));
      final shape = tester
          .widget<Material>(
            find.descendant(
              of: find.byType(FilledButton),
              matching: find.byType(Material),
            ),
          )
          .shape;
      expect(shape, isA<StadiumBorder>());
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
        RallyColors.background,
      );
    });
  });

  testWidgets('状態のタグは、色だけでなく呼び名の文字とアイコンを出す', (tester) async {
    await pumpPart(
      tester,
      const StatusTag(label: '確定待ち', status: RallyStatus.warning),
    );
    expect(find.text('確定待ち'), findsOneWidget);
    expect(find.byIcon(RallyStatus.warning.icon), findsOneWidget);
    final text = tester.widget<Text>(find.text('確定待ち'));
    expect(text.style!.color, RallyColors.warning.onContainer);
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
    handle.dispose();
  });

  testWidgets('知らせは状態の面とアイコンで出し、読み上げに伝える', (tester) async {
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
  });

  testWidgets('空の表示は一言と次の操作を出す', (tester) async {
    await pumpPart(
      tester,
      EmptyState(
        message: 'まだこれからの予約はありません。コーチを探して始めましょう',
        action: RallyButton(
          label: 'コーチを探す',
          variant: RallyButtonVariant.secondary,
          onPressed: () {},
        ),
      ),
    );
    expect(find.textContaining('まだこれからの予約はありません'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'コーチを探す'), findsOneWidget);
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
  });
}
