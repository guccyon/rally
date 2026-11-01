import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'chat.dart';
import 'student_flow.dart';

/// お知らせ（glossary の notification）の状態。画面の段（[FlowStep]）と、API を呼んでいる間の印・直前の操作の失敗の文言は
/// アプリ全体で 1 つなので、[StudentFlowState] の `step` / `busy` / `errorMessage` を使う。
/// 一覧をコンストラクタで写して持つので const では作らない。
class NotificationsState {
  NotificationsState({
    List<ApiNotification>? notifications,
    this.failed = false,
    Set<String> newIds = const {},
    this.returnStep,
    // 渡した元を後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : notifications = notifications == null
           ? null
           : List.unmodifiable(notifications),
       newIds = Set.unmodifiable(newIds);

  /// 生徒のお知らせ（作った時刻の新しい順。サーバーの既読のまま）。読み込む前・読み込み中・読めなかったときは null
  final List<ApiNotification>? notifications;

  /// お知らせの画面で一覧を読めなかったら true（画面にその旨と読み直しのボタンを出す）
  final bool failed;

  /// お知らせの画面を開いたときに未読だったお知らせの ID（既読にしたあとも、画面では「新着」として見分ける）
  final Set<String> newIds;

  /// お知らせの画面を閉じたときに戻る段（画面を開いた段）
  final FlowStep? returnStep;

  /// 未読のお知らせがあるか（ホームのしるし）。読めていなければ false
  bool get hasUnread =>
      (notifications ?? const <ApiNotification>[]).any((n) => !n.isRead);
}

final notificationsProvider =
    NotifierProvider<NotificationsNotifier, NotificationsState>(
      NotificationsNotifier.new,
    );

/// お知らせの操作。`listMyNotifications` / `markMyNotificationsRead` を呼ぶのはここだけ。段の移動と busy は
/// [StudentFlowNotifier] に任せる（[StudentFlowNotifier.moveTo] と [StudentFlowNotifier.run]）。
class NotificationsNotifier extends Notifier<NotificationsState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  NotificationsNotifier([NotificationsState? initial])
    : _initial = initial ?? NotificationsState();

  final NotificationsState _initial;

  @override
  NotificationsState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  StudentFlowNotifier get _flow => ref.read(studentFlowProvider.notifier);

  /// ホームのしるしのために一覧を読む（[StudentFlowNotifier] がホームの欄と並べて呼ぶ）。
  /// 読めなければしるしを出さないだけで、失敗の文言は出さない
  Future<void> loadForHome() async {
    try {
      final list = await _api.listMyNotifications();
      if (!ref.mounted) return;
      state = NotificationsState(
        notifications: list.notifications,
        returnStep: state.returnStep,
      );
    } on StudentApiException {
      if (!ref.mounted) return;
      state = NotificationsState(returnStep: state.returnStep);
    }
  }

  /// お知らせの画面を開き、一覧を読んでからすべて既読にする（開くたびに読み直す）。閉じたら今の段に戻る
  Future<void> open() {
    final step = ref.read(studentFlowProvider).step;
    if (step != FlowStep.notifications) {
      state = NotificationsState(
        notifications: state.notifications,
        returnStep: step,
      );
      _flow.moveTo(FlowStep.notifications);
    }
    return refresh();
  }

  /// 一覧を読み直し、すべて既読にする。読めなければ [NotificationsState.failed] を立て、画面にその旨だけを出す
  /// （失敗の文言は出さない）。既読にできなくても一覧は出し（失敗の文言は出さない）、未読のまま残して
  /// 次に開いたときにもう一度既読にする
  Future<void> refresh() => _flow.run(() async {
    final returnStep = state.returnStep;
    state = NotificationsState(returnStep: returnStep);
    final List<ApiNotification> listed;
    try {
      listed = (await _api.listMyNotifications()).notifications;
    } on StudentApiException {
      if (!ref.mounted) return;
      state = NotificationsState(failed: true, returnStep: returnStep);
      return;
    }
    if (!ref.mounted) return;
    final newIds = {
      for (final n in listed)
        if (!n.isRead) n.notificationId,
    };
    state = NotificationsState(
      notifications: listed,
      newIds: newIds,
      returnStep: returnStep,
    );
    if (newIds.isEmpty) return;
    try {
      final read = await _api.markMyNotificationsRead();
      if (!ref.mounted) return;
      state = NotificationsState(
        notifications: read.notifications,
        // 読んだあとに届いたお知らせも、既読にした一覧に入れば新着として見分ける
        newIds: {
          ...newIds,
          for (final n in read.notifications)
            if (!listed.any((l) => l.notificationId == n.notificationId))
              n.notificationId,
        },
        returnStep: returnStep,
      );
    } on StudentApiException {
      // 既読にできなくても一覧はそのまま出す（未読のまま残る）
    }
  });

  /// お知らせの画面を閉じて、開いた段に戻る
  void close() => _flow.moveTo(state.returnStep ?? FlowStep.home);

  /// [notification] の行から進めるか（本部からのお知らせは本文がすべてで行き先がなく、このアプリの版が知らない種類も
  /// 進めない）
  bool canOpen(ApiNotification notification) => switch (notification.kind) {
    NotificationKind.bookingConfirmed ||
    NotificationKind.videoReviewReturned ||
    NotificationKind.messageReceived => true,
    NotificationKind.announcement || null => false,
  };

  /// 行を押したとき。お知らせは行き先の ID を持たないので、種類に応じた一覧の画面を開く
  /// （予約の確定は自分の予約・添削の返却は自分の動画・メッセージの受信はチャットのスレッドの一覧）。
  /// 開いた画面を閉じるとお知らせの画面に戻る
  Future<void> select(ApiNotification notification) async {
    switch (notification.kind) {
      case NotificationKind.bookingConfirmed:
        await _flow.openMyBookings();
      case NotificationKind.videoReviewReturned:
        await _flow.openMyVideos();
      case NotificationKind.messageReceived:
        await ref.read(chatProvider.notifier).openChat();
      case NotificationKind.announcement || null:
        return;
    }
  }
}
