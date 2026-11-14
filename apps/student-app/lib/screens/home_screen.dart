import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/chat.dart';
import '../state/coach_search.dart';
import '../state/favorite_coaches.dart';
import '../state/lesson_completions.dart';
import '../state/next_actions.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../state/subscription.dart';
import '../ui/components/booking_card.dart';
import '../ui/components/list_row.dart';
import '../ui/components/notice.dart';
import '../ui/components/rally_button.dart';
import '../ui/components/rally_card.dart';
import '../ui/components/section_loading.dart';
import '../ui/components/status_tag.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';
import '../ui/ticking_now.dart';

/// ホーム（登録後に最初に開く）。先頭の「次にやること」（`getMyNextActions`。サーバーが返した順のまま）・これからの予約（`listMyBookings`）・今月のビデオ通話の残り（`getMyEntitlement`）・
/// お気に入りのコーチ（`listFavoriteCoaches`）・コーチとのやり取り（`listMessageThreads`）・返ってきた添削（`listMyVideos`）と、コーチを探す画面・添削の依頼・受けたレッスンの入口と、
/// レッスンを受け終えた生徒への次のレッスンの予約の入口（`listMyLessonCompletions`）を 1 画面に並べる。
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
        const _NextActionsSection(),
        const _NextLessonSection(),
        const _BookingsSection(),
        const SizedBox(height: 24),
        const _FavoriteCoachesSection(),
        const _LiveLessonQuotaSection(),
        const SizedBox(height: 24),
        const _ThreadsSection(),
        const SizedBox(height: 24),
        const _ReturnedReviewsSection(),
        const _VideoReviewRequestSection(),
        const SizedBox(height: 24),
        OutlinedButton.icon(
          onPressed: state.busy
              ? null
              : ref.read(lessonCompletionsProvider.notifier).open,
          icon: const Icon(Icons.history),
          label: const Text('受けたレッスン'),
        ),
        const SizedBox(height: 8),
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

