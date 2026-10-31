import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 月額契約の開始（`startSubscription`）。契約プランを選んで始める（既定はスタンダード）。決済は Gen1 ではモック。
/// 開いたら契約プランの一覧（`listSubscriptionPlans`）を読み、各プランにビデオ通話の暦月の上限を添える。
/// 読めなければ回数を出さずにプランの名前だけで選ばせる。
class SubscriptionScreen extends ConsumerStatefulWidget {
  const SubscriptionScreen({super.key});

  @override
  ConsumerState<SubscriptionScreen> createState() => _SubscriptionScreenState();
}

class _SubscriptionScreenState extends ConsumerState<SubscriptionScreen> {
  SubscriptionPlan _plan = SubscriptionPlan.standard;

  @override
  void initState() {
    super.initState();
    // 契約の前だけ、プランごとの上限を読む（契約の後はサーバーが返す受講権の枠を出す）
    if (ref.read(studentFlowProvider).subscription == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        ref.read(studentFlowProvider.notifier).loadSubscriptionPlans();
      });
    }
  }

  /// [plan] の選択肢の 1 行。上限を読めていれば回数を添え、読めていなければ（読み込み中・失敗）名前だけ
  String _choiceLabel(StudentFlowState state, SubscriptionPlan plan) {
    final detail = state.subscriptionPlans
        ?.where((d) => d.plan == plan)
        .firstOrNull;
    return detail == null
        ? subscriptionPlanLabel(plan)
        : subscriptionPlanChoiceLabel(detail);
  }

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
                    title: Text(_choiceLabel(state, plan)),
                    contentPadding: EdgeInsets.zero,
                  ),
              ],
            ),
          ),
          if (state.subscriptionPlansFailed)
            const Text('プランごとのビデオ通話の回数を読み込めませんでした。プランの名前で選んで始められます。'),
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
