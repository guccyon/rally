import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../state/subscription.dart';
import '../ui/components/confirm_dialog.dart';
import '../ui/components/list_row.dart';
import '../ui/components/notice.dart';
import '../ui/components/section_loading.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';

/// 自分のプロフィール（`getMyStudentProfile` / `updateMyStudentProfile`）。表示名を見て変え、「保存する」で保存する。
/// 表示名の規則はサーバーが判定し、断られたら（422）サーバーの文言を出す。スポーツは見るだけ（変えられない）。
/// 表示名の欄の下に月額契約の欄（`getMyEntitlement`。契約の有無はサーバーの受講権から出す。ADR 0010）。
/// いちばん下にログアウトの欄（確かめてから [StudentFlowNotifier.logout] で登録の画面へ。家族の端末や学校の PC で次の人に見せない。#1588）。
class ProfileScreen extends ConsumerStatefulWidget {
  const ProfileScreen({super.key});

  @override
  ConsumerState<ProfileScreen> createState() => _ProfileScreenState();
}

class _ProfileScreenState extends ConsumerState<ProfileScreen> {
  late final TextEditingController _displayName;

  /// 直前の「保存する」で保存できたか（入力を変えたら消す）
  bool _saved = false;

  /// 「保存する」を押してから応答が返るまで true
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController(
      text: ref.read(studentFlowProvider).student?.displayName ?? '',
    );
  }

  @override
  void dispose() {
    _displayName.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    setState(() {
      _saved = false;
      _saving = true;
    });
    final saved = await ref
        .read(studentFlowProvider.notifier)
        .saveDisplayName(_displayName.text);
    if (!mounted) return;
    setState(() {
      _saved = saved;
      _saving = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    // 読み直した表示名は、入力を変えていなければ入力欄にも写す
    ref.listen(studentFlowProvider.select((s) => s.student?.displayName), (
      previous,
      next,
    ) {
      if (next != null && _displayName.text == (previous ?? '')) {
        _displayName.text = next;
      }
    });
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final student = state.student;
    final unchanged = _displayName.text == (student?.displayName ?? '');
    return FlowScaffold(
      title: 'プロフィール',
      leading: IconButton(
        tooltip: 'プロフィールを閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeProfile,
      ),
      children: [
        const Text('表示名は担当コーチにも表示されます。'),
        const SizedBox(height: 8),
        TextField(
          controller: _displayName,
          decoration: const InputDecoration(labelText: '表示名'),
          enabled: student != null,
          onChanged: (_) => setState(() => _saved = false),
        ),
        if (student != null) ...[
          const SizedBox(height: 8),
          Text('スポーツ: ${sportLabel(student.sport)}'),
        ],
        const SizedBox(height: 16),
        FilledButton(
          onPressed: state.busy || student == null || unchanged ? null : _save,
          child: Text(_saving ? '保存しています…' : '保存する'),
        ),
        if (_saved) ...[const SizedBox(height: 8), const Text('表示名を保存しました。')],
        const SizedBox(height: 24),
        const _SubscriptionSection(),
        const SizedBox(height: 24),
        const _LogoutSection(),
      ],
    );
  }
}

/// ログアウトの欄。行を押すと確かめ、ログアウトしたら登録の画面になる。ログインの画面はまだなく、
/// ログアウトすると同じ生徒には戻れないので、取り消せない操作として確かめる（patterns.yaml の confirm）。
/// 確かめの前に確定待ちの予約をサーバーから読み（ADR 0010）、あれば確かめにその予約をキャンセルすることを書き、
/// 「ログアウトする」でキャンセルしてからログアウトする（ログアウトすると確定もキャンセルもできず、スロットを押さえ続けるため。#1718）。
/// 読めない・キャンセルできないときはログアウトせず、失敗の文言を出す
class _LogoutSection extends ConsumerWidget {
  const _LogoutSection();

  Future<void> _logout(BuildContext context, WidgetRef ref) async {
    final notifier = ref.read(studentFlowProvider.notifier);
    final pending = await notifier.loadPendingBookingsBeforeLogout();
    if (pending == null || !context.mounted) return;
    final displayName = ref.read(studentFlowProvider).student?.displayName;
    final who = displayName == null ? 'この生徒' : '$displayNameさん';
    final confirmed = await showConfirmDialog(
      context,
      title: 'ログアウトしますか？',
      body:
          '${pending.isEmpty ? '' : _pendingNotice(pending, usesQuota: pending.any(notifier.cancellationUsesQuota))}'
          '$whoの予約・チャット・月額契約をこの端末に出さないようにし、登録の画面に戻ります。'
          'ログインの画面はまだないため、ログアウトするとこのアカウントには戻れません。',
      confirmLabel: 'ログアウトする',
      danger: true,
    );
    if (!confirmed) return;
    if (pending.isEmpty) {
      notifier.logout();
    } else {
      await notifier.cancelPendingAndLogout(pending);
    }
  }

  /// 確定待ちの予約があるときに確かめの本文の頭に置く文。予約の日時（1 件なら日時、複数なら開始のいちばん早い予約の日時と残りの件数）と、
  /// ログアウトすると確定できないのでキャンセルしてからログアウトすること。今キャンセルすると枠を使うならそれも添える
  static String _pendingNotice(
    List<MyBooking> pending, {
    required bool usesQuota,
  }) {
    final first = pending
        .map((item) => item.slot)
        .reduce((a, b) => b.startsAt.isBefore(a.startsAt) ? b : a);
    final when = formatTimeRange(first.startsAt, first.endsAt);
    final which = pending.length == 1
        ? '（$when）'
        : '（$when ほか ${pending.length - 1} 件）';
    return '確定待ちの予約$whichがあります。ログアウトするとこの予約を確定できなくなるため、'
        'キャンセルしてからログアウトします。${usesQuota ? cancelConsumesQuotaNotice : ''}';
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: RallySpacing.sm),
          child: Text('ログアウト', style: Theme.of(context).textTheme.titleMedium),
        ),
        ListRow(
          title: 'ログアウトする',
          subtitle: '家族の端末や学校の PC など、ほかの人も使う端末では使い終わったらログアウトしてください',
          onTap: busy ? null : () => _logout(context, ref),
        ),
      ],
    );
  }
}

