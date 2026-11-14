import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/chat.dart';
import '../state/student_flow.dart';
import '../state/subscription.dart';
import '../ui/components/section_loading.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 月額契約の開始（`startSubscription`）。契約プランを選んで始める（既定はスタンダード）。決済は Gen1 ではモック。
/// 開いたら受講権（`getMyEntitlement`）を読み直し、別のタブや端末で契約済みなら、プランの欄の代わりに今の契約のプランと残りの回数を出す
/// （読み直すまではプランの欄を出さない）。あわせて契約プランの一覧（`listSubscriptionPlans`）を読み、各プランにビデオ通話の暦月の上限を添える。
/// 読めなければ回数を出さずにプランの名前だけで選ばせる。
/// チャットの「月額契約へ進む」から開いたときは、契約を始めたあとに開いていたスレッドへ戻る「チャットへ戻る」を出す。
/// 契約と受講権の状態と操作は [subscriptionProvider]、契約するスポーツ（生徒）と busy は [studentFlowProvider] のもの。
class SubscriptionScreen extends ConsumerStatefulWidget {
  const SubscriptionScreen({super.key});

  @override
  ConsumerState<SubscriptionScreen> createState() => _SubscriptionScreenState();
}

class _SubscriptionScreenState extends ConsumerState<SubscriptionScreen> {
  SubscriptionPlan _plan = SubscriptionPlan.standard;

  /// 開いたときの受講権の読み直しが終わったか。終わるまでは、別のタブや端末で契約済みかわからないので、
  /// プランの欄と「月額契約を始める」を出さない
  bool _entitlementChecked = false;

  @override
  void initState() {
    super.initState();
    // 契約の後はサーバーが返した受講権の枠を出すので、何も読まない
    if (ref.read(subscriptionProvider).hasActiveSubscription) {
      _entitlementChecked = true;
      return;
    }
    // 契約の前に見えていても、別のタブや端末で始めたかもしれないので受講権を読み直す（ADR 0010）。
    // あわせてプランごとの上限を読む
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final notifier = ref.read(subscriptionProvider.notifier);
      notifier.loadSubscriptionPlans();
      await notifier.refreshEntitlement();
      if (mounted) setState(() => _entitlementChecked = true);
    });
  }

  /// [plan] の選択肢の 1 行。上限を読めていれば回数を添え、読めていなければ（読み込み中・失敗）名前だけ
  String _choiceLabel(SubscriptionState state, SubscriptionPlan plan) {
    final detail = state.subscriptionPlans
        ?.where((d) => d.plan == plan)
        .firstOrNull;
    return detail == null
        ? subscriptionPlanLabel(plan)
        : subscriptionPlanChoiceLabel(detail);
  }

  @override
  Widget build(BuildContext context) {
    final flow = ref.watch(studentFlowProvider);
    final state = ref.watch(subscriptionProvider);
    final notifier = ref.read(subscriptionProvider.notifier);
    final subscription = state.subscription;
    final quota = state.entitlement?.liveLessonQuota;
    return FlowScaffold(
      title: '月額契約',
      children: [
        Text('スポーツ: ${sportLabel(flow.student!.sport)}'),
        const Text('動画とチャットは送り放題です。ビデオ通話は、選んだプランの回数まで毎月使えます。'),
        const SizedBox(height: 16),
        if (!state.hasActiveSubscription && !_entitlementChecked)
          const SectionLoading(message: '月額契約を読み込んでいます…')
        else if (!state.hasActiveSubscription) ...[
          const Text('プランを選んでください'),
          RadioGroup<SubscriptionPlan>(
            groupValue: _plan,
            onChanged: (plan) => setState(() => _plan = plan!),
            child: Column(
              children: [
                for (final plan in SubscriptionPlan.values)
                  RadioListTile<SubscriptionPlan>(
                    value: plan,
                    enabled: !flow.busy,
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
            onPressed: flow.busy
                ? null
                : () => notifier.startSubscription(plan: _plan),
            child: const Text('月額契約を始める'),
          ),
        ] else ...[
          // 別のタブや端末で始めた契約（受講権だけを読んだ）は、ここで始めたと読ませない。受講権には開始の日時がない
          Text(subscription != null ? '月額契約を開始しました' : '今の月額契約'),
          if ((subscription?.plan ?? state.entitlement?.plan) case final plan?)
            Text('プラン: ${subscriptionPlanLabel(plan)}'),
          if (subscription != null)
            Text('開始: ${formatDateTime(subscription.startedAt)}'),
          if (quota != null) Text('今月のビデオ通話: ${liveLessonQuotaLabel(quota)}'),
          if (flow.subscriptionFromChat) ...[
            const SizedBox(height: 16),
            FilledButton(
              onPressed: flow.busy
                  ? null
                  : ref.read(chatProvider.notifier).returnFromSubscription,
              child: const Text('チャットへ戻る'),
            ),
          ],
        ],
      ],
    );
  }
}
