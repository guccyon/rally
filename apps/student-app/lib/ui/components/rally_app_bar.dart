import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// アプリバーの入口（components.yaml の app_bar.actions.items）。並びはこの順で、文言は tooltip と読み上げのラベル。
enum RallyAppBarEntry {
  home('ホーム', Icons.home_outlined),
  myBookings('自分の予約', Icons.event_note_outlined),
  myVideos('自分の動画', Icons.video_library_outlined),
  chat('チャット', Icons.chat_bubble_outline),
  notifications('お知らせ', Icons.notifications_outlined),
  notificationSettings('通知の設定', Icons.edit_notifications_outlined),
  profile('プロフィール', Icons.person_outline);

  const RallyAppBarEntry(this.label, this.icon);

  final String label;
  final IconData icon;
}

/// アプリバーの入口 1 つ。[unreadCount] はチャットの count のしるし、[hasUnread] はお知らせの dot のしるし（どちらもホームだけ）。
class RallyAppBarAction {
  const RallyAppBarAction(
    this.entry, {
    required this.onPressed,
    this.unreadCount = 0,
    this.hasUnread = false,
  });

  final RallyAppBarEntry entry;
  final VoidCallback onPressed;
  final int unreadCount;
  final bool hasUnread;
}

/// アプリバー（components.yaml の app_bar。0.10.0）。background に下の border 1px・影なし・高さ 56px・左右 xs 4px。
/// [onBack] があれば左に戻る（種類 back）、なければ出さない（種類 top）。題は title 17px 900 で 1 行、末尾を「…」で切り、見出しとして読む。
/// 入口（[actions]）は 48px の押せる範囲にアイコン 24px。出す条件（登録したあと・脇の画面でない など）は呼ぶ側が決める。
/// [busy]（送信中）は戻ると入口を text_muted にして押せなくし、端末の戻るも止める。
/// [loading] は送信中が 300ms を超えたとき（呼ぶ側が決める）で、下に loading の bar（4px）を出す。
/// 失敗はバーを変えず、本文の上に notice の inline（danger）を出す（呼ぶ側）。
class RallyAppBar extends StatelessWidget implements PreferredSizeWidget {
  const RallyAppBar({
    super.key,
    required this.title,
    this.onBack,
    this.actions = const [],
    this.busy = false,
    this.loading = false,
  });

  /// 入口が 4 つ以上で題が入らない画面は空にして、本文の先頭に headline で出す（app_bar.behavior）
  final String title;

  /// 戻る。端末の戻ると同じ行き先にする
  final VoidCallback? onBack;
  final List<RallyAppBarAction> actions;
  final bool busy;
  final bool loading;

  static const double _height = 56;
  static const double _progressHeight = 4;

  @override
  Size get preferredSize =>
      Size.fromHeight(_height + (loading ? _progressHeight : 0));

  @override
  Widget build(BuildContext context) {
    final back = onBack;
    return PopScope(
      // 送る途中で離れると届いたかが分からないので、端末の戻るも止める（app_bar の should）
      canPop: !busy,
      child: Material(
        color: RallyColors.background,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              height: _height,
              padding: const EdgeInsets.symmetric(horizontal: RallySpacing.xs),
              decoration: BoxDecoration(
                border: loading
                    ? null
                    : const Border(
                        bottom: BorderSide(color: RallyColors.border),
                      ),
              ),
              child: Row(
                children: [
                  if (back != null)
                    _AppBarIconButton(
                      label: '戻る',
                      icon: const Icon(Icons.arrow_back_rounded),
                      onPressed: busy ? null : back,
                    ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.only(left: RallySpacing.md),
                      child: Semantics(
                        header: true,
                        child: Text(
                          title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: RallyTypography.title.copyWith(
                            color: RallyColors.text,
                          ),
                        ),
                      ),
                    ),
                  ),
                  for (final action in actions)
                    _AppBarIconButton(
                      label: action.entry.label,
                      semanticsLabel: _semanticsLabel(action),
                      icon: _icon(action),
                      onPressed: busy ? null : action.onPressed,
                    ),
                ],
              ),
            ),
            // 色はテーマ（ProgressIndicatorThemeData）の primary と primary_container、高さ 4px
            if (loading) const LinearProgressIndicator(),
          ],
        ),
      ),
    );
  }

  /// 未読は「チャット、未読 3 件」「お知らせ、未読あり」と読む（app_bar.a11y）
  static String _semanticsLabel(RallyAppBarAction action) {
    if (action.unreadCount > 0) {
      return '${action.entry.label}、未読 ${action.unreadCount} 件';
    }
    if (action.hasUnread) return '${action.entry.label}、未読あり';
    return action.entry.label;
  }

  /// しるしはテーマ（BadgeThemeData）の Badge。チャットは count、お知らせは dot
  static Widget _icon(RallyAppBarAction action) {
    final icon = Icon(action.entry.icon);
    if (action.unreadCount > 0) {
      return Badge.count(count: action.unreadCount, child: icon);
    }
    if (action.hasUnread) return Badge(child: icon);
    return icon;
  }
}

/// アプリバーのアイコンのボタン。押せる範囲 48px・アイコン 24px（text）。押下はアイコンの下に primary_container の丸（pill）、
/// フォーカスは focus_ring 2px、押せないときは text_muted。tooltip は入口の語、読み上げは [semanticsLabel]（未読の数つき）。
class _AppBarIconButton extends StatelessWidget {
  const _AppBarIconButton({
    required this.label,
    required this.icon,
    required this.onPressed,
    this.semanticsLabel,
  });

  final String label;
  final String? semanticsLabel;
  final Widget icon;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    // tooltip の文は読み上げに重ねず、ボタンのラベル（未読の数つき）だけを読む
    return Tooltip(
      message: label,
      excludeFromSemantics: true,
      child: IconButton(
        onPressed: onPressed,
        icon: Semantics(
          label: semanticsLabel ?? label,
          child: ExcludeSemantics(child: icon),
        ),
        style: ButtonStyle(
          fixedSize: const WidgetStatePropertyAll(
            Size.square(RallySpacing.touchTarget),
          ),
          iconSize: const WidgetStatePropertyAll(24),
          shape: const WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.all(Radius.circular(RallyRadius.pill)),
            ),
          ),
          animationDuration: RallyMotion.fast,
          backgroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.pressed)
                ? RallyColors.primaryContainer
                : Colors.transparent,
          ),
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? RallyColors.textMuted
                : RallyColors.text,
          ),
          side: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.focused)
                ? const BorderSide(color: RallyColors.focusRing, width: 2)
                : BorderSide.none,
          ),
          overlayColor: const WidgetStatePropertyAll(Colors.transparent),
        ),
      ),
    );
  }
}
