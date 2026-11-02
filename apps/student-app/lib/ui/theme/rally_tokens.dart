import 'package:flutter/material.dart';

// デザインシステムの基礎（トークン）の写し。正本は specs/design/tokens.yaml（コンセプト rally-court）。
// 名前は tokens.yaml の <分類>.<役割> を lowerCamelCase にしたもの（color.roles.primary → RallyColors.primary）。
// 値を変えるときは tokens.yaml を先に直す（governance.yaml）。

/// 色の役割（color.roles）。明るい配色だけ（color.dark_mode.gen1）。
abstract final class RallyColors {
  static const primary = Color(0xFF1E3FD0);
  static const onPrimary = Color(0xFFFFFFFF);
  static const primaryPressed = Color(0xFF1A2FA8);
  static const primaryContainer = Color(0xFFE6EBFF);
  static const onPrimaryContainer = Color(0xFF14246E);

  /// ライム。文字の色にしない（濃い面の上の強調か、on_secondary の文字の下の面だけ）
  static const secondary = Color(0xFFD4F53C);
  static const onSecondary = Color(0xFF10131A);
  static const text = Color(0xFF10131A);
  static const textMuted = Color(0xFF4A5163);
  static const background = Color(0xFFFFFFFF);
  static const surface = Color(0xFFF1F3F8);

  /// 濃い面。1 画面に 1 か所
  static const inverseSurface = Color(0xFF10131A);
  static const onInverseSurface = Color(0xFFFFFFFF);
  static const onInverseSurfaceMuted = Color(0xFFB8BFCC);

  /// 飾りの区切り線だけ。部品の輪郭には [borderStrong]
  static const border = Color(0xFFDDE1EA);
  static const borderStrong = Color(0xFF7D8599);
  static const focusRing = Color(0xFF1E3FD0);

  /// rgba(16,19,26,0.56)
  static const scrim = Color(0x8F10131A);

  static const info = RallyStatusColors(
    fg: Color(0xFF1E3FD0),
    container: Color(0xFFE6EBFF),
    onContainer: Color(0xFF14246E),
  );
  static const success = RallyStatusColors(
    fg: Color(0xFF0E7A3E),
    container: Color(0xFFE2F6EA),
    onContainer: Color(0xFF0A4F28),
  );
  static const warning = RallyStatusColors(
    fg: Color(0xFF9A5B00),
    container: Color(0xFFFFF1D6),
    onContainer: Color(0xFF5A3500),
  );
  static const danger = RallyStatusColors(
    fg: Color(0xFFC0182F),
    container: Color(0xFFFDE8EB),
    onContainer: Color(0xFF7A0F1E),
  );

  /// 予約のキャンセル済みなどの中立（color.booking_state.cancelled）
  static const neutral = RallyStatusColors(
    fg: textMuted,
    container: surface,
    onContainer: textMuted,
  );
}

/// 状態の色（color.status）。fg は文字・アイコン・輪郭、container は面、onContainer は面の上の文字。
class RallyStatusColors {
  const RallyStatusColors({
    required this.fg,
    required this.container,
    required this.onContainer,
  });

  final Color fg;
  final Color container;
  final Color onContainer;
}

/// 状態の種類。色と一緒に必ずアイコンか文字を出す（color.status_rule）。
enum RallyStatus {
  info,
  success,
  warning,
  danger,
  neutral;

  RallyStatusColors get colors => switch (this) {
    info => RallyColors.info,
    success => RallyColors.success,
    warning => RallyColors.warning,
    danger => RallyColors.danger,
    neutral => RallyColors.neutral,
  };

  IconData get icon => switch (this) {
    info => Icons.info_outline_rounded,
    success => Icons.check_circle_outline_rounded,
    warning => Icons.schedule_rounded,
    danger => Icons.error_outline_rounded,
    neutral => Icons.remove_circle_outline_rounded,
  };
}

/// 余白（spacing.scale_px）。段階にない値を余白に使わない。
abstract final class RallySpacing {
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 16;
  static const double lg = 24;
  static const double xl = 40;

  /// 押せる部品の高さの下限（spacing.rules）
  static const double touchTarget = 48;
}

/// 角丸（shape.radius_px）。
abstract final class RallyRadius {
  static const double sm = 6;
  static const double md = 12;
  static const double pill = 999;
}

/// 動き（motion.durations_ms / easing）。
abstract final class RallyMotion {
  static const fast = Duration(milliseconds: 120);
  static const base = Duration(milliseconds: 200);
  static const slow = Duration(milliseconds: 320);
  static const standard = Cubic(0.2, 0, 0, 1);
  static const enter = Cubic(0, 0, 0, 1);
  static const exit = Cubic(0.3, 0, 1, 1);
}

/// 書体（typography）。大きさ・太さ・行間・字間は tokens.yaml の scale。
abstract final class RallyTypography {
  /// 同梱の書体（Web フォントは読まない）
  static const family = 'Noto Sans CJK JP';
  static const fallback = ['Noto Sans JP', 'sans-serif'];

  /// 数字は等幅
  static const _tnum = [FontFeature.tabularFigures()];

  static TextStyle _style(
    double size,
    FontWeight weight,
    double height,
    double letterSpacingEm,
  ) => TextStyle(
    fontFamily: family,
    fontFamilyFallback: fallback,
    fontSize: size,
    fontWeight: weight,
    height: height,
    letterSpacing: size * letterSpacingEm,
    fontFeatures: _tnum,
  );

  static final display = _style(44, FontWeight.w900, 1.1, -0.02);
  static final headline = _style(28, FontWeight.w900, 1.25, -0.01);
  static final title = _style(20, FontWeight.w700, 1.25, -0.01);
  static final body = _style(16, FontWeight.w400, 1.65, 0.01);
  static final small = _style(14, FontWeight.w400, 1.6, 0.01);
  static final caption = _style(12, FontWeight.w700, 1.5, 0.02);

  /// ボタンの文言（components.yaml の button.size.label: 16px・700）
  static final label = body.copyWith(fontWeight: FontWeight.w700, height: 1.25);
}
