import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/screens/design_catalog_screen.dart';
import 'package:rally_student_app/ui/design_catalog.dart';
import 'package:rally_student_app/ui/rally_button.dart';
import 'package:rally_student_app/ui/rally_confirm_step.dart';
import 'package:yaml/yaml.dart';

/// デザインシステムの部品（lib/ui/ の Rally〜）とカタログが specs/design/components.yaml と合っていることを確かめる。
/// components.yaml に部品・種類・状態を足したら、部品とカタログ（lib/ui/design_catalog.dart）にも足す。
void main() {
  final spec = loadYaml(
    File('../../specs/design/components.yaml').readAsStringSync(),
  ) as YamlMap;
  final components = (spec['components'] as YamlList).cast<YamlMap>();

  /// components.yaml から、部品 × 種類 × 状態の名前の全部
  final expected = [
    for (final component in components)
      for (final variant in _variantsOf(component))
        for (final state in (component['states'] as YamlList).cast<String>())
          CatalogEntry(
            component['id'] as String,
            variant,
            state,
            (_) => const SizedBox(),
          ).name,
  ];

  test('カタログは components.yaml の部品 × 種類 × 状態をちょうど 1 つずつ持つ', () {
    final actual = designCatalog.map((e) => e.name).toList();
    expect(actual.toSet().length, actual.length);
    expect([...actual]..sort(), [...expected]..sort());
  });

  testWidgets('カタログの画面に、部品ごとの見出しとすべての枠の見出しが出る', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: DesignCatalogScreen()));
    for (final component in components) {
      expect(
        find.text(component['id'] as String, skipOffstage: false),
        findsOneWidget,
      );
    }
    for (final name in expected) {
      expect(find.text(name, skipOffstage: false), findsOneWidget);
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('処理中のボタンは押せず、文言が処理中のものになる', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: RallyButton(
            label: '予約を確定する',
            busy: true,
            busyLabel: '送っています…',
            onPressed: () {},
          ),
        ),
      ),
    );
    expect(find.text('予約を確定する'), findsNothing);
    final button = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '送っています…'),
    );
    expect(button.onPressed, isNull);
  });

  testWidgets('確認の段は内容を見せ、確定とやめるを呼ぶ。処理中は両方押せない', (tester) async {
    var confirmed = 0;
    var cancelled = 0;
    Widget step({required bool busy}) => MaterialApp(
      home: Scaffold(
        body: RallyConfirmStep(
          title: '予約をキャンセルしますか？',
          message: '予約をキャンセルします。',
          items: const [('コーチ', '佐藤 コーチ')],
          confirmLabel: 'キャンセルする',
          busyLabel: 'キャンセルしています…',
          danger: true,
          busy: busy,
          onConfirm: () => confirmed++,
          onCancel: () => cancelled++,
        ),
      ),
    );

    await tester.pumpWidget(step(busy: false));
    expect(find.text('佐藤 コーチ'), findsOneWidget);
    await tester.tap(find.text('キャンセルする'));
    await tester.tap(find.text('やめる'));
    expect((confirmed, cancelled), (1, 1));

    await tester.pumpWidget(step(busy: true));
    await tester.tap(find.text('キャンセルしています…'));
    await tester.tap(find.text('やめる'));
    expect((confirmed, cancelled), (1, 1));
  });
}

/// 部品の種類の id。種類がなければ null の 1 つ
List<String?> _variantsOf(YamlMap component) => switch (component['variants']) {
  final YamlList variants => [
    for (final variant in variants.cast<YamlMap>()) variant['id'] as String,
  ],
  _ => const [null],
};
