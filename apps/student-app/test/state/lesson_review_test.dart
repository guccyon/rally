import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/state/lesson_review.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';

void main() {
  final afterCompletion = StudentFlowState(
    step: FlowStep.lessonReview,
    student: Fixtures.student,
    booking: Fixtures.bookingConfirmed,
    session: Fixtures.sessionEnded,
    lesson: Fixtures.liveLesson,
  );
  final lessonId = Fixtures.liveLesson.lessonId;

  /// API をモックにした ProviderContainer（画面は描かない）
  ProviderContainer containerOf(
    FakeStudentApi api, {
    StudentFlowState? flow,
    LessonReviewState? review,
  }) {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        studentFlowProvider.overrideWith(
          () => StudentFlowNotifier(flow ?? afterCompletion),
        ),
        lessonReviewProvider.overrideWith(() => LessonReviewNotifier(review)),
        clockProvider.overrideWithValue(() => Fixtures.now),
      ],
    );
    addTearDown(container.dispose);
    return container;
  }

  test('評価を送ると完了を記録したレッスンに submitLessonReview を呼び、応答を持って評価済みにする', () async {
    final api = FakeStudentApi();
    final container = containerOf(api);

    await container
        .read(lessonReviewProvider.notifier)
        .submitReview(rating: 4, comment: '丁寧でした');

    expect(api.calls, ['submitLessonReview']);
    expect((api.requests.single as SubmitLessonReviewRequest).toJson(), {
      'lesson_id': lessonId,
      'rating': 4,
      'comment': '丁寧でした',
    });
    final state = container.read(lessonReviewProvider);
    expect(state.lessonReview?.lessonId, lessonId);
    expect(state.lessonReview?.rating, 4);
    expect(state.reviewedLessonIds, {lessonId});
    expect(state.isReviewed(lessonId), isTrue);
    expect(state.isReviewed(Fixtures.asyncLesson.lessonId), isFalse);
    final flow = container.read(studentFlowProvider);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, isNull);
    expect(flow.step, FlowStep.lessonReview);
  });

  test('409 lesson_already_reviewed なら応答は持たずに評価済みにし、失敗の文言を出す', () async {
    final api = FakeStudentApi(
      failures: {'submitLessonReview': Fixtures.alreadyReviewed},
    );
    final container = containerOf(api);

    await container
        .read(lessonReviewProvider.notifier)
        .submitReview(rating: 5, comment: 'よかった');

    final state = container.read(lessonReviewProvider);
    expect(state.lessonReview, isNull);
    expect(state.isReviewed(lessonId), isTrue);
    final flow = container.read(studentFlowProvider);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, 'このレッスンはすでに評価しています。');
  });

  test('ほかの失敗では評価済みにせず、失敗の文言を出す', () async {
    final api = FakeStudentApi(
      failures: {'submitLessonReview': Fixtures.validationFailed},
    );
    final container = containerOf(api);

    await container
        .read(lessonReviewProvider.notifier)
        .submitReview(rating: 3, comment: 'ふつう');

    final state = container.read(lessonReviewProvider);
    expect(state.lessonReview, isNull);
    expect(state.reviewedLessonIds, isEmpty);
    expect(container.read(studentFlowProvider).errorMessage, '入力内容を確かめてください。');
  });

  test('ほかの操作が API を呼んでいる間（busy）は送らない', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: afterCompletion.copyWith(busy: true),
    );

    await container
        .read(lessonReviewProvider.notifier)
        .submitReview(rating: 4, comment: '丁寧でした');

    expect(api.calls, isEmpty);
    expect(container.read(lessonReviewProvider).reviewedLessonIds, isEmpty);
  });

  test('評価済みのレッスンは追記で増え、読み出しのビューからは変えられない', () async {
    final container = containerOf(
      FakeStudentApi(),
      review: LessonReviewState(
        reviewedLessonIds: {Fixtures.asyncLesson.lessonId},
      ),
    );

    await container
        .read(lessonReviewProvider.notifier)
        .submitReview(rating: 4, comment: '丁寧でした');

    final state = container.read(lessonReviewProvider);
    expect(state.reviewedLessonIds, {Fixtures.asyncLesson.lessonId, lessonId});
    expect(() => state.reviewedLessonIds.add('other'), throwsUnsupportedError);
    expect(state.copyWith().reviewedLessonIds, state.reviewedLessonIds);
  });

  test('渡した元の Set を後から変えても、状態の評価済みのレッスンは変わらない', () {
    final source = {Fixtures.asyncLesson.lessonId};
    final state = LessonReviewState(reviewedLessonIds: source);
    final copied = state.copyWith(reviewedLessonIds: source);

    source.add(lessonId);

    expect(state.reviewedLessonIds, {Fixtures.asyncLesson.lessonId});
    expect(state.isReviewed(lessonId), isFalse);
    expect(copied.reviewedLessonIds, {Fixtures.asyncLesson.lessonId});
    expect(() => state.reviewedLessonIds.add('other'), throwsUnsupportedError);
    expect(() => copied.reviewedLessonIds.add('other'), throwsUnsupportedError);
  });
}
