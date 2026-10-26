import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 月額契約の開始（`startSubscription`）。決済は Gen1 ではモック。
class SubscriptionScreen extends ConsumerWidget {
  const SubscriptionScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final subscription = state.subscription;
    final quota = state.entitlement?.liveLessonQuota;
    return FlowScaffold(
      title: '月額契約',
      children: [
        Text('スポーツ: ${sportLabel(state.student!.sport)}'),
        const Text('動画とチャットは送り放題、ビデオ通話は暦月 2 回まで。'),
        const SizedBox(height: 16),
        if (subscription == null) ...[
          const Text('決済はモックです（請求は発生しません）。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.startSubscription,
            child: const Text('月額契約を始める'),
          ),
        ] else ...[
          const Text('月額契約を開始しました'),
          Text('開始: ${formatDateTime(subscription.startedAt)}'),
          if (quota != null)
            Text('今月のビデオ通話: 残り ${quota.remaining} / ${quota.limit} 回'),
        ],
      ],
    );
  }
}
