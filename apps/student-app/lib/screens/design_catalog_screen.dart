import 'package:flutter/material.dart';

import '../ui/design_catalog.dart';
import '../ui/theme/rally_tokens.dart';

/// デザインシステムのカタログ。specs/design/components.yaml の部品の見本（lib/ui/design_catalog.dart）を部品ごとに並べる。
/// アプリの画面の流れ（FlowStep）からは開けない。画面のキャプチャ（id `design-catalog`）とテストのための画面。
class DesignCatalogScreen extends StatelessWidget {
  const DesignCatalogScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final components = {for (final entry in designCatalog) entry.component};
    return Scaffold(
      appBar: AppBar(title: const Text('デザインシステムの部品')),
      // 動き（読み込み中の spinner など）を止めて、いつも同じ絵にする（テストとキャプチャで描き終えられるように）。
      // 全部の枠を描いておく（ListView の遅延の描画にしない）
      body: TickerMode(
        enabled: false,
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(RallySpacing.md),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final MapEntry(key: name, value: reason)
                  in notInStudentCatalog.entries)
                Text(
                  '$name: $reason',
                  style: textTheme.bodySmall?.copyWith(
                    color: RallyColors.textMuted,
                  ),
                ),
              for (final component in components) ...[
                Padding(
                  padding: const EdgeInsets.only(
                    top: RallySpacing.lg,
                    bottom: RallySpacing.sm,
                  ),
                  child: Semantics(
                    header: true,
                    child: Text(component, style: textTheme.titleLarge),
                  ),
                ),
                for (final entry in designCatalog.where(
                  (e) => e.component == component,
                )) ...[
                  Text(entry.name, style: textTheme.labelSmall),
                  const SizedBox(height: RallySpacing.xs),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Builder(builder: entry.builder),
                  ),
                  const SizedBox(height: RallySpacing.md),
                ],
              ],
            ],
          ),
        ),
      ),
    );
  }
}
