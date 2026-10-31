import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';

import '../support/fake_student_api.dart';

void main() {
  final onSubscription = StudentFlowState(
    step: FlowStep.subscription,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  /// API をモックにした ProviderContainer（画面は描かない）
  ProviderContainer containerOf(
    FakeStudentApi api, {
    StudentFlowState? flow,
    SubscriptionState? subscription,
  }) {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        studentFlowProvider.overrideWith(
          () => StudentFlowNotifier(flow ?? onSubscription),
        ),
        subscriptionProvider.overrideWith(
          () => SubscriptionNotifier(subscription),
        ),
        clockProvider.overrideWithValue(() => Fixtures.now),
      ],
    );
    addTearDown(container.dispose);
    return container;
  }

  test('契約プランの一覧を読むと、応答の上限を持つ（busy にしない）', () async {
    final api = FakeStudentApi();
    final container = containerOf(api);

    await container.read(subscriptionProvider.notifier).loadSubscriptionPlans();

    expect(api.calls, ['listSubscriptionPlans']);
    final state = container.read(subscriptionProvider);
    expect(state.subscriptionPlans, Fixtures.subscriptionPlans);
    expect(state.subscriptionPlansFailed, isFalse);
    expect(container.read(studentFlowProvider).busy, isFalse);
  });

  test('契約プランの一覧を読めなければ印を立て、失敗の文言は出さず直前の文言も消さない', () async {
    final api = FakeStudentApi(
      failures: {'listSubscriptionPlans': Fixtures.unreachable},
    );
    final container = containerOf(
      api,
      flow: onSubscription.copyWith(errorMessage: '直前の失敗'),
    );

    await container.read(subscriptionProvider.notifier).loadSubscriptionPlans();

    final state = container.read(subscriptionProvider);
    expect(state.subscriptionPlans, isNull);
    expect(state.subscriptionPlansFailed, isTrue);
    expect(container.read(studentFlowProvider).errorMessage, '直前の失敗');
  });

  test('読み直して読めたら、読めなかった印を下ろす', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      subscription: SubscriptionState(subscriptionPlansFailed: true),
    );

    await container.read(subscriptionProvider.notifier).loadSubscriptionPlans();

    final state = container.read(subscriptionProvider);
    expect(state.subscriptionPlansFailed, isFalse);
    expect(state.subscriptionPlans, Fixtures.subscriptionPlans);
  });

  test('読んだプランの一覧は読み取り専用の写しで持ち、外から変えられない', () async {
    final container = containerOf(FakeStudentApi());

    await container.read(subscriptionProvider.notifier).loadSubscriptionPlans();

    final state = container.read(subscriptionProvider);
    expect(
      () => state.subscriptionPlans!.add(Fixtures.subscriptionPlans.first),
      throwsUnsupportedError,
    );
    expect(
      () => state.copyWith().subscriptionPlans!.clear(),
      throwsUnsupportedError,
    );
  });

  test('渡した元のリストを後から変えても、状態のプランの一覧は変わらない', () {
    final source = [Fixtures.subscriptionPlans.first];
    final state = SubscriptionState(subscriptionPlans: source);
    final copied = state.copyWith(subscriptionPlans: source);

    source.add(Fixtures.subscriptionPlans.last);

    expect(state.subscriptionPlans, [Fixtures.subscriptionPlans.first]);
    expect(copied.subscriptionPlans, [Fixtures.subscriptionPlans.first]);
    expect(
      () => state.subscriptionPlans!.add(Fixtures.subscriptionPlans.last),
      throwsUnsupportedError,
    );
    expect(
      () => copied.subscriptionPlans!.add(Fixtures.subscriptionPlans.last),
      throwsUnsupportedError,
    );
  });

  test('月額契約を始めると生徒のスポーツと選んだプランで startSubscription を呼び、契約と受講権を持つ', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      subscription: SubscriptionState(noEntitlement: true),
    );

    await container
        .read(subscriptionProvider.notifier)
        .startSubscription(plan: SubscriptionPlan.plus);

    expect(api.calls, ['startSubscription']);
    expect((api.requests.single as StartSubscriptionRequest).toJson(), {
      'sport': 'tennis',
      'plan': 'plus',
      'payment_method_token': mockPaymentMethodToken,
    });
    final state = container.read(subscriptionProvider);
    expect(state.subscription?.plan, SubscriptionPlan.plus);
    expect(state.entitlement?.liveLessonQuota?.limit, 4);
    expect(state.noEntitlement, isFalse);
    final flow = container.read(studentFlowProvider);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, isNull);
    expect(flow.step, FlowStep.subscription);
  });

  test('プランを渡さなければ standard で始める', () async {
    final api = FakeStudentApi();
    final container = containerOf(api);

    await container.read(subscriptionProvider.notifier).startSubscription();

    expect(
      (api.requests.single as StartSubscriptionRequest).plan,
      SubscriptionPlan.standard,
    );
    expect(
      container.read(subscriptionProvider).subscription?.plan,
      SubscriptionPlan.standard,
    );
  });

  test('月額契約を始められなければ契約を持たず、失敗の文言を出す', () async {
    final api = FakeStudentApi(
      failures: {'startSubscription': Fixtures.unreachable},
    );
    final container = containerOf(api);

    await container.read(subscriptionProvider.notifier).startSubscription();

    final state = container.read(subscriptionProvider);
    expect(state.subscription, isNull);
    expect(state.entitlement, isNull);
    final flow = container.read(studentFlowProvider);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, 'サーバーにつながりませんでした。通信状態を確かめてください。');
  });

  test('ほかの操作が API を呼んでいる間（busy）は月額契約を始めない', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: onSubscription.copyWith(busy: true),
    );

    await container.read(subscriptionProvider.notifier).startSubscription();

    expect(api.calls, isEmpty);
    expect(container.read(subscriptionProvider).subscription, isNull);
  });

  test('受講権を読むと持ち、月額契約の前の印を下ろす', () async {
    final entitlement = Fixtures.subscriptionStarted.entitlement;
    final api = FakeStudentApi(entitlement: entitlement);
    final container = containerOf(
      api,
      subscription: SubscriptionState(noEntitlement: true),
    );

    await container.read(subscriptionProvider.notifier).loadEntitlement();

    expect(api.calls, ['getMyEntitlement']);
    final state = container.read(subscriptionProvider);
    expect(state.entitlement, entitlement);
    expect(state.noEntitlement, isFalse);
  });

  test('受講権の 404 は失敗ではなく、月額契約の前の印を立てる', () async {
    final container = containerOf(FakeStudentApi());

    await container.read(subscriptionProvider.notifier).loadEntitlement();

    final state = container.read(subscriptionProvider);
    expect(state.entitlement, isNull);
    expect(state.noEntitlement, isTrue);
  });

  test('受講権を 404 以外で読めなければ投げ直し、状態は変えない', () async {
    final container = containerOf(
      FakeStudentApi(failures: {'getMyEntitlement': Fixtures.unreachable}),
    );

    await expectLater(
      container.read(subscriptionProvider.notifier).loadEntitlement(),
      throwsA(isA<StudentApiException>()),
    );

    final state = container.read(subscriptionProvider);
    expect(state.entitlement, isNull);
    expect(state.noEntitlement, isFalse);
  });

  test('ホームを読むと受講権もこちらに持ち、404 なら月額契約の前とする', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: onSubscription.copyWith(step: FlowStep.home),
    );

    await container.read(studentFlowProvider.notifier).refreshHome();

    expect(api.calls, contains('getMyEntitlement'));
    expect(container.read(subscriptionProvider).noEntitlement, isTrue);
    expect(container.read(studentFlowProvider).homeFailures, isEmpty);
  });

  test('キャンセルで回数を使うかは、こちらの受講権の契約種別で見る', () {
    final soon = Fixtures.myBooking(
      bookingId: 'booking-soon',
      startsIn: const Duration(hours: 1),
    ).booking;
    final before = containerOf(FakeStudentApi());
    final subscribed = containerOf(
      FakeStudentApi(),
      subscription: SubscriptionState(
        entitlement: Fixtures.subscriptionStarted.entitlement,
      ),
    );

    expect(
      before.read(studentFlowProvider.notifier).cancellationUsesQuota(soon),
      isFalse,
    );
    expect(
      subscribed.read(studentFlowProvider.notifier).cancellationUsesQuota(soon),
      isTrue,
    );
  });
}
