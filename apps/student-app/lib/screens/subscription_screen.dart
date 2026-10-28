import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 月額契約の開始（`startSubscription`）。契約プランを選んで始める（既定はスタンダード）。決済は Gen1 ではモック。
class SubscriptionScreen extends ConsumerStatefulWidget {
  const SubscriptionScreen({super.key});

  @override
  ConsumerState<SubscriptionScreen> createState() => _SubscriptionScreenState();
}

class _SubscriptionScreenState extends ConsumerState<SubscriptionScreen> {
  SubscriptionPlan _plan = SubscriptionPlan.standard;

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final subscription = state.subscription;
    final quota = state.entitlement?.liveLessonQuota;
    return FlowScaffold(
      title: '月額契約',
      children: [
        Text('スポーツ: ${sportLabel(state.student!.sport)}'),
        const Text('動画とチャットは送り放題です。ビデオ通話は、選んだプランの回数まで毎月使えます。'),
        const SizedBox(height: 16),
        if (subscription == null) ...[
          const Text('プランを選んでください'),
          RadioGroup<SubscriptionPlan>(
            groupValue: _plan,
            onChanged: (plan) => setState(() => _plan = plan!),
            child: Column(
              children: [
                for (final plan in SubscriptionPlan.values)
                  RadioListTile<SubscriptionPlan>(
                    value: plan,
                    enabled: !state.busy,
                    title: Text(subscriptionPlanChoiceLabel(plan)),
                    contentPadding: EdgeInsets.zero,
                  ),
              ],
            ),
          ),
          const Text('決済はモックです（請求は発生しません）。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy
                ? null
                : () => notifier.startSubscription(plan: _plan),
            child: const Text('月額契約を始める'),
          ),
        ] else ...[
          const Text('月額契約を開始しました'),
          Text('プラン: ${subscriptionPlanLabel(subscription.plan)}'),
          Text('開始: ${formatDateTime(subscription.startedAt)}'),
          if (quota != null) Text('今月のビデオ通話: ${liveLessonQuotaLabel(quota)}'),
        ],
      ],
    );
  }
}
