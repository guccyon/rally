import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/components/rally_navigation_bar.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

/// 下のタブ（navigation_bar）の部品（components.yaml の 0.12.0）。
void main() {
  Future<void> pumpBar(WidgetTester tester, Widget bar) => tester.pumpWidget(
    MaterialApp(
      theme: buildRallyTheme(),
      home: Scaffold(body: const SizedBox(), bottomNavigationBar: bar),
    ),
  );

  testWidgets('高さ 72px・影なしで、5 つの行き先のラベルをいつも出す', (tester) async {
    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.home,
        onSelected: (_) {},
      ),
    );
    final bar = tester.widget<NavigationBar>(find.byType(NavigationBar));
    expect(tester.getSize(find.byType(NavigationBar)).height, 72);
    final theme = NavigationBarTheme.of(
      tester.element(find.byType(NavigationBar)),
    );
    expect(theme.elevation, 0);
    expect(
      bar.labelBehavior ?? theme.labelBehavior,
      NavigationDestinationLabelBehavior.alwaysShow,
    );
    for (final destination in RallyNavDestination.values) {
      expect(find.text(destination.label), findsOneWidget);
    }
    expect(RallyNavDestination.values.map((d) => d.label), [
      'ホーム',
      '予約',
      'チャット',
      'お知らせ',
      'マイページ',
    ]);
  });

  testWidgets('押すと行き先を渡し、選択中を押すと上端へ戻す', (tester) async {
    final selected = <RallyNavDestination>[];
    var reselected = 0;
    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.home,
        onSelected: selected.add,
        onReselected: () => reselected++,
      ),
    );
    await tester.tap(find.text('予約'));
    expect(selected, [RallyNavDestination.bookings]);
    await tester.tap(find.text('ホーム'));
    expect(selected, [RallyNavDestination.bookings]);
    expect(reselected, 1);
  });

  testWidgets('選択中は塗りのアイコンと 900 の text、ほかは線のアイコンと 700 の text_muted', (
    tester,
  ) async {
    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.chat,
        onSelected: (_) {},
      ),
    );
    expect(find.byIcon(Icons.chat_bubble), findsOneWidget);
    expect(find.byIcon(Icons.home_outlined), findsOneWidget);
    expect(find.byIcon(Icons.home), findsNothing);
    final theme = NavigationBarTheme.of(
      tester.element(find.byType(NavigationBar)),
    );
    final selected = theme.labelTextStyle!.resolve({WidgetState.selected})!;
    final other = theme.labelTextStyle!.resolve({})!;
    expect(selected.fontWeight, FontWeight.w900);
    expect(selected.color, RallyColors.text);
    expect(other.fontWeight, FontWeight.w700);
    expect(other.color, RallyColors.textMuted);
    expect(theme.indicatorColor, RallyColors.primaryContainer);
  });

  testWidgets('チャットは未読の数、お知らせは点。読み上げは 1 つの文', (tester) async {
    final handle = tester.ensureSemantics();
    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.home,
        onSelected: (_) {},
        chatUnreadCount: 3,
        hasUnreadNotifications: true,
      ),
    );
    expect(find.text('3'), findsOneWidget);
    expect(find.bySemanticsLabel('チャット、未読 3 件、タブ、5 個中 3 個目'), findsOneWidget);
    expect(find.bySemanticsLabel('お知らせ、未読あり、タブ、5 個中 4 個目'), findsOneWidget);
    expect(
      tester.getSemantics(find.bySemanticsLabel('ホーム、タブ、5 個中 1 個目')),
      matchesSemantics(
        label: 'ホーム、タブ、5 個中 1 個目',
        isSelected: true,
        hasSelectedState: true,
        hasTapAction: true,
      ),
    );
    handle.dispose();
  });

  testWidgets('100 件以上は「99+」、未読の数を読めないときはしるしを出さない', (tester) async {
    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.chat,
        onSelected: (_) {},
        chatUnreadCount: 120,
      ),
    );
    expect(find.text('99+'), findsOneWidget);

    await pumpBar(
      tester,
      RallyNavigationBar(
        selected: RallyNavDestination.home,
        onSelected: (_) {},
      ),
    );
    expect(find.text('99+'), findsNothing);
    expect(find.text('0'), findsNothing);
  });

  testWidgets('キーボードが出ているあいだは隠す', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRallyTheme(),
        home: MediaQuery(
          data: const MediaQueryData(viewInsets: EdgeInsets.only(bottom: 300)),
          child: RallyNavigationBar(
            selected: RallyNavDestination.home,
            onSelected: (_) {},
          ),
        ),
      ),
    );
    expect(find.byType(NavigationBar), findsNothing);
  });
}
