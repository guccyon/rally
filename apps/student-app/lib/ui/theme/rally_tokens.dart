import 'package:flutter/material.dart';

// デザインシステムの基礎（トークン）の写し。正本は specs/design/tokens.yaml（コンセプト rally-court）。
// 名前は tokens.yaml の <分類>.<役割> を lowerCamelCase にしたもの（color.roles.primary → RallyColors.primary）。
// 値を変えるときは tokens.yaml を先に直す（governance.yaml）。

/// 色の役割（color.roles）。明るい配色だけ（color.dark_mode.gen1）。
/// 0.5.0 で青をやめ、primary をインク・灰を中立の灰にした（「スコアボード」の方向）。
abstract final class RallyColors {
  /// インク。主なボタン・リンク・選択中。inverse_surface の上には置かない
  static const primary = Color(0xFF0E1013);
  static const onPrimary = Color(0xFFFFFFFF);
  static const primaryPressed = Color(0xFF33372F);

  /// 副のボタン・選択中の行・押下の面
  static const primaryContainer = Color(0xFFE4E6E0);

  /// 副のボタンの押下
  static const primaryContainerPressed = Color(0xFFD6D9D0);
  static const onPrimaryContainer = Color(0xFF0E1013);

  /// ライム。文字の色にしない。score_panel の中・次のレッスンの日付のコマ・完了の印の 3 つだけ
  static const secondary = Color(0xFFD4F53C);
  static const onSecondary = Color(0xFF0E1013);
  static const text = Color(0xFF0E1013);
  static const textMuted = Color(0xFF5B6058);

  /// 画面の地・カード・欄
  static const background = Color(0xFFFFFFFF);

  /// 淡い面（空の表示・無効・日付のコマ）。カードと欄を囲う面にしない
  static const surface = Color(0xFFF2F3EE);

  /// 濃い面。1 画面に 1 か所（成功の知らせは数えない）
  static const inverseSurface = Color(0xFF0E1013);
  static const onInverseSurface = Color(0xFFFFFFFF);
  static const onInverseSurfaceMuted = Color(0xFFB9BDB3);

  /// カードの輪郭と区切り線。押せることを伝える輪郭には [borderStrong]
  static const border = Color(0xFFE4E6E0);

  /// 入力欄の輪郭・中立の状態の点
  static const borderStrong = Color(0xFF7A7F74);

  /// 濃い面の上では [secondary]
  static const focusRing = Color(0xFF667F00);

  /// rgba(14,16,19,0.56)
  static const scrim = Color(0x8F0E1013);

  static const info = RallyStatusColors(
    fg: Color(0xFF0E1013),
    container: Color(0xFFF2F3EE),
    onContainer: Color(0xFF0E1013),
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

  /// 予約のキャンセル済み・スロットの終了などの中立（color.booking_state.cancelled・slot_state.closed）。
  /// 点は [borderStrong]、文字は [textMuted]（color.state_rules）
  static const neutral = RallyStatusColors(
    fg: borderStrong,
    container: surface,
    onContainer: textMuted,
  );
}

/// 状態の色（color.status）。fg は状態の点とアイコン。container は data_table の急ぎの行だけで、
/// 状態のタグと知らせの面には使わない（color.status_rule）。onContainer は互換のために残す。
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

/// 状態の種類。色の点と一緒に必ず glossary の呼び名の文字を出す（color.status_rule）。
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

/// スロットの状態（glossary の slot.states）と色の対応（color.slot_state）。
enum RallySlotState {
  open,
  booked,
  closed;

  RallyStatus get status => switch (this) {
    open => RallyStatus.info,
    booked => RallyStatus.success,
    closed => RallyStatus.neutral,
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

  /// student-app のボタンの高さ（components.yaml の button.size.height_px）
  static const double buttonHeight = 56;
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
/// 日時と回数の大きな数字（display・figure・figureSm）は numeric の Barlow Condensed（pubspec の fonts に同梱、OFL 1.1）。
/// それ以外は sans の Noto Sans CJK JP で、数字は等幅。
abstract final class RallyTypography {
  /// 和文と本文（typography.families.sans）。Web フォントは読まない
  static const family = 'Noto Sans CJK JP';
  static const fallback = ['Noto Sans JP', 'sans-serif'];

  /// 日時と回数の数字（typography.families.numeric）。和文と約物（〜）は [family] で出る
  static const numericFamily = 'Barlow Condensed';
  static const numericFallback = [family, ...fallback];

  /// 数字は等幅
  static const _tnum = [FontFeature.tabularFigures()];

  static TextStyle _style(
    double size,
    FontWeight weight,
    double height,
    double letterSpacingEm, {
    bool numeric = false,
  }) => TextStyle(
    fontFamily: numeric ? numericFamily : family,
    fontFamilyFallback: numeric ? numericFallback : fallback,
    fontSize: size,
    fontWeight: weight,
    height: height,
    letterSpacing: size * letterSpacingEm,
    fontFeatures: _tnum,
  );

  /// 残りの回数・次のレッスンの日時の数字。1 画面 1 か所
  static final display = _style(64, FontWeight.w800, 1.0, 0, numeric: true);
  static final headline = _style(32, FontWeight.w900, 1.2, -0.03);
  static final title = _style(17, FontWeight.w900, 1.4, 0);

  /// 予約のカードの日付のコマの日・一覧のまとめの数
  static final figure = _style(32, FontWeight.w700, 1.0, 0, numeric: true);

  /// カードの時刻（12:00〜12:30）
  static final figureSm = _style(22, FontWeight.w700, 1.1, 0.01, numeric: true);
  static final body = _style(16, FontWeight.w400, 1.65, 0.01);
  static final small = _style(14, FontWeight.w400, 1.6, 0.01);
  static final caption = _style(12, FontWeight.w700, 1.5, 0.02);

  /// ボタンの文言（components.yaml の button.size.label: 16px・900）
  static final label = body.copyWith(fontWeight: FontWeight.w900, height: 1.25);
}
