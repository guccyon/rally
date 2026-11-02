import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// カード（components.yaml の card）。1 件のまとまりを surface の面で囲む（影なし）。
/// [onTap] があるときはカード全体を 1 つのボタンにする（中にボタンを置かない）。押すと primary_container の面。
class RallyCard extends StatelessWidget {
  const RallyCard({
    super.key,
    required this.children,
    this.onTap,
    this.semanticsLabel,
  });

  /// 題・meta・status_tag の順。行と行のあいだは sm 8px
  final List<Widget> children;
  final VoidCallback? onTap;

  /// 押せるカードの読み上げ（題・日時・状態を 1 つの文で）
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final radius = BorderRadius.circular(RallyRadius.md);
    final content = Padding(
      padding: const EdgeInsets.all(RallySpacing.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        spacing: RallySpacing.sm,
        children: children,
      ),
    );
    return Material(
      color: RallyColors.surface,
      borderRadius: radius,
      child: onTap == null
          ? content
          : Semantics(
              button: true,
              label: semanticsLabel,
              excludeSemantics: semanticsLabel != null,
              child: InkWell(
                onTap: onTap,
                borderRadius: radius,
                splashFactory: NoSplash.splashFactory,
                highlightColor: RallyColors.primaryContainer,
                child: content,
              ),
            ),
    );
  }
}
