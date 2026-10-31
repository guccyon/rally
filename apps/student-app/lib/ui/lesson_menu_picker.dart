import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';

/// 添削の依頼（動画の送付）で選ぶレッスンメニュー。「指定しない」（既定）と、コーチの公開中の非同期のメニューのうち
/// [sport] のもの（作った順。名前と説明）から選ぶ。メニューがなければ選択の欄を出さず、読めなければその旨を出す。
/// チャットの添削の依頼と一周シナリオの「動画を送る」画面で使う。メニューは呼ぶ側が
/// [StudentFlowNotifier.loadCoachLessonMenus] で読む。
class LessonMenuPicker extends ConsumerWidget {
  const LessonMenuPicker({
    super.key,
    required this.coachId,
    required this.sport,
    required this.selected,
    required this.onChanged,
  });

  final String coachId;

  /// 動画のスポーツ（メニューはこのスポーツのものだけを出す）
  final Sport sport;

  /// 選んだメニュー（null は「指定しない」）
  final String? selected;
  final ValueChanged<String?> onChanged;

  /// 選べるメニュー。読み込み中か読めなかったときは null
  static List<LessonMenu>? menusOf(
    StudentFlowState state, {
    required String coachId,
    required Sport sport,
  }) => state.coachLessonMenusFailures.contains(coachId)
      ? null
      : state.asyncLessonMenusOf(coachId, sport: sport);

  /// 送るメニュー。読み直したメニューから選んでいたものが消えていたら「指定しない」（null）に戻す
  static String? selectionOf(
    StudentFlowState state, {
    required String coachId,
    required Sport sport,
    required String? selected,
  }) =>
      menusOf(
            state,
            coachId: coachId,
            sport: sport,
          )?.any((menu) => menu.lessonMenuId == selected) ??
          false
      ? selected
      : null;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final menus = menusOf(state, coachId: coachId, sport: sport);
    if (state.coachLessonMenusFailures.contains(coachId)) {
      return const Text('レッスンメニューを読み込めませんでした。メニューを指定せずに依頼できます。');
    }
    if (menus == null) return const Text('レッスンメニューを読み込んでいます。');
    if (menus.isEmpty) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(height: 8),
        const Text('レッスンメニュー'),
        RadioGroup<String?>(
          groupValue: selectionOf(
            state,
            coachId: coachId,
            sport: sport,
            selected: selected,
          ),
          onChanged: onChanged,
          child: Column(
            children: [
              const RadioListTile<String?>(
                value: null,
                title: Text('指定しない'),
                contentPadding: EdgeInsets.zero,
              ),
              for (final menu in menus)
                RadioListTile<String?>(
                  value: menu.lessonMenuId,
                  title: Text(menu.name),
                  subtitle: menu.description.isEmpty
                      ? null
                      : Text(menu.description),
                  contentPadding: EdgeInsets.zero,
                ),
            ],
          ),
        ),
      ],
    );
  }
}

/// メニューを付けて送ったあとの 422 の文言。選択肢はコーチ・公開中・非同期・動画のスポーツで絞ってあるので、
/// 読み込んだ後にメニューが変わった（非公開にした・直した）ものと読む（admin-web の枠の公開と同じ読み方）。
class LessonMenuRejectedNotice extends StatelessWidget {
  const LessonMenuRejectedNotice({super.key});

  static const message =
      '選んだレッスンメニューでは依頼できませんでした。メニューが非公開になったか、内容が変わった可能性があります。'
      'メニューを読み込み直したので、選び直してください（選んだ動画はそのままです）。';

  @override
  Widget build(BuildContext context) => Text(
    message,
    style: TextStyle(color: Theme.of(context).colorScheme.error),
  );
}
