import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// ボタンの種類（components.yaml の button.variants）。
enum RallyButtonVariant {
  /// 主なボタン。1 画面に 1 つ
  primary,
  secondary,

  /// inverse_surface の面の中だけ
  onDark,

  /// 取り消せない操作の確認の中だけ
  danger,

  /// 補助の操作（「予約をすべて見る」）
  text,
}

/// ボタン（components.yaml の button）。文言は動詞で終える。
/// [loading] のあいだはラベルを残したまま spinner を前に出し、押せなくする（二度押しで同じ操作を 2 回送らない）。
class RallyButton extends StatelessWidget {
  const RallyButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.variant = RallyButtonVariant.primary,
    this.icon,
    this.loading = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final RallyButtonVariant variant;
  final IconData? icon;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    final onPressed = loading ? null : this.onPressed;
    final leading = loading
        ? const SizedBox.square(
            dimension: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : icon == null
        ? null
        : Icon(icon, size: 20);
    final child = Text(label);
    return switch (variant) {
      RallyButtonVariant.primary => _filled(onPressed, leading, child, null),
      RallyButtonVariant.onDark => _filled(
        onPressed,
        leading,
        child,
        _colors(
          RallyColors.secondary,
          RallyColors.onSecondary,
          RallyColors.secondary,
        ),
      ),
      RallyButtonVariant.danger => _filled(
        onPressed,
        leading,
        child,
        _colors(
          RallyColors.danger.fg,
          RallyColors.onPrimary,
          RallyColors.danger.onContainer,
        ),
      ),
      RallyButtonVariant.secondary =>
        leading == null
            ? OutlinedButton(onPressed: onPressed, child: child)
            : OutlinedButton.icon(
                onPressed: onPressed,
                icon: leading,
                label: child,
              ),
      RallyButtonVariant.text =>
        leading == null
            ? TextButton(onPressed: onPressed, child: child)
            : TextButton.icon(
                onPressed: onPressed,
                icon: leading,
                label: child,
              ),
    };
  }

  static Widget _filled(
    VoidCallback? onPressed,
    Widget? leading,
    Widget child,
    ButtonStyle? style,
  ) => leading == null
      ? FilledButton(onPressed: onPressed, style: style, child: child)
      : FilledButton.icon(
          onPressed: onPressed,
          style: style,
          icon: leading,
          label: child,
        );

  /// 面と文字の色だけを差し替える（形・大きさはテーマの FilledButton のまま）。
  static ButtonStyle _colors(Color container, Color label, Color pressed) =>
      ButtonStyle(
        backgroundColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.disabled)) return RallyColors.surface;
          if (states.contains(WidgetState.pressed)) return pressed;
          return container;
        }),
        foregroundColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? RallyColors.textMuted
              : label,
        ),
      );
}
