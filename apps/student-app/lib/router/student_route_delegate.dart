import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../screens/live_lesson_screen.dart';
import '../state/chat.dart';
import '../state/coach_profile.dart';
import '../state/lesson_completions.dart';
import '../state/notifications.dart';
import '../state/student_flow.dart';

/// 段（[FlowStep]）を写したブラウザーの履歴の 1 項目。
/// 履歴はホームを底にした 2 段（[depth] はホームと登録が 0、ほかの段は 1）で、ホームから進むときだけ積み、段から段へは今の項目を置き換える。
/// [step] は URL に出すだけで、開き直したときの画面の復元には使わない（ADR 0010。開き直したらホームを開く）
class StudentRoutePath {
  const StudentRoutePath(this.step, {this.depth});

  /// URL の段（知らない URL なら null）
  final FlowStep? step;

  /// 履歴の項目に残した深さ（アプリが積んだ項目でなければ null。読み込み直したときもない）
  final int? depth;
}

/// URL（`/slot-search` のように段の名前をつないだもの）と履歴の項目の状態（深さ）を [StudentRoutePath] に読み書きする
class StudentRouteInformationParser
    extends RouteInformationParser<StudentRoutePath> {
  const StudentRouteInformationParser();

  @override
  Future<StudentRoutePath> parseRouteInformation(
    RouteInformation routeInformation,
  ) async {
    final path = routeInformation.uri.path;
    final state = routeInformation.state;
    return StudentRoutePath(
      FlowStep.values.where((step) => _pathOf(step) == path).firstOrNull,
      depth: state is Map && state['depth'] is int
          ? state['depth'] as int
          : null,
    );
  }

  @override
  RouteInformation restoreRouteInformation(StudentRoutePath configuration) =>
      RouteInformation(
        uri: Uri.parse(_pathOf(configuration.step ?? FlowStep.home)),
        state: {'depth': configuration.depth ?? 0},
      );

  static String _pathOf(FlowStep step) =>
      '/${step.name.replaceAllMapped(RegExp('[A-Z]'), (m) => '-${m[0]!.toLowerCase()}')}';
}