/// 次にやること（`getMyNextActions`。ADR 0010）。サーバーが返した順のまま、1 件目をカード（主なボタンはこの 1 つ）、2 件目からを一覧の行で出す
/// （並べ替え・数え直しをしない。ADR 0006）。0 件なら欄ごと出さない。読み込み中・読めなかったときは見出しと旨を出す（ほかの欄はそのまま）
class _NextActionsSection extends ConsumerWidget {
  const _NextActionsSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final state = ref.watch(nextActionsProvider);
    final actions = state.actions;
    if (!state.failed && actions != null && actions.isEmpty) {
      return const SizedBox.shrink();
    }
    final notifier = ref.read(nextActionsProvider.notifier);
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Semantics(
        container: true,
        label: '次にやること',
        explicitChildNodes: true,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const _SectionTitle('次にやること'),
            if (state.failed)
              const Notice(
                message: '次にやることを読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
                status: RallyStatus.danger,
              )
            else if (actions == null)
              const SectionLoading(message: '次にやることを読み込んでいます…')
            else ...[
              _NextActionCard(actions.first, busy: busy),
              for (final action in actions.skip(1))
                ListRow(
                  title: _nextActionButtonLabel(action),
                  subtitle: _nextActionRowSubtitle(action),
                  onTap: busy ? null : () => notifier.open(action),
                ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 次の操作のボタン（2 件目からの行の題も同じ）
String _nextActionButtonLabel(NextAction action) => switch (action.kind) {
  NextActionKind.confirmBooking => '内容を確かめて確定する',
  NextActionKind.joinLiveLesson => 'ビデオ通話で受ける',
  NextActionKind.videoReviewInProgress => '送った動画を見る',
  NextActionKind.recordLessonCompletion => '完了の記録へ進む',
  NextActionKind.startSubscription => '月額契約へ進む',
};

/// 次の操作のコーチ（名前がないときは表示できない旨）
String _nextActionCoachLabel(NextAction action) =>
    'コーチ: ${action.coachDisplayName ?? '（名前を表示できません）'}';

/// 予約の日時。例: `10/28（水）12:00〜12:30`
String? _nextActionRange(NextAction action) =>
    switch ((action.startsAt, action.endsAt)) {
      (final start?, final end?) => formatTimeRange(start, end),
      (final start?, null) => formatDateTime(start),
      _ => null,
    };

/// 2 件目からの行の subtitle（日時かコーチ）
String? _nextActionRowSubtitle(NextAction action) => switch (action.kind) {
  NextActionKind.confirmBooking ||
  NextActionKind.joinLiveLesson => _nextActionRange(action),
  NextActionKind.videoReviewInProgress ||
  NextActionKind.recordLessonCompletion => _nextActionCoachLabel(action),
  NextActionKind.startSubscription => null,
};

/// 次にやることの 1 件目（components.yaml の card の plain。カード全体は押せず、押せるのは中のボタン）
class _NextActionCard extends ConsumerWidget {
  const _NextActionCard(this.action, {required this.busy});

  final NextAction action;
  final bool busy;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final notifier = ref.read(nextActionsProvider.notifier);
    final trial = action.isTrial ?? false;
    final muted = RallyTypography.small.copyWith(color: RallyColors.textMuted);
    Widget title(String text) => Text(text, style: RallyTypography.title);
    // 主なボタンはカードの幅いっぱい
    Widget primary() => SizedBox(
      width: double.infinity,
      child: RallyButton(
        label: _nextActionButtonLabel(action),
        onPressed: busy ? null : () => notifier.open(action),
      ),
    );
    final bookingMeta = [
      if (action.sport case final sport?) sportLabel(sport),
      if (action.lessonMenu case final menu?) bookingLessonMenuLabel(menu),
      _nextActionCoachLabel(action),
    ].join('・');
    final children = switch (action.kind) {
      NextActionKind.confirmBooking => [
        StatusTag(
          label: bookingStatusLabel(BookingStatus.pending),
          status: RallyBookingState.pending.status,
        ),
        title(trial ? '体験予約がまだ確定していません' : '予約がまだ確定していません'),
        if (_nextActionRange(action) case final range?)
          Text(range, style: RallyTypography.figureSm),
        Text(bookingMeta, style: muted),
        Text(
          trial
              ? '確定するまで、このスロットは押さえたままになり、ほかのスロットで体験予約を取り直すこともできません。'
              : '確定するまで、このスロットは押さえたままになります。',
        ),
        primary(),
      ],
      NextActionKind.joinLiveLesson => [
        StatusTag(
          label: bookingStatusLabel(BookingStatus.confirmed),
          status: RallyBookingState.confirmed.status,
        ),
        title(trial ? '体験予約のレッスンを受けましょう' : '次のレッスンを受けましょう'),
        if (_nextActionRange(action) case final range?)
          Text(range, style: RallyTypography.figureSm),
        Text(bookingMeta, style: muted),
        if (action.startsAt case final startsAt?)
          _NextLiveLessonEntry(
            key: ValueKey(startsAt),
            startsAt: startsAt,
            onPressed: busy ? null : () => notifier.open(action),
          ),
        if (trial && action.bookingId != null)
          RallyButton(
            label: '動画を送って添削を受ける',
            variant: RallyButtonVariant.text,
            onPressed: busy
                ? null
                : () => ref
                      .read(studentFlowProvider.notifier)
                      .chooseVideoFor(action.bookingId!),
          ),
      ],
      NextActionKind.videoReviewInProgress => [
        title('コーチが添削しています'),
        Text(
          [
            if (action.submittedAt case final at?)
              '${formatDayClock(at)} に送った動画',
            _nextActionCoachLabel(action),
          ].join('・'),
          style: muted,
        ),
        const Text('添削が返ってくると、ここから完了の記録へ進めます。'),
        // 待つだけの状態なので primary を出さない
        RallyButton(
          label: _nextActionButtonLabel(action),
          variant: RallyButtonVariant.text,
          onPressed: busy ? null : () => notifier.open(action),
        ),
      ],
      NextActionKind.recordLessonCompletion =>
        action.lessonType == LessonType.liveLesson
            ? [
                title('ビデオ通話が終わりました'),
                Text(
                  [
                    ?_nextActionRange(action),
                    _nextActionCoachLabel(action),
                  ].join('・'),
                  style: muted,
                ),
                const Text('レッスンを受け終えたら、完了を記録しましょう。'),
                primary(),
              ]
            : [
                title('添削が返ってきました'),
                Text(
                  [
                    if (action.returnedAt case final at?)
                      '返却: ${formatDayClock(at)}',
                    _nextActionCoachLabel(action),
                  ].join('・'),
                  style: muted,
                ),
                const Text('添削を読んだら、レッスンの完了を記録しましょう。'),
                primary(),
              ],
      NextActionKind.startSubscription => [
        title('月額契約で、続けて教わりましょう'),
        const Text('動画とチャットは送り放題、ビデオ通話は選んだプランの回数まで毎月受けられます。'),
        primary(),
      ],
    };
    return RallyCard(children: children);
  }
}

/// 次にやることの「ビデオ通話で受ける」（開始 5 分前から押せる。入室前チェックの決まり [prejoinCheckLeadTime] のまま）と、その下の案内。
/// 開いたまま 5 分前になったら押せるようにし、読み上げで 1 回伝える
class _NextLiveLessonEntry extends StatefulWidget {
  const _NextLiveLessonEntry({
    super.key,
    required this.startsAt,
    required this.onPressed,
  });

  final DateTime startsAt;
  final VoidCallback? onPressed;

  @override
  State<_NextLiveLessonEntry> createState() => _NextLiveLessonEntryState();
}

class _NextLiveLessonEntryState extends State<_NextLiveLessonEntry> {
  bool? _wasOpen;

  @override
  Widget build(BuildContext context) {
    return TickingNow(
      builder: (context, now) {
        final open = canOpenPrejoinCheck(widget.startsAt, now);
        if (_wasOpen == false && open) {
          final view = View.of(context);
          final direction = Directionality.of(context);
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => SemanticsService.sendAnnouncement(
              view,
              '入室前チェックができるようになりました',
              direction,
            ),
          );
        }
        _wasOpen = open;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          spacing: RallySpacing.sm,
          children: [
            RallyButton(
              label: 'ビデオ通話で受ける',
              onPressed: open ? widget.onPressed : null,
            ),
            Text(
              open
                  ? '入室前チェックでカメラとマイクを確かめてから、開始時刻 ${formatClock(widget.startsAt)} に入室します。'
                  : '${formatDayClock(prejoinCheckOpensAt(widget.startsAt))} から入室前チェックができます'
                        '（開始 ${prejoinCheckLeadTime.inMinutes} 分前から）。',
              style: RallyTypography.small.copyWith(
                color: RallyColors.textMuted,
              ),
            ),
          ],
        );
      },
    );
  }
}

/// 次のレッスンの予約の入口（スロットの選択を開く）。完了したレッスンが 1 件でもあれば出し、
/// まだない・読み込み中・読めなかったときは欄ごと出さない（初回かどうかはアプリで数え直さない。ADR 0006）
class _NextLessonSection extends ConsumerWidget {
  const _NextLessonSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final completions = ref.watch(
      lessonCompletionsProvider.select((s) => s.completions),
    );
    if (completions == null || completions.isEmpty) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _SectionTitle('次のレッスン'),
          const Text('レッスンおつかれさまでした。続けて受けられるよう、次のレッスンも予約しておきましょう。'),
          const SizedBox(height: 8),
          // 主なボタンは「次にやること」の 1 つだけにするので secondary
          RallyButton(
            label: '次のレッスンを予約する',
            variant: RallyButtonVariant.secondary,
            icon: Icons.event_available,
            onPressed: busy
                ? null
                : ref.read(studentFlowProvider.notifier).goToSlotSearch,
          ),
        ],
      ),
    );
  }
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
          // 開始の近い順なので、先頭が次のレッスン（日付のコマをライムにするのは 1 つだけ）
          for (final (index, item) in upcoming.indexed)
            Padding(
              padding: const EdgeInsets.only(bottom: RallySpacing.sm),
              child: _BookingCard(item, next: index == 0),
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
            TextButton(
              onPressed: state.busy
                  ? null
                  : ref.read(coachSearchProvider.notifier).open,
              child: const Text('コーチを探す'),
            ),
          ],
        ),
      ],
    );
  }
}

