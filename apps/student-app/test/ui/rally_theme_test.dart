import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/rally_theme.dart';
import 'package:yaml/yaml.dart';

/// lib/ui/rally_theme.dart がデザイントークン（specs/design/tokens.yaml）と同じ値で ThemeData を組むこと、
/// 今までの見た目（ThemeData(colorSchemeSeed: Colors.teal)）から変わっていないことを確かめる。
void main() {
  final spec = loadYaml(
    File('../../specs/design/tokens.yaml').readAsStringSync(),
  ) as YamlMap;
  final base = spec['tokens'] as YamlMap;
  final app = (spec['apps'] as YamlMap)['student-app'] as YamlMap;
  final overrides = app['overrides'] as YamlMap;

  Object? valueOf(String id) {
    expect(base.containsKey(id), isTrue, reason: '$id は tokens にない');
    return overrides[id] ?? (base[id] as YamlMap)['value'];
  }

  Color colorOf(String id) {
    final hex = valueOf(id) as String;
    expect(hex, matches(RegExp(r'^#[0-9A-Fa-f]{6}$')), reason: id);
    return Color(int.parse('FF${hex.substring(1)}', radix: 16));
  }

  final theme = rallyTheme();
  final scheme = theme.colorScheme;
  final schemeColors = <String, Color>{
    'primary': scheme.primary,
    'onPrimary': scheme.onPrimary,
    'primaryContainer': scheme.primaryContainer,
    'onPrimaryContainer': scheme.onPrimaryContainer,
    'error': scheme.error,
    'onError': scheme.onError,
    'errorContainer': scheme.errorContainer,
    'onErrorContainer': scheme.onErrorContainer,
    'surface': scheme.surface,
    'onSurface': scheme.onSurface,
    'onSurfaceVariant': scheme.onSurfaceVariant,
  };
  final text = theme.textTheme;
  final textStyles = <String, TextStyle?>{
    'displayLarge': text.displayLarge,
    'displayMedium': text.displayMedium,
    'displaySmall': text.displaySmall,
    'headlineLarge': text.headlineLarge,
    'headlineMedium': text.headlineMedium,
    'headlineSmall': text.headlineSmall,
    'titleLarge': text.titleLarge,
    'titleMedium': text.titleMedium,
    'titleSmall': text.titleSmall,
    'bodyLarge': text.bodyLarge,
    'bodyMedium': text.bodyMedium,
    'bodySmall': text.bodySmall,
    'labelLarge': text.labelLarge,
    'labelMedium': text.labelMedium,
    'labelSmall': text.labelSmall,
  };

  test('seed は tokens の student-app の seed', () {
    final hex = app['seed'] as String;
    expect(
      RallyTokens.seed,
      Color(int.parse('FF${hex.substring(1)}', radix: 16)),
    );
  });

  test('student-app の上書きは tokens にある名前', () {
    for (final id in overrides.keys.cast<String>()) {
      expect(base.containsKey(id), isTrue, reason: id);
    }
  });

  test('ColorScheme の主要色は tokens の対応どおりの値', () {
    final mapping = (app['color_scheme'] as YamlMap).cast<String, String>();
    expect(mapping.keys.toSet(), schemeColors.keys.toSet());
    for (final MapEntry(key: field, value: id) in mapping.entries) {
      expect(schemeColors[field], colorOf(id), reason: '$field ← $id');
    }
  });

  test('TextTheme の大きさは tokens の対応どおりの値', () {
    final mapping = (app['text_theme'] as YamlMap).cast<String, String>();
    expect(mapping.keys.toSet(), textStyles.keys.toSet());
    for (final MapEntry(key: role, value: id) in mapping.entries) {
      expect(
        textStyles[role]?.fontSize,
        (valueOf(id) as num).toDouble(),
        reason: '$role ← $id',
      );
    }
  });

  test('ColorScheme は今までの seed（Colors.teal）から出るものと同じ', () {
    expect(scheme, ThemeData(colorSchemeSeed: Colors.teal).colorScheme);
  });

  testWidgets('画面で使う TextTheme は今までの既定と同じ', (tester) async {
    Future<TextTheme> resolved(ThemeData data) async {
      late TextTheme result;
      await tester.pumpWidget(
        MaterialApp(
          theme: data,
          home: Builder(
            builder: (context) {
              result = Theme.of(context).textTheme;
              return const SizedBox();
            },
          ),
        ),
      );
      return result;
    }

    final before = await resolved(ThemeData(colorSchemeSeed: Colors.teal));
    final after = await resolved(theme);
    expect(after, before);
  });
}
