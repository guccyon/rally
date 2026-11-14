import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 月額契約と受講権の状態。契約の画面とホームの「今月のビデオ通話」の欄が読む。
/// 契約するスポーツ（[StudentFlowState.student]）と、画面の段（[FlowStep]）・
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
    this.entitlementFailed = false,
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

  /// プロフィールで受講権（`getMyEntitlement`）を読めなかったら true（月額契約の欄にだけその旨を出す。ホームの欄は
  /// [StudentFlowState.homeFailures] のまま）
  final bool entitlementFailed;

  /// 月額契約が始まっているか（この端末で始めた契約があるか、読んだ受講権が月額）。別のタブや端末で始めた契約も
  /// 受講権を読み直せばここに出る（ADR 0010）。契約の画面はこれでプランの欄と「月額契約を始める」を消す
  bool get hasActiveSubscription =>
      subscription != null ||
      (entitlement?.contractType == ContractType.subscription &&
          entitlement?.plan != null);

  /// 受講権を読んでいる途中か（まだ読めておらず、契約の前とも読めなかったともわかっていない）
  bool get entitlementLoading =>
      entitlement == null && !noEntitlement && !entitlementFailed;

  SubscriptionState copyWith({
    List<SubscriptionPlanDetail>? subscriptionPlans,
    bool? subscriptionPlansFailed,
    Subscription? subscription,
    Entitlement? entitlement,
    bool? noEntitlement,
    bool? entitlementFailed,
  }) => SubscriptionState(
    subscriptionPlans: subscriptionPlans ?? this.subscriptionPlans,
    subscriptionPlansFailed:
        subscriptionPlansFailed ?? this.subscriptionPlansFailed,
    subscription: subscription ?? this.subscription,
    entitlement: entitlement ?? this.entitlement,
    noEntitlement: noEntitlement ?? this.noEntitlement,
    entitlementFailed: entitlementFailed ?? this.entitlementFailed,
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

  /// 受講権を読み直す（[loadEntitlement]）。プロフィールと契約の画面を開いたとき、アプリやタブが前面に戻ったとき
  /// （[StudentFlowNotifier.refreshEntitlementOnResume]）に呼ぶ。読めなければ [SubscriptionState.entitlementFailed] を
  /// 立てるだけで投げない（ほかの欄はそのまま使え、失敗の文言も出さない）
  Future<void> refreshEntitlement() async {
    state = state.copyWith(entitlementFailed: false);
    try {
      await loadEntitlement();
    } on StudentApiException {
      if (!ref.mounted) return;
      state = state.copyWith(entitlementFailed: true);
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
  /// 409 `subscription_already_active`（別のタブや端末で契約済み）なら、押したプランではなくサーバーの受講権を読み直して
  /// 今の契約を表示の元にする（ADR 0010）。失敗の文言（「すでに始まっています」）はそのまま出す
  Future<void> startSubscription({
    SubscriptionPlan plan = SubscriptionPlan.standard,
  }) => ref.read(studentFlowProvider.notifier).run(() async {
    final StartSubscriptionResponse response;
    try {
      response = await _api.startSubscription(
        StartSubscriptionRequest(
          sport: ref.read(studentFlowProvider).student!.sport,
          plan: plan,
          paymentMethodToken: mockPaymentMethodToken,
        ),
      );
    } on StudentApiException catch (e) {
      if (e.error?.code == ErrorCode.subscriptionAlreadyActive) {
        try {
          await loadEntitlement();
        } on StudentApiException {
          // 読み直せなくても、元の失敗の文言を出す
        }
      }
      rethrow;
    }
    state = state.copyWith(
      subscription: response.subscription,
      entitlement: response.entitlement,
      noEntitlement: false,
      entitlementFailed: false,
    );
  });
}

/// 決済（Stripe 相当）のモックのトークン。サーバーは受け取るだけで使わない。
const mockPaymentMethodToken = 'mock-payment-method';