/// 月額契約の欄。受講権がなければ（404）「月額契約へ進む」の行（押すと月額契約の画面）、あれば「月額契約」とプラン（この端末で始めた契約なら開始の日も）の行。
/// 契約したあとの行は、この端末で始めた契約（[SubscriptionState.subscription]）があるときだけ押せる
/// （月額契約の画面は契約の中身をそれで出し、ないと契約の前の画面になるため）
class _SubscriptionSection extends ConsumerWidget {
  const _SubscriptionSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final state = ref.watch(subscriptionProvider);
    final flow = ref.read(studentFlowProvider.notifier);
    final subscription = state.subscription;
    final plan = state.entitlement?.plan ?? subscription?.plan;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: RallySpacing.sm),
          child: Text('月額契約', style: Theme.of(context).textTheme.titleMedium),
        ),
        if (state.entitlementFailed)
          const Notice(
            message: '月額契約を読み込めませんでした。プロフィールを開き直すと、もう一度読み込めます。',
            status: RallyStatus.danger,
          )
        else if (state.noEntitlement)
          ListRow(
            title: '月額契約へ進む',
            subtitle: 'まだ月額契約はありません',
            onTap: busy ? null : flow.goToSubscription,
          )
        else if (plan != null)
          ListRow(
            title: '月額契約',
            subtitle: subscriptionSummaryLabel(
              plan,
              startedAt: subscription?.startedAt,
            ),
            onTap: busy || subscription == null ? null : flow.goToSubscription,
          )
        else if (state.entitlementLoading)
          const SectionLoading(message: '月額契約を読み込んでいます…'),
      ],
    );
  }
}
