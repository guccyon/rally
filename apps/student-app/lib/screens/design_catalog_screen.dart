import 'package:flutter/material.dart';

import '../ui/design_catalog.dart';

/// デザインシステムのカタログ。部品 × 種類 × 状態（lib/ui/design_catalog.dart）を部品ごとにすべて並べる。
/// アプリの画面の流れ（FlowStep）からは開けない。画面のキャプチャ（id `design-catalog`）とテストのための画面。
class DesignCatalogScreen extends StatelessWidget {
  const DesignCatalogScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final components = {for (final entry in designCatalog) entry.component};
    return Scaffold(
      appBar: AppBar(title: const Text('デザインシステムの部品')),
      // 全部の枠を描いておく（ListView の遅延の描画にしない。テストとキャプチャで全部を見る）
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final component in components) ...[
              Padding(
                padding: const EdgeInsets.only(top: 8, bottom: 8),
                child: Semantics(
                  header: true,
                  child: Text(component, style: textTheme.titleLarge),
                ),
              ),
              for (final entry in designCatalog.where(
                (e) => e.component == component,
              )) ...[
                Text(entry.name, style: textTheme.labelSmall),
                const SizedBox(height: 4),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Builder(builder: entry.builder),
                ),
                const SizedBox(height: 16),
              ],
            ],
          ],
        ),
      ),
    );
  }
}