/// これからの予約の 1 件（components.yaml の card の booking）。押せない（予約の操作は「予約をすべて見る」の先）
class _BookingCard extends StatelessWidget {
  const _BookingCard(this.item, {required this.next});

  final MyBooking item;
  final bool next;

  @override
  Widget build(BuildContext context) {
    final date = dateBlockLabels(item.slot.startsAt);
    return RallyBookingCard(
      month: date.month,
      day: date.day,
      weekday: date.weekday,
      time: formatClockRange(item.slot.startsAt, item.slot.endsAt),
      meta: [
        sportLabel(item.slot.sport),
        if (item.booking.lessonMenu case final menu?)
          bookingLessonMenuLabel(menu),
        if (item.slot.coachDisplayName case final name?) 'コーチ: $name',
      ].join('・'),
      status: StatusTag(
        label: bookingStatusLabel(item.booking.status),
        status: _bookingState(item.booking.status).status,
      ),
      next: next,
    );
  }
}

RallyBookingState _bookingState(BookingStatus status) => switch (status) {
  BookingStatus.pending => RallyBookingState.pending,
  BookingStatus.confirmed => RallyBookingState.confirmed,
  BookingStatus.cancelled => RallyBookingState.cancelled,
};

/// お気に入りのコーチ（お気に入りに入れた時刻の新しい順にすべて。押すとそのコーチの紹介を開き、戻るとホームに戻る）。
/// 0 件なら欄ごと出さない（読み込み中と読めなかったときは見出しと旨を出す）
class _FavoriteCoachesSection extends ConsumerWidget {
  const _FavoriteCoachesSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final failed = ref.watch(
      studentFlowProvider.select(
        (s) => s.homeFailures.contains(HomeSection.favoriteCoaches),
      ),
    );
    final coaches = ref.watch(
      favoriteCoachesProvider.select((s) => s.favoriteCoaches),
    );
    final notifier = ref.read(favoriteCoachesProvider.notifier);
    if (!failed && coaches != null && coaches.isEmpty) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _SectionTitle('お気に入りのコーチ'),
          if (failed)
            const _SectionFailure('お気に入りのコーチを読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
          else if (coaches == null)
            const Text('お気に入りのコーチを読み込んでいます…')
          else
            // 契約どおりお気に入りに入れた時刻の新しい順で返るので、返った順のまま出す
            for (final coach in coaches)
              Card(
                child: ListTile(
                  title: Text(coachNameLabel(coach.displayName)),
                  subtitle: Text(favoriteCoachSummaryLabel(coach)),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: busy ? null : () => notifier.open(coach),
                ),
              ),
        ],
      ),
    );
  }
}