/// 段の移動をブラウザーの履歴に写す。ホームより上の段へ進んだら項目を積み、段から段への移動・登録からホームへは
/// 今の項目を置き換える（ホームを履歴の底にし、登録の画面を履歴に残さない）。
/// アプリが報告した項目は [StudentRoutePath.depth] を持つので、深さが下がった項目が来たら「戻る」と読み、
/// アプリの閉じる・戻るのボタンと同じ行き先へ移す（[_back]）。それ以外の項目（再読み込み・「進む」・URL を開いた）では段を変えない
class StudentRouterDelegate extends RouterDelegate<StudentRoutePath>
    with ChangeNotifier {
  StudentRouterDelegate(this._container, {required this.home}) {
    _subscription = _container.listen<FlowStep>(
      studentFlowProvider.select((s) => s.step),
      (_, step) => _moved(step),
    );
  }

  final ProviderContainer _container;

  /// 履歴に関わらず出し続ける画面（段の出し分けはこの中）
  final Widget home;

  /// 画面の上に開いたダイアログを「戻る」で閉じるのと、通話を終えるかを確かめるダイアログを出すのに使う
  final navigatorKey = GlobalKey<NavigatorState>();

  late final ProviderSubscription<FlowStep> _subscription;

  /// ブラウザーの今の項目の深さ
  int _depth = 0;

  FlowStep get _step => _container.read(studentFlowProvider).step;

  @override
  StudentRoutePath get currentConfiguration =>
      StudentRoutePath(_step, depth: _depth);

  @override
  Widget build(BuildContext context) => Navigator(
    key: navigatorKey,
    pages: [MaterialPage(child: home)],
    onDidRemovePage: (_) {},
  );

  /// 読み込み直したとき（再読み込み・「進む」・URL を開いた）。URL の段は使わず、今の段（開き直したらホームか登録）を写し直す
  @override
  Future<void> setInitialRoutePath(StudentRoutePath configuration) async {
    _depth = _depthOf(_step);
    notifyListeners();
  }

  /// ブラウザーが履歴の項目を移った。深さが下がっていれば「戻る」として今の段を閉じ、移った項目を今の段で書き直す
  /// （閉じても段がホームより上なら、もう一度積む）。URL の段では画面を変えない
  @override
  Future<void> setNewRoutePath(StudentRoutePath configuration) async {
    final depth = configuration.depth ?? 0;
    final back = depth < _depth;
    _depth = depth;
    if (back) _back();
    _depth = max(_depth, _depthOf(_step));
    notifyListeners();
  }

  /// 端末の「戻る」（Android の戻るボタンなど）。ホームと登録では抜ける（false）
  @override
  Future<bool> popRoute() async {
    if (_depthOf(_step) == 0 && !_hasDialog) return false;
    _back();
    return true;
  }

  void _moved(FlowStep step) {
    _depth = max(_depth, _depthOf(step));
    notifyListeners();
  }

  bool get _hasDialog => navigatorKey.currentState?.canPop() ?? false;

  /// 「戻る」。ダイアログが開いていれば閉じる（「やめる」と同じ）。API を呼んでいる間はボタンと同じく何もしない。
  /// それ以外は今の段の閉じる・戻るのボタンと同じ行き先へ移す（脇の画面は開いた段、一周の段はホーム）。
  /// 入室前チェックと予約の確定待ちはホームへ（確定待ちの予約はホームの「次にやること」に残る）。
  /// ビデオ通話の最中は通話の画面にとどまり、通話を終えるかを確かめる
  void _back() {
    if (_hasDialog) {
      navigatorKey.currentState!.maybePop();
      return;
    }
    final flow = _container.read(studentFlowProvider);
    if (flow.busy) return;
    final notifier = _container.read(studentFlowProvider.notifier);
    switch (flow.step) {
      case FlowStep.register || FlowStep.home:
        return;
      case FlowStep.liveLesson
          when flow.session?.status == LiveLessonSessionStatus.inProgress:
        final context = navigatorKey.currentContext;
        if (context != null) confirmEndingCall(context, notifier);
      case FlowStep.coachProfile:
        _container.read(coachProfileProvider.notifier).close();
      case FlowStep.messageThreads:
        _container.read(chatProvider.notifier).closeChat();
      case FlowStep.messages:
        _container.read(chatProvider.notifier).backToThreads();
      case FlowStep.notificationSettings:
        notifier.closeNotificationSettings();
      case FlowStep.notifications:
        _container.read(notificationsProvider.notifier).close();
      case FlowStep.profile:
        notifier.closeProfile();
      case FlowStep.myBookings:
        notifier.closeMyBookings();
      case FlowStep.myVideos:
        notifier.closeMyVideos();
      case FlowStep.myVideoReview:
        notifier.closeMyVideoReview();
      case FlowStep.lessonCompletionReview:
        _container.read(lessonCompletionsProvider.notifier).backFromReview();
      case FlowStep.coachSearch ||
          FlowStep.slotSearch ||
          FlowStep.bookingPending ||
          FlowStep.prejoinCheck ||
          FlowStep.liveLesson ||
          FlowStep.videoSubmit ||
          FlowStep.videoReview ||
          FlowStep.lessonCompletion ||
          FlowStep.lessonReview ||
          FlowStep.subscription ||
          FlowStep.lessonCompletions:
        notifier.goHome();
    }
  }

  static int _depthOf(FlowStep step) =>
      step == FlowStep.home || step == FlowStep.register ? 0 : 1;

  @override
  void dispose() {
    _subscription.close();
    super.dispose();
  }
}

/// [StudentRouterDelegate] が報告した項目を、深さが上がったときだけ積み、ほかは今の項目を置き換える
class StudentRouteInformationProvider extends PlatformRouteInformationProvider {
  StudentRouteInformationProvider()
    : super(
        initialRouteInformation: RouteInformation(
          uri: Uri.parse(
            WidgetsBinding.instance.platformDispatcher.defaultRouteName,
          ),
        ),
      );

  @override
  void routerReportsNewRouteInformation(
    RouteInformation routeInformation, {
    RouteInformationReportingType type = RouteInformationReportingType.none,
  }) => super.routerReportsNewRouteInformation(
    routeInformation,
    type: _depthOf(routeInformation) > _depthOf(value)
        ? RouteInformationReportingType.navigate
        : RouteInformationReportingType.neglect,
  );

  static int _depthOf(RouteInformation information) =>
      switch (information.state) {
        {'depth': final int depth} => depth,
        _ => 0,
      };
}
