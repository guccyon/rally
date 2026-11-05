import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/screens/design_catalog_screen.dart';
import 'package:rally_student_app/ui/design_catalog.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:yaml/yaml.dart';

/// デザインシステムのカタログ（lib/ui/design_catalog.dart）が specs/design/components.yaml の部品と合っていることを確かめる。
/// components.yaml に部品・種類を足したら、カタログにも見本を足す。置けないものは notInStudentCatalog に理由とともに書く。
/// 部品そのものの振る舞いは test/ui/components/components_test.dart。
void main() {
  final spec = loadYaml(
    File('../../specs/design/components.yaml').readAsStringSync(),
  ) as YamlMap;
  final components = {
    for (final component in (spec['components'] as YamlList).cast<YamlMap>())
      component['id'] as String: component,
  };

  test('部品と種類は、置けないと書いたもの以外すべてカタログに見本がある', () {
    for (final MapEntry(key: id, value: component) in components.entries) {
      if (notInStudentCatalog.containsKey(id)) continue;
      final entries = designCatalog.where((e) => e.component == id);
      expect(entries, isNotEmpty, reason: id);
      for (final variant in _keysOf(component['variants'])) {
        if (notInStudentCatalog.containsKey('$id / $variant')) continue;
        expect(
          entries.any((e) => e.variant == variant),
          isTrue,
          reason: '$id / $variant',
        );
      }
    }
  });

  test('カタログの部品・種類・状態は components.yaml にあるものだけで、枠の名前は重ならない', () {
    for (final entry in designCatalog) {
      final component = components[entry.component];
      expect(component, isNotNull, reason: entry.name);
      if (entry.variant case final variant?) {
        expect(_keysOf(component!['variants']), contains(variant));
      }
      // default は「状態を付けない見た目」。components.yaml の states に書いていなくてよい
      if (entry.state case final state? when state != 'default') {
        expect(_keysOf(component!['states']), contains(state));
      }
    }
    final names = designCatalog.map((e) => e.name).toList();
    expect(names.toSet().length, names.length);
  });

  test('置けないと書いた部品と種類は components.yaml にあり、カタログには置かない', () {
    for (final name in notInStudentCatalog.keys) {
      switch (name.split(' / ')) {
        case [final id]:
          expect(components, contains(id));
          expect(designCatalog.any((e) => e.component == id), isFalse);
        case [final id, final variant]:
          expect(_keysOf(components[id]?['variants']), contains(variant));
          expect(
            designCatalog.any(
              (e) => e.component == id && e.variant == variant,
            ),
            isFalse,
          );
        default:
          fail('notInStudentCatalog の名前は「部品」か「部品 / 種類」: $name');
      }
    }
  });

  testWidgets('カタログの画面に、部品ごとの見出しとすべての枠の見出しが出る', (tester) async {
    await tester.pumpWidget(
      MaterialApp(theme: buildRallyTheme(), home: const DesignCatalogScreen()),
    );
    await tester.pumpAndSettle();
    // 種類・状態・見本の名前のない枠は、枠の名前が部品の見出しと同じ文字になる
    for (final id in components.keys) {
      if (notInStudentCatalog.containsKey(id)) continue;
      final plain = designCatalog.where((e) => e.name == id).length;
      expect(find.text(id, skipOffstage: false), findsNWidgets(1 + plain));
    }
    for (final entry in designCatalog.where((e) => e.name != e.component)) {
      expect(find.text(entry.name, skipOffstage: false), findsOneWidget);
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('確認のダイアログの見本は、押すとダイアログを開く', (tester) async {
    await tester.pumpWidget(
      MaterialApp(theme: buildRallyTheme(), home: const DesignCatalogScreen()),
    );
    final open = find.text('確認のダイアログを開く');
    await tester.ensureVisible(open);
    await tester.tap(open);
    await tester.pumpAndSettle();
    expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
  });
}

/// components.yaml の variants・states は、辞書ならそのキー、配列ならその要素が名前
List<String> _keysOf(Object? node) => switch (node) {
  final YamlMap map => map.keys.cast<String>().toList(),
  final YamlList list => list.cast<String>().toList(),
  _ => const [],
};