/// 今月のビデオ通話の残り（月額契約のプランで決まる上限のうち）。受講権がない（`getMyEntitlement` が 404）ときは、
/// その旨と文字のボタン「月額契約へ進む」（月額契約の画面へ。主なボタンは「次にやること」の 1 つだけにするので text）
class _LiveLessonQuotaSection extends ConsumerWidget {
  const _LiveLessonQuotaSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final failed = ref.watch(
      studentFlowProvider.select(
        (s) => s.homeFailures.contains(HomeSection.entitlement),
      ),
    );
    final state = ref.watch(subscriptionProvider);
    final entitlement = state.entitlement;
    final quota = entitlement?.liveLessonQuota;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _SectionTitle('今月のビデオ通話'),
        if (failed)
          const _SectionFailure('ビデオ通話の残りの回数を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。')
        else if (state.noEntitlement) ...[
          const Text('まだ月額契約はありません。月額契約を始めると、毎月のビデオ通話の残りの回数がここに出ます。'),
          Align(
            alignment: Alignment.centerLeft,
            child: RallyButton(
              label: '月額契約へ進む',
              variant: RallyButtonVariant.text,
              onPressed: busy
                  ? null
                  : ref.read(studentFlowProvider.notifier).goToSubscription,
            ),
          ),
        ] else if (quota != null) ...[
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
    final allThreads = ref.watch(chatProvider.select((s) => s.threads));
    final chat = ref.read(chatProvider.notifier);
    final threads = chat.recentThreads();
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
        else if (allThreads != null && threads.isEmpty)
          const Text('まだ新しいやり取りはありません。')
        else
          for (final thread in threads)
            Card(
              child: ListTile(
                title: Text(coachNameLabel(coachNames[thread.coachId])),
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
                onTap: state.busy ? null : () => chat.openThread(thread),
              ),
            ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: state.busy ? null : chat.openChat,
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
                  title: Text(coachNameLabel(video.coachDisplayName)),
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
                      : () => notifier.openMyVideoReview(video.videoId),
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
    final threads =
        ref.watch(chatProvider.select((s) => s.threads)) ??
        const <MessageThread>[];
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
        // 主なボタンは「次にやること」の 1 つだけにするので secondary
        RallyButton(
          label: '添削を依頼する',
          variant: RallyButtonVariant.secondary,
          icon: Icons.videocam_outlined,
          onPressed: state.busy || !available
              ? null
              : ref.read(chatProvider.notifier).startVideoReviewRequest,
        ),
      ],
    );
  }
}
