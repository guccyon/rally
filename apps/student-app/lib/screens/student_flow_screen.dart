import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import 'booking_pending_screen.dart';
import 'coach_profile_screen.dart';
import 'coach_search_screen.dart';
import 'home_screen.dart';
import 'lesson_completion_screen.dart';
import 'lesson_completions_screen.dart';
import 'lesson_review_screen.dart';
import 'live_lesson_screen.dart';
import 'message_threads_screen.dart';
import 'messages_screen.dart';
import 'my_bookings_screen.dart';
import 'my_videos_screen.dart';
import 'notification_settings_screen.dart';
import 'notifications_screen.dart';
import 'prejoin_check_screen.dart';
import 'profile_screen.dart';
import 'register_screen.dart';
import 'slot_search_screen.dart';
import 'subscription_screen.dart';
import 'video_review_screen.dart';
import 'video_submit_screen.dart';

/// 状態の段に応じて画面を出し分ける（画面遷移は state.step だけで決まる）。
/// アプリやタブが前面に戻ったら、月額契約の欄を読み直す（[StudentFlowNotifier.refreshEntitlementOnResume]。
/// 別のタブや端末で契約したあとに「月額契約へ進む」を出したままにしない）。
class StudentFlowScreen extends ConsumerStatefulWidget {
  const StudentFlowScreen({super.key});

  @override
  ConsumerState<StudentFlowScreen> createState() => _StudentFlowScreenState();
}

class _StudentFlowScreenState extends ConsumerState<StudentFlowScreen> {
  late final AppLifecycleListener _lifecycle;

  @override
  void initState() {
    super.initState();
    // Web 版では、タブを切り替えて戻ったとき（visibilitychange）にも呼ばれる
    _lifecycle = AppLifecycleListener(
      onResume: () =>
          ref.read(studentFlowProvider.notifier).refreshEntitlementOnResume(),
    );
  }

  @override
  void dispose() {
    _lifecycle.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final step = ref.watch(studentFlowProvider.select((s) => s.step));
    return switch (step) {
      FlowStep.register => const RegisterScreen(),
      FlowStep.home => const HomeScreen(),
      FlowStep.coachSearch => const CoachSearchScreen(),
      FlowStep.coachProfile => const CoachProfileScreen(),
      FlowStep.slotSearch => const SlotSearchScreen(),
      FlowStep.bookingPending => const BookingPendingScreen(),
      FlowStep.prejoinCheck => const PrejoinCheckScreen(),
      FlowStep.liveLesson => const LiveLessonScreen(),
      FlowStep.videoSubmit => const VideoSubmitScreen(),
      FlowStep.videoReview => const VideoReviewScreen(),
      FlowStep.lessonCompletion => const LessonCompletionScreen(),
      FlowStep.lessonReview => const LessonReviewScreen(),
      FlowStep.subscription => const SubscriptionScreen(),
      FlowStep.messageThreads => const MessageThreadsScreen(),
      FlowStep.messages => const MessagesScreen(),
      FlowStep.notificationSettings => const NotificationSettingsScreen(),
      FlowStep.notifications => const NotificationsScreen(),
      FlowStep.profile => const ProfileScreen(),
      FlowStep.myBookings => const MyBookingsScreen(),
      FlowStep.myVideos => const MyVideosScreen(),
      FlowStep.myVideoReview => const VideoReviewScreen(),
      FlowStep.lessonCompletions => const LessonCompletionsScreen(),
      FlowStep.lessonCompletionReview => const LessonReviewScreen(),
    };
  }
}
