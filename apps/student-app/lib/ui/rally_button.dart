import 'package:flutter/material.dart';

/// components.yaml の button の種類
enum RallyButtonVariant { primary, secondary, danger }

/// ボタン（components.yaml の button）。主は [FilledButton]、副は [OutlinedButton]、危険はエラーの色の [FilledButton]。
/// [onPressed] が null なら無効。[busy] の間は押せなくし、[busyLabel] があれば文言をそれにする。
class RallyButton extends StatelessWidget {
  const RallyButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.variant = RallyButtonVariant.primary,
    this.busy = false,
    this.busyLabel,
  });

  final String label;
  final VoidCallback? onPressed;
  final RallyButtonVariant variant;
  final bool busy;
  final String? busyLabel;

  @override
  Widget build(BuildContext context) {
    final onPressed = busy ? null : this.onPressed;
    final child = Text(busy ? (busyLabel ?? label) : label);
    switch (variant) {
      case RallyButtonVariant.primary:
        return FilledButton(onPressed: onPressed, child: child);
      case RallyButtonVariant.secondary:
        return OutlinedButton(onPressed: onPressed, child: child);
      case RallyButtonVariant.danger:
        final scheme = Theme.of(context).colorScheme;
        return FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: scheme.error,
            foregroundColor: scheme.onError,
          ),
          onPressed: onPressed,
          child: child,
        );
    }
  }
}
