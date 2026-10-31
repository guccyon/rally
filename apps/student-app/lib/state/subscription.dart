import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 月額契約と受講権の状態。契約の画面とホームの「今月のビデオ通話」の欄、キャンセルの確認
/// （[StudentFlowNotifier.cancellationUsesQuota]）が読む。契約するスポーツ（[StudentFlowState.student]）と、画面の段（[FlowStep]）・
/// API を呼んでいる間の印・直前の操作の失敗の文言・ホームで読めなかった欄はアプリ全体で 1 つなので、
/// [StudentFlowState] の `student` / `step` / `busy` / `errorMessage` / `homeFailures` を使う。
/// 一覧をコンストラクタで写して持つので const では作らない。
class SubscriptionState {
  SubscriptionState({
    List<SubscriptionPlanDetail>? subscriptionPlans,
    this.subscriptionPlansFailed = false,
    this.subscription,
    this.entitlement,
    this.noEntitlement = false,
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : subscriptionPlans = subscriptionPlans == null
           ? null
           : List.unmodifiable(subscriptionPlans);

  /// 契約プランとビデオ通話の暦月の上限（`listSubscriptionPlans`。契約の画面を開いたときに読む。未取得なら null）。
  /// コンストラクタでも copyWith でも、渡したリストの読み取り専用の写しを持つ（Riverpod を通さずに状態を変えさせない）
  final List<SubscriptionPlanDetail>? subscriptionPlans;

  /// 契約プランの一覧を読めなかったら true（契約の画面はプランの名前だけで選ばせ、プランの欄にだけその旨を出す）
  final bool subscriptionPlansFailed;

  /// この端末で始めた月額契約（`startSubscription` の応答）
  final Subscription? subscription;

  /// 受講権（`getMyEntitlement`、または `startSubscription` の応答）
  final Entitlement? entitlement;

  /// `getMyEntitlement` が 404（受講権がまだない＝月額契約の前）を返したら true
  final bool noEntitlement;

  SubscriptionState copyWith({
    List<SubscriptionPlanDetail>? subscriptionPlans,
    bool? subscriptionPlansFailed,
    Subscription? subscription,
    Entitlement? entitlement,
    bool? noEntitlement,
  }) => SubscriptionState(
    subscriptionPlans: subscriptionPlans ?? this.subscriptionPlans,
    subscriptionPlansFailed:
        subscriptionPlansFailed ?? this.subscriptionPlansFailed,
    subscription: subscription ?? this.subscription,
    entitlement: entitlement ?? this.entitlement,
    noEntitlement: noEntitlement ?? this.noEntitlement,
  );
}

final subscriptionProvider =
    NotifierProvider<SubscriptionNotifier, SubscriptionState>(
      SubscriptionNotifier.new,
    );

/// 月額契約と受講権の操作。`listSubscriptionPlans`・`startSubscription`・`getMyEntitlement` を呼ぶのはここだけ。
/// busy・失敗の文言は [StudentFlowNotifier] に任せる（[StudentFlowNotifier.run] で 1 つずつ走らせる）。
class SubscriptionNotifier extends Notifier<SubscriptionState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  SubscriptionNotifier([SubscriptionState? initial])
    : _initial = initial ?? SubscriptionState();

  final SubscriptionState _initial;

  @override
  SubscriptionState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  /// 受講権（`getMyEntitlement`）を読む。ホームの欄を読むとき（[StudentFlowNotifier] の中）に呼ぶ。
  /// 404 は失敗ではなく、月額契約の前（[SubscriptionState.noEntitlement]）。ほかの失敗は投げ直す（呼ぶ側がその欄にだけ旨を出す）
  Future<void> loadEntitlement() async {
    try {
      final entitlement = await _api.getMyEntitlement();
      state = state.copyWith(entitlement: entitlement, noEntitlement: false);
    } on StudentApiException catch (e) {
      if (e.statusCode != 404) rethrow;
      state = state.copyWith(noEntitlement: true);
    }
  }

  /// 契約プランとビデオ通話の暦月の上限（`listSubscriptionPlans`）を読む。契約の画面を開いたときに呼ぶ。
  /// 読めなくてもプランは名前だけで選べるので、busy にせず失敗の文言も出さない
  /// （[SubscriptionState.subscriptionPlansFailed] を立て、プランの欄にだけその旨を出す）。
  Future<void> loadSubscriptionPlans() async {
    state = state.copyWith(subscriptionPlansFailed: false);
    try {
      final list = await _api.listSubscriptionPlans();
      if (!ref.mounted) return;
      state = state.copyWith(subscriptionPlans: list.plans);
    } on StudentApiException {
      if (!ref.mounted) return;
      state = state.copyWith(subscriptionPlansFailed: true);
    }
  }

  /// `startSubscription`。[plan] は選んだ契約プラン（既定は standard）。スポーツは生徒のもの（[StudentFlowState.student]）。
  /// 決済は Gen1 ではモックなので、決済手段のトークンも固定のモック。
  Future<void> startSubscription({
    SubscriptionPlan plan = SubscriptionPlan.standard,
  }) => ref.read(studentFlowProvider.notifier).run(() async {
    final response = await _api.startSubscription(
      StartSubscriptionRequest(
        sport: ref.read(studentFlowProvider).student!.sport,
        plan: plan,
        paymentMethodToken: mockPaymentMethodToken,
      ),
    );
    state = state.copyWith(
      subscription: response.subscription,
      entitlement: response.entitlement,
      noEntitlement: false,
    );
  });
}

/// 決済（Stripe 相当）のモックのトークン。サーバーは受け取るだけで使わない。
const mockPaymentMethodToken = 'mock-payment-method';
