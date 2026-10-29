import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import 'booking_pending_screen.dart';
import 'home_screen.dart';
import 'lesson_completion_screen.dart';
import 'lesson_review_screen.dart';
import 'live_lesson_screen.dart';
import 'message_threads_screen.dart';
import 'messages_screen.dart';
import 'my_bookings_screen.dart';
import 'notification_settings_screen.dart';
import 'register_screen.dart';
import 'slot_search_screen.dart';
import 'subscription_screen.dart';
import 'video_review_screen.dart';
import 'video_submit_screen.dart';

/// 状態の段に応じて画面を出し分ける（画面遷移は state.step だけで決まる）。
class StudentFlowScreen extends ConsumerWidget {
  const StudentFlowScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final step = ref.watch(studentFlowProvider.select((s) => s.step));
    return switch (step) {
      FlowStep.register => const RegisterScreen(),
      FlowStep.home => const HomeScreen(),
      FlowStep.slotSearch => const SlotSearchScreen(),
      FlowStep.bookingPending => const BookingPendingScreen(),
      FlowStep.liveLesson => const LiveLessonScreen(),
      FlowStep.videoSubmit => const VideoSubmitScreen(),
      FlowStep.videoReview => const VideoReviewScreen(),
      FlowStep.lessonCompletion => const LessonCompletionScreen(),
      FlowStep.lessonReview => const LessonReviewScreen(),
      FlowStep.subscription => const SubscriptionScreen(),
      FlowStep.messageThreads => const MessageThreadsScreen(),
      FlowStep.messages => const MessagesScreen(),
      FlowStep.notificationSettings => const NotificationSettingsScreen(),
      FlowStep.myBookings => const MyBookingsScreen(),
    };
  }
}
