import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';
import 'package:yaml/yaml.dart';

/// lib/ui/theme のトークンが正本（specs/design/tokens.yaml）と同じ値を持ち、テーマがそれを使うことを確かめる。
void main() {
  final tokens = loadYaml(
    File('../../specs/design/tokens.yaml').readAsStringSync(),
  ) as YamlMap;
  final color = tokens['color'] as YamlMap;
  final roles = color['roles'] as YamlMap;
  final status = color['status'] as YamlMap;

  Color hex(Object? value) =>
      Color(int.parse('FF${(value as String).substring(1)}', radix: 16));

  test('色の役割は tokens.yaml の color.roles と同じ', () {
    final dart = <String, Color>{
      'primary': RallyColors.primary,
      'on_primary': RallyColors.onPrimary,
      'primary_pressed': RallyColors.primaryPressed,
      'primary_container': RallyColors.primaryContainer,
      'on_primary_container': RallyColors.onPrimaryContainer,
      'secondary': RallyColors.secondary,
      'on_secondary': RallyColors.onSecondary,
      'text': RallyColors.text,
      'text_muted': RallyColors.textMuted,
      'background': RallyColors.background,
      'surface': RallyColors.surface,
      'inverse_surface': RallyColors.inverseSurface,
      'on_inverse_surface': RallyColors.onInverseSurface,
      'on_inverse_surface_muted': RallyColors.onInverseSurfaceMuted,
      'border': RallyColors.border,
      'border_strong': RallyColors.borderStrong,
      'focus_ring': RallyColors.focusRing,
    };
    final hexRoles = roles.keys.where((k) => k != 'scrim').toSet();
    expect(dart.keys.toSet(), hexRoles);
    for (final MapEntry(:key, :value) in dart.entries) {
      expect(value, hex(roles[key]), reason: key);
    }
    // scrim は rgba(16,19,26,0.56)
    expect(roles['scrim'], 'rgba(16,19,26,0.56)');
    expect(RallyColors.scrim.toARGB32(), 0x8F10131A);
  });

  test('状態の色は tokens.yaml の color.status と同じ', () {
    final dart = {
      'info': RallyColors.info,
      'success': RallyColors.success,
      'warning': RallyColors.warning,
      'danger': RallyColors.danger,
    };
    expect(dart.keys.toSet(), status.keys.toSet());
    for (final MapEntry(:key, :value) in dart.entries) {
      final s = status[key] as YamlMap;
      expect(value.fg, hex(s['fg']), reason: key);
      expect(value.container, hex(s['container']), reason: key);
      expect(value.onContainer, hex(s['on_container']), reason: key);
    }
  });

  test('余白・角丸・動きの段は tokens.yaml と同じ', () {
    final spacing = (tokens['spacing'] as YamlMap)['scale_px'] as YamlMap;
    expect(
      [
        RallySpacing.xs,
        RallySpacing.sm,
        RallySpacing.md,
        RallySpacing.lg,
        RallySpacing.xl,
      ],
      [
        spacing['xs'],
        spacing['sm'],
        spacing['md'],
        spacing['lg'],
        spacing['xl'],
      ],
    );
    final radius = (tokens['shape'] as YamlMap)['radius_px'] as YamlMap;
    expect(
      [RallyRadius.sm, RallyRadius.md, RallyRadius.pill],
      [radius['sm'], radius['md'], radius['pill']],
    );
    final motion = (tokens['motion'] as YamlMap)['durations_ms'] as YamlMap;
    expect(
      [
        RallyMotion.fast.inMilliseconds,
        RallyMotion.base.inMilliseconds,
        RallyMotion.slow.inMilliseconds,
      ],
      [motion['fast'], motion['base'], motion['slow']],
    );
  });

  test('書体の段は tokens.yaml の typography.scale と同じ大きさ・太さ・行間', () {
    final scale = (tokens['typography'] as YamlMap)['scale'] as YamlMap;
    final dart = {
      'display': RallyTypography.display,
      'headline': RallyTypography.headline,
      'title': RallyTypography.title,
      'body': RallyTypography.body,
      'small': RallyTypography.small,
      'caption': RallyTypography.caption,
    };
    expect(dart.keys.toSet(), scale.keys.toSet());
    for (final MapEntry(:key, :value) in dart.entries) {
      final s = scale[key] as YamlMap;
      expect(value.fontSize, s['size'], reason: key);
      expect(value.fontWeight!.value, s['weight'], reason: key);
      expect(value.height, s['line_height'], reason: key);
      final em = double.parse(
        (s['letter_spacing'] as String).replaceAll('em', ''),
      );
      expect(
        value.letterSpacing,
        closeTo(value.fontSize! * em, 1e-9),
        reason: key,
      );
      expect(value.fontFamily, RallyTypography.family);
    }
    expect(
      (tokens['typography'] as YamlMap)['families']['sans'],
      startsWith("'${RallyTypography.family}'"),
    );
  });

  test('テーマは明るい配色で、トークンの色と書体を使い、影を付けない', () {
    final theme = buildRallyTheme();
    expect(theme.brightness, Brightness.light);
    expect(theme.colorScheme.primary, RallyColors.primary);
    expect(theme.colorScheme.error, RallyColors.danger.fg);
    expect(theme.colorScheme.onSurface, RallyColors.text);
    expect(theme.colorScheme.outline, RallyColors.borderStrong);
    expect(theme.scaffoldBackgroundColor, RallyColors.background);
    expect(theme.textTheme.bodyMedium!.fontFamily, RallyTypography.family);
    expect(theme.textTheme.headlineSmall!.fontSize, 28);
    expect(theme.cardTheme.elevation, 0);
    expect(theme.cardTheme.color, RallyColors.surface);
    expect(theme.appBarTheme.elevation, 0);
    expect(theme.dialogTheme.elevation, 0);
    expect(theme.colorScheme.shadow, Colors.transparent);
  });
}
