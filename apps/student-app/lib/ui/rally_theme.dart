import 'package:flutter/material.dart';

/// デザイントークン（正本は specs/design/tokens.yaml。値は student-app の上書きを当てたもの）。
/// 名前は tokens.yaml の id（例: color.text.muted → textMuted）。
/// ずれていないかは test/ui/rally_theme_test.dart で確かめる。
abstract final class RallyTokens {
  /// ColorScheme を作る seed（apps.student-app.seed）
  static const seed = Color(0xFF009688);

  static const primary = Color(0xFF006A60);
  static const primaryStrong = Color(0xFF005048);
  static const primaryFaint = Color(0xFF9EF2E4);
  static const danger = Color(0xFFBA1A1A);
  static const dangerStrong = Color(0xFF93000A);
  static const dangerFaint = Color(0xFFFFDAD6);
  static const textDefault = Color(0xFF161D1C);
  static const textMuted = Color(0xFF3F4947);
  static const textInverse = Color(0xFFFFFFFF);
  static const surfaceDefault = Color(0xFFF4FBF8);

  static const fontSize2xs = 11.0;
  static const fontSizeXs = 12.0;
  static const fontSizeSm = 14.0;
  static const fontSizeBase = 16.0;
  static const fontSizeLg = 22.0;
  static const fontSizeXl = 24.0;
  static const fontSize2xl = 28.0;
  static const fontSize3xl = 32.0;
  static const fontSize4xl = 36.0;
  static const fontSize5xl = 45.0;
  static const fontSize6xl = 57.0;
}

/// アプリ全体の ThemeData。ColorScheme は seed から作って主要色をトークンで上書きし、
/// TextTheme は役割ごとの大きさをトークンで決める（対応は tokens.yaml の color_scheme / text_theme）。
ThemeData rallyTheme() {
  final colorScheme = ColorScheme.fromSeed(seedColor: RallyTokens.seed)
      .copyWith(
        primary: RallyTokens.primary,
        onPrimary: RallyTokens.textInverse,
        primaryContainer: RallyTokens.primaryFaint,
        onPrimaryContainer: RallyTokens.primaryStrong,
        error: RallyTokens.danger,
        onError: RallyTokens.textInverse,
        errorContainer: RallyTokens.dangerFaint,
        onErrorContainer: RallyTokens.dangerStrong,
        surface: RallyTokens.surfaceDefault,
        onSurface: RallyTokens.textDefault,
        onSurfaceVariant: RallyTokens.textMuted,
      );
  const textTheme = TextTheme(
    displayLarge: TextStyle(fontSize: RallyTokens.fontSize6xl),
    displayMedium: TextStyle(fontSize: RallyTokens.fontSize5xl),
    displaySmall: TextStyle(fontSize: RallyTokens.fontSize4xl),
    headlineLarge: TextStyle(fontSize: RallyTokens.fontSize3xl),
    headlineMedium: TextStyle(fontSize: RallyTokens.fontSize2xl),
    headlineSmall: TextStyle(fontSize: RallyTokens.fontSizeXl),
    titleLarge: TextStyle(fontSize: RallyTokens.fontSizeLg),
    titleMedium: TextStyle(fontSize: RallyTokens.fontSizeBase),
    titleSmall: TextStyle(fontSize: RallyTokens.fontSizeSm),
    bodyLarge: TextStyle(fontSize: RallyTokens.fontSizeBase),
    bodyMedium: TextStyle(fontSize: RallyTokens.fontSizeSm),
    bodySmall: TextStyle(fontSize: RallyTokens.fontSizeXs),
    labelLarge: TextStyle(fontSize: RallyTokens.fontSizeSm),
    labelMedium: TextStyle(fontSize: RallyTokens.fontSizeXs),
    labelSmall: TextStyle(fontSize: RallyTokens.fontSize2xs),
  );
  return ThemeData(colorScheme: colorScheme, textTheme: textTheme);
}
