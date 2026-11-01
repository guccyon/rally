import 'package:flutter/material.dart';

/// 結果の知らせ（components.yaml の result_message）。直前の操作や読み込みの結果を 1 行で知らせる。
/// 失敗（failure）はエラーの色のカードで、支援技術にすぐ読み上げられるよう liveRegion にする。
class RallyResultMessage extends StatelessWidget {
  const RallyResultMessage.success(this.message, {super.key}) : ok = true;

  const RallyResultMessage.failure(this.message, {super.key}) : ok = false;

  final String message;
  final bool ok;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Semantics(
      liveRegion: !ok,
      child: Card(
        color: ok ? scheme.primaryContainer : scheme.errorContainer,
        child: Padding(padding: const EdgeInsets.all(12), child: Text(message)),
      ),
    );
  }
}
