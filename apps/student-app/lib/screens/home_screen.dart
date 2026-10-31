import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// ホーム（登録後に最初に開く）。これからの予約（`listMyBookings`）・今月のビデオ通話の残り（`getMyEntitlement`）・
/// コーチとのやり取り（`listMessageThreads`）・返ってきた添削（`listMyVideos`）と、添削の依頼の入口を 1 画面に並べる。
/// 欄ごとに別の API を読み、読めなかった欄にだけその旨を出す（ほかの欄はそのまま出す）。
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    return FlowScaffold(
      title: 'ホーム',
      children: [
        if (state.student case final student?)
          Text('${student.displayName}さん、こんにちは'),
        const SizedBox(height: 16),
        const _BookingsSection(),
        const SizedBox(height: 24),
        const _LiveLessonQuotaSection(),
        const SizedBox(height: 24),
        const _ThreadsSection(),
        const SizedBox(height: 24),
        const _ReturnedReviewsSection(),
        const _VideoReviewRequestSection(),
        const SizedBox(height: 24),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshHome,
          child: const Text('ホームを更新する'),
        ),
      ],
    );
  }
}

/// 欄の見出し
class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text(text, style: Theme.of(context).textTheme.titleMedium),
  );
}

/// 読めなかった欄の文言
class _SectionFailure extends StatelessWidget {
  const _SectionFailure(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Card(
    color: Theme.of(context).colorScheme.errorContainer,
    child: Padding(padding: const EdgeInsets.all(12), child: Text(text)),
  );
}

/// これからの予約（開始の近い順に 3 件まで）
class _BookingsSection extends ConsumerWidget {
  const _BookingsSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final upcoming = notifier.upcomingBookings();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _SectionTitle('これからの予約'),
        if (state.homeFailures.contains(HomeSection.bookings))
          const _SectionFailure('予約を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
        else if (state.myBookings != null && upcoming.isEmpty)
          const Text('まだこれからの予約はありません。')
        else
          for (final item in upcoming)
            Card(
              child: ListTile(
                title: Text(
                  formatTimeRange(item.slot.startsAt, item.slot.endsAt),
                ),
                subtitle: Text(
                  [
                    sportLabel(item.slot.sport),
                    if (item.booking.lessonMenu case final menu?)
                      bookingLessonMenuLabel(menu),
                    if (item.slot.coachDisplayName case final name?)
                      'コーチ: $name',
                    bookingStatusLabel(item.booking.status),
                  ].join('・'),
                ),
              ),
            ),
        Wrap(
          spacing: 8,
          children: [
            TextButton(
              onPressed: state.busy ? null : notifier.openMyBookings,
              child: const Text('予約をすべて見る'),
            ),
            TextButton(
              onPressed: state.busy ? null : notifier.goToSlotSearch,
              child: const Text('スロットを探して予約する'),
            ),
          ],
        ),
      ],
    );
  }
}

/// 今月のビデオ通話の残り（月額契約のプランで決まる上限のうち）
class _LiveLessonQuotaSection extends ConsumerWidget {
  const _LiveLessonQuotaSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final entitlement = state.entitlement;
    final quota = entitlement?.liveLessonQuota;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _SectionTitle('今月のビデオ通話'),
        if (state.homeFailures.contains(HomeSection.entitlement))
          const _SectionFailure('ビデオ通話の残りの回数を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
        else if (state.noEntitlement)
          const Text('まだ月額契約はありません。月額契約を始めると、毎月のビデオ通話の残りの回数がここに出ます。')
        else if (quota != null) ...[
          Text(
            liveLessonQuotaLabel(quota),
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          if (entitlement?.plan case final plan?)
            Text('プラン: ${subscriptionPlanLabel(plan)}'),
        ],
      ],
    );
  }
}

/// コーチとのやり取り（未読のあるスレッドと、最近の添削の依頼・返却。押すとそのチャットを開く）
class _ThreadsSection extends ConsumerWidget {
  const _ThreadsSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final threads = notifier.recentThreads();
    // スレッドにはコーチの名前がないので、自分の予約に載っている名前を使う（なければ「担当コーチ」）
    final coachNames = {
      for (final item in state.myBookings ?? const <MyBooking>[])
        item.slot.coachId: ?item.slot.coachDisplayName,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _SectionTitle('コーチとのやり取り'),
        if (state.homeFailures.contains(HomeSection.threads))
          const _SectionFailure('コーチとのやり取りを読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
        else if (state.threads != null && threads.isEmpty)
          const Text('まだ新しいやり取りはありません。')
        else
          for (final thread in threads)
            Card(
              child: ListTile(
                title: Text(
                  coachNames[thread.coachId] == null
                      ? '担当コーチ'
                      : '${coachNames[thread.coachId]} コーチ',
                ),
                subtitle: thread.lastMessage == null
                    ? null
                    : Text(
                        messageSummaryLabel(thread.lastMessage!),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                trailing: thread.unreadCount > 0
                    ? Badge(label: Text('未読 ${thread.unreadCount}'))
                    : null,
                onTap: state.busy ? null : () => notifier.openThread(thread),
              ),
            ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: state.busy ? null : notifier.openChat,
            child: const Text('チャットをすべて見る'),
          ),
        ),
      ],
    );
  }
}

/// 返ってきた添削（返却済みの動画を返却の新しい順に 3 件まで。押すとその添削結果を開く）。
/// 返却済みが 0 件なら欄ごと出さない（読めなかったときは旨を出す）
class _ReturnedReviewsSection extends ConsumerWidget {
  const _ReturnedReviewsSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final failed = state.homeFailures.contains(HomeSection.videos);
    final videos = notifier.returnedVideos();
    if (!failed && videos.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _SectionTitle('返ってきた添削'),
          if (failed)
            const _SectionFailure('返ってきた添削を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
          else
            for (final video in videos)
              Card(
                child: ListTile(
                  title: Text(
                    video.coachDisplayName == null
                        ? '担当コーチ'
                        : '${video.coachDisplayName} コーチ',
                  ),
                  subtitle: Text(
                    [
                      if (video.returnedAt case final at?)
                        '返却: ${formatDateTime(at)}',
                      if (video.lessonMenu case final menu?)
                        videoLessonMenuLabel(menu),
                    ].join('・'),
                  ),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: state.busy
                      ? null
                      : () => notifier.openMyVideoReview(video),
                ),
              ),
        ],
      ),
    );
  }
}

/// 添削の依頼の入口（チャットの添削の依頼を開く）。担当コーチとのスレッドがなければ押せない
class _VideoReviewRequestSection extends ConsumerWidget {
  const _VideoReviewRequestSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final threads = state.threads ?? const <MessageThread>[];
    final available =
        threads.isNotEmpty && !state.homeFailures.contains(HomeSection.threads);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _SectionTitle('動画の添削'),
        Text(
          available
              ? 'フォームの動画を送ると、担当コーチが添削して返します。'
              : '予約が確定すると担当コーチとのチャットが開き、動画の添削を依頼できるようになります。',
        ),
        const SizedBox(height: 8),
        FilledButton.icon(
          onPressed: state.busy || !available
              ? null
              : notifier.startVideoReviewRequest,
          icon: const Icon(Icons.videocam_outlined),
          label: const Text('添削を依頼する'),
        ),
      ],
    );
  }
}
