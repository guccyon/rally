import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 担当コーチとのチャットの状態（スレッドの一覧・開いているスレッドのメッセージ・スレッドに返却が流れた添削）。
/// 画面の段（[FlowStep]）と、API を呼んでいる間の印・直前の操作の失敗の文言はアプリ全体で 1 つなので、
/// [StudentFlowState] の `step` / `busy` / `errorMessage` を使う。
/// 一覧をコンストラクタで写して持つので const では作らない。
class ChatState {
  ChatState({
    this.returnStep,
    this.promptVideoReviewRequest = false,
    List<MessageThread>? threads,
    this.thread,
    List<Message>? messages,
    Map<String, VideoReview> reviews = const {},
    Set<String> reviewFailures = const {},
    // 渡した元の一覧を後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : threads = threads == null ? null : List.unmodifiable(threads),
       messages = messages == null ? null : List.unmodifiable(messages),
       reviews = Map.unmodifiable(reviews),
       reviewFailures = Set.unmodifiable(reviewFailures);

  /// チャットを閉じたときに戻る段（チャットを開いた段）
  final FlowStep? returnStep;

  /// スレッドを開いたら、すぐに添削の依頼（動画の選択）を出すか（ホームの「添削を依頼する」から開いたとき）
  final bool promptVideoReviewRequest;

  /// 担当コーチとのスレッド（`listMessageThreads`。未取得なら null）
  final List<MessageThread>? threads;

  /// 開いているスレッド
  final MessageThread? thread;

  /// 開いているスレッドのメッセージ（送った順）
  final List<Message>? messages;

  /// 読んだスレッドの未読の数の合計（ホームのアプリバーの「チャット」のしるし。未取得なら 0）
  int get unreadCount => (threads ?? const <MessageThread>[]).fold(
    0,
    (sum, thread) => sum + thread.unreadCount,
  );

  /// スレッドに返却が流れた動画の添削（`getVideoReview`。動画の ID ごと）。返却された添削は変わらないので、読めたものは読み直さない
  final Map<String, VideoReview> reviews;

  /// スレッドに返却が流れたが添削を読めなかった動画の ID（その返却のカードにだけ読めなかった旨を出し、次に確かめたときに読み直す）
  final Set<String> reviewFailures;

  ChatState copyWith({
    FlowStep? returnStep,
    bool? promptVideoReviewRequest,
    List<MessageThread>? threads,
    MessageThread? thread,
    List<Message>? messages,
    Map<String, VideoReview>? reviews,
    Set<String>? reviewFailures,
  }) => ChatState(
    returnStep: returnStep ?? this.returnStep,
    promptVideoReviewRequest:
        promptVideoReviewRequest ?? this.promptVideoReviewRequest,
    threads: threads ?? this.threads,
    thread: thread ?? this.thread,
    messages: messages ?? this.messages,
    reviews: reviews ?? this.reviews,
    reviewFailures: reviewFailures ?? this.reviewFailures,
  );
}

final chatProvider = NotifierProvider<ChatNotifier, ChatState>(
  ChatNotifier.new,
);

/// チャットの操作。チャットの API を呼ぶのはここだけ。段の移動と busy・失敗の文言は [StudentFlowNotifier] に任せる
/// （[StudentFlowNotifier.run] で 1 つずつ走らせる）。
class ChatNotifier extends Notifier<ChatState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  ChatNotifier([ChatState? initial]) : _initial = initial ?? ChatState();

  final ChatState _initial;

  @override
  ChatState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  StudentFlowNotifier get _flow => ref.read(studentFlowProvider.notifier);

  FlowStep get _step => ref.read(studentFlowProvider).step;

  Future<void> _run(Future<void> Function() action) => _flow.run(action);

  /// チャットを開く（`listMessageThreads`）。閉じたら今の段に戻る。
  Future<void> openChat() {
    if (!_step.isChat) {
      state = state.copyWith(returnStep: _step);
      _flow.moveTo(FlowStep.messageThreads);
    }
    return _run(loadThreads);
  }

  /// チャットを閉じて、開いた段に戻る
  void closeChat() => _flow.moveTo(state.returnStep ?? FlowStep.slotSearch);

  /// スレッドを開く。相手のメッセージを `markMessageThreadRead` で既読にしてから `listMessages` で読む
  /// （一覧を読むだけでは既読にならない契約なので、画面に出したら既読にする）。
  /// 既読が先なので、既読にしたメッセージは必ず一覧に入る（間に届いたものは未読のまま出て、次に確かめたときに既読になる）。
  /// 既読にできなくてもメッセージを読めれば開く（未読の数は残し、次に確かめたときにもう一度既読にする）。
  /// チャットの外（ホーム）から開いたら、チャットを閉じたときに開いた段に戻る。
  Future<void> openThread(MessageThread thread) {
    if (!_step.isChat) {
      state = state.copyWith(returnStep: _step);
    }
    return _run(() async {
      await _loadMessages(thread);
      _flow.moveTo(FlowStep.messages);
    });
  }

  /// ホームに出すコーチとのやり取り: 未読のあるスレッドと、最後のメッセージが添削の依頼か返却のスレッドを、最後のメッセージの新しい順に
  List<MessageThread> recentThreads() {
    bool notable(MessageThread thread) =>
        thread.unreadCount > 0 ||
        switch (thread.lastMessage?.kind) {
          MessageKind.videoReviewRequest ||
          MessageKind.videoReviewReturned => true,
          _ => false,
        };
    DateTime lastAt(MessageThread thread) =>
        thread.lastMessage?.sentAt ?? thread.openedAt;
    return [
      for (final thread in state.threads ?? const <MessageThread>[])
        if (notable(thread)) thread,
    ]..sort((a, b) => lastAt(b).compareTo(lastAt(a)));
  }

  /// ホームの「添削を依頼する」。スレッドが 1 つならそのスレッドを開いてすぐに添削の依頼を出し、
  /// 2 つ以上ならスレッドの一覧を開いて担当コーチを選ばせる（スレッドがなければ呼ばない）
  Future<void> startVideoReviewRequest() async {
    final threads = state.threads ?? const <MessageThread>[];
    if (threads.length != 1) return openChat();
    state = state.copyWith(promptVideoReviewRequest: true);
    await openThread(threads.single);
    if (ref.mounted && _step != FlowStep.messages) {
      state = state.copyWith(promptVideoReviewRequest: false);
    }
  }

  /// スレッドを開いたときの添削の依頼を出したら、二度は出さない
  void clearVideoReviewRequestPrompt() =>
      state = state.copyWith(promptVideoReviewRequest: false);

  /// 開いているスレッドの新しいメッセージを確かめる（既読にしてから読み直す。順は [openThread] と同じ）
  Future<void> refreshMessages() => _run(() => _loadMessages(state.thread!));

  /// [thread] を既読にしてからメッセージを読む。既読にできたら、読んだスレッドの一覧のそのスレッドも既読の後のものに置き換える
  /// （一覧に戻ったときに読み直せなくても未読の数が消える）。既読の失敗は投げずに飲み込み、失敗の文言も出さない
  /// （メッセージは読めているので画面を開いたままにする）。メッセージを読めなければ投げる
  Future<void> _loadMessages(MessageThread thread) async {
    MessageThread? read;
    try {
      read = await _api.markMessageThreadRead(thread.threadId);
    } on StudentApiException {
      // 既読にできなくても開く（未読のまま残し、次に確かめたときにもう一度既読にする）
    }
    final list = await _api.listMessages(thread.threadId);
    state = state.copyWith(
      thread: read ?? thread,
      messages: list.messages,
      threads: read == null ? null : _replaced(read),
    );
    await _loadReviews();
  }

  /// 読んだスレッドの一覧の [thread] と同じスレッドを置き換えた一覧（未取得なら null で、一覧は変えない）
  List<MessageThread>? _replaced(MessageThread thread) => state.threads
      ?.map((t) => t.threadId == thread.threadId ? thread : t)
      .toList();

  /// スレッドに返却が流れた動画の添削（`getVideoReview`）を、まだ読めていないものだけ読む。
  /// 読めなくてもメッセージの一覧は出す（その動画を [ChatState.reviewFailures] に入れるだけで、失敗の文言は出さない）
  Future<void> _loadReviews() async {
    final videoIds = {
      for (final message in state.messages ?? const <Message>[])
        if (message.kind == MessageKind.videoReviewReturned) ?message.videoId,
    }.difference(state.reviews.keys.toSet());
    if (videoIds.isEmpty) return;
    final reviews = <String, VideoReview>{};
    final failures = <String>{};
    Future<void> load(String videoId) async {
      try {
        reviews[videoId] = await _api.getVideoReview(videoId);
      } on StudentApiException {
        failures.add(videoId);
      }
    }

    await Future.wait(videoIds.map(load));
    if (!ref.mounted) return;
    state = state.copyWith(
      reviews: {...state.reviews, ...reviews},
      reviewFailures: state.reviewFailures.difference(videoIds).union(failures),
    );
  }

  /// `sendMessage`。送れたら true（月額契約の前は 403 `chat_not_entitled` で false）。
  Future<bool> sendMessage(String body) =>
      _sendMessage(SendMessageRequest(body: body));

  /// 本文の代わりにスタンプを送る（`sendMessage` の `stamp_id`）。受講権の扱いは本文と同じ。送れたら true。
  Future<bool> sendStamp(StampId stamp) =>
      _sendMessage(SendMessageRequest.stamp(stamp));

  Future<bool> _sendMessage(SendMessageRequest request) async {
    var sent = false;
    await _run(() async {
      final message = await _api.sendMessage(state.thread!.threadId, request);
      state = state.copyWith(messages: [...?state.messages, message]);
      sent = true;
    });
    return sent;
  }

  /// チャットから動画の添削を依頼する（`submitVideo`。一周シナリオの動画の送付と同じ API）。送り先はスレッドの担当コーチで、
  /// スポーツは生徒のもの。動画本体のアップロードは Gen1 ではモックなので行わない。添削依頼はサーバーがスレッドに流すので、
  /// 送れたらスレッドを読み直す。[lessonMenuId] は担当コーチの非同期のメニュー（[StudentFlowState.asyncLessonMenusOf]）で、
  /// 省けばメニューなし。メニューを付けて 422 が返ったら、失敗の文言は出さずにメニューを読み直す
  /// （[StudentFlowNotifier.submitVideoForReview]）。
  Future<VideoReviewRequestResult> requestVideoReview({
    required VideoContentType contentType,
    String? note,
    String? lessonMenuId,
  }) async {
    var result = VideoReviewRequestResult.failed;
    await _run(() async {
      final thread = state.thread!;
      final video = await _flow.submitVideoForReview(
        SubmitVideoRequest(
          coachId: thread.coachId,
          sport: ref.read(studentFlowProvider).student!.sport,
          contentType: contentType,
          note: note,
          lessonMenuId: lessonMenuId,
        ),
      );
      if (video == null) {
        result = VideoReviewRequestResult.menuRejected;
        return;
      }
      result = VideoReviewRequestResult.sent;
      await _loadMessages(thread);
    });
    return result;
  }

  /// スレッドの一覧を読み直す
  Future<void> refreshThreads() => _run(loadThreads);

  /// スレッドの一覧に戻る（未読の数と最後のメッセージを読み直す）
  Future<void> backToThreads() {
    _flow.moveTo(FlowStep.messageThreads);
    return _run(loadThreads);
  }

  /// スレッドの一覧（`listMessageThreads`）を読む。busy にせず、失敗はそのまま投げる（ホームの欄の読み込みからも呼ぶ）
  Future<void> loadThreads() async {
    final list = await _api.listMessageThreads();
    state = state.copyWith(threads: list.threads);
  }
}
