import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';

/// 各画面の共通の枠。直前の操作の失敗と、API を呼んでいる間の表示を出す。
class FlowScaffold extends ConsumerWidget {
  const FlowScaffold({super.key, required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    return Scaffold(
      appBar: AppBar(title: Text(title)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (state.busy) const LinearProgressIndicator(),
          if (state.errorMessage case final message?)
            Card(
              color: Theme.of(context).colorScheme.errorContainer,
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Text(message),
              ),
            ),
          ...children,
        ],
      ),
    );
  }
}
