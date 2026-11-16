import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 下のタブの行き先（components.yaml の navigation_bar.destinations）。並びはこの順で、文言はラベルと読み上げの名前。
/// アイコンはふだん線の形、選択中は塗りの形。
enum RallyNavDestination {
  home('ホーム', Icons.home_outlined, Icons.home),
  bookings('予約', Icons.event_note_outlined, Icons.event_note),
  chat('チャット', Icons.chat_bubble_outline, Icons.chat_bubble),
  notifications('お知らせ', Icons.notifications_outlined, Icons.notifications),
  myPage('マイページ', Icons.person_outline, Icons.person);

  const RallyNavDestination(this.label, this.icon, this.selectedIcon);

  final String label;
  final IconData icon;
  final IconData selectedIcon;
}

/// 下のタブ（components.yaml の navigation_bar。0.12.0）。Material の NavigationBar を包み、見た目はテーマ（NavigationBarThemeData）。
/// 高さ 72px・影なし・上に border 1px・下の安全領域の分だけ下に伸びる。アイコンとラベルはいつも両方出す。
/// チャットは未読の合計を badge の count（100 以上は「99+」）、お知らせは未読があれば dot。[chatUnreadCount] が null（読めない）なら出さない。
/// 選択中のタブを押すと [onReselected]（上端へ戻す。呼ぶ側）。切り替えは fast 120ms（reduced_motion では動きなし）。
/// キーボードが出ているあいだは隠す。読み上げは「チャット、未読 3 件、タブ、5 個中 3 個目」と 1 つの文で読み、選択中を添える。
/// ラベルの文字の拡大は 14px で止める（長押しで tooltip）。
class RallyNavigationBar extends StatelessWidget {
  const RallyNavigationBar({
    super.key,
    required this.selected,
    required this.onSelected,
    this.onReselected,
    this.chatUnreadCount,
    this.hasUnreadNotifications = false,
  });

  /// 選択中の行き先。深い画面では開いた元のタブ
  final RallyNavDestination selected;
  final ValueChanged<RallyNavDestination> onSelected;
  final VoidCallback? onReselected;

  /// チャットの未読の合計。null は読めなかった（しるしを出さない。0 や前の数も出さない）
  final int? chatUnreadCount;
  final bool hasUnreadNotifications;

  /// caption 12px のラベルを 14px で止める（navigation_bar.a11y）
  static const double _maxLabelScale = 14 / 12;

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.viewInsetsOf(context).bottom > 0) {
      return const SizedBox.shrink();
    }
    const destinations = RallyNavDestination.values;
    void select(RallyNavDestination destination) {
      if (destination == selected) {
        onReselected?.call();
      } else {
        onSelected(destination);
      }
    }

    return DecoratedBox(
      decoration: const BoxDecoration(
        border: Border(top: BorderSide(color: RallyColors.border)),
      ),
      child: MediaQuery.withClampedTextScaling(
        maxScaleFactor: _maxLabelScale,
        child: NavigationBar(
          selectedIndex: selected.index,
          onDestinationSelected: (index) => select(destinations[index]),
          animationDuration: MediaQuery.disableAnimationsOf(context)
              ? Duration.zero
              : RallyMotion.fast,
          destinations: [
            for (final destination in destinations)
              _Destination(
                destination: destination,
                selected: destination == selected,
                position: destination.index + 1,
                count: destinations.length,
                unreadCount: destination == RallyNavDestination.chat
                    ? chatUnreadCount
                    : null,
                hasUnread:
                    destination == RallyNavDestination.notifications &&
                    hasUnreadNotifications,
                onTap: () => select(destination),
              ),
          ],
        ),
      ),
    );
  }
}

/// 行き先 1 つ。押せる範囲は item 全体。フォーカスは item の内側に focus_ring 2px。
class _Destination extends StatefulWidget {
  const _Destination({
    required this.destination,
    required this.selected,
    required this.position,
    required this.count,
    required this.unreadCount,
    required this.hasUnread,
    required this.onTap,
  });

  final RallyNavDestination destination;
  final bool selected;
  final int position;
  final int count;
  final int? unreadCount;
  final bool hasUnread;
  final VoidCallback onTap;

  @override
  State<_Destination> createState() => _DestinationState();
}

class _DestinationState extends State<_Destination> {
  bool _focused = false;

  /// 「チャット、未読 3 件、タブ、5 個中 3 個目」。選択中は selected の印で読み上げが添える
  String get _semanticsLabel {
    final label = widget.destination.label;
    final unreadCount = widget.unreadCount;
    final unread = unreadCount != null && unreadCount > 0
        ? '、未読 $unreadCount 件'
        : widget.hasUnread
        ? '、未読あり'
        : '';
    return '$label$unread、タブ、${widget.count} 個中 ${widget.position} 個目';
  }

  Widget _icon(IconData icon) {
    final unreadCount = widget.unreadCount;
    final Widget? badge = unreadCount != null && unreadCount > 0
        ? _CountBadge(unreadCount)
        : widget.hasUnread
        ? const _DotBadge()
        : null;
    if (badge == null) return Icon(icon);
    return Stack(
      clipBehavior: Clip.none,
      children: [
        Icon(icon),
        // アイコンの中心から xs 右・しるしの上端は数なら xs 上、点なら xs 下（どちらも background の 2px のふちの分を引く）
        Positioned(
          left: 12 + RallySpacing.xs - _badgeRing,
          top:
              (badge is _CountBadge ? -RallySpacing.xs : RallySpacing.xs) -
              _badgeRing,
          child: badge,
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: Container(
        foregroundDecoration: _focused
            ? BoxDecoration(
                border: Border.all(color: RallyColors.focusRing, width: 2),
                borderRadius: BorderRadius.circular(RallyRadius.md),
              )
            : null,
        child: Semantics(
          container: true,
          selected: widget.selected,
          label: _semanticsLabel,
          onTap: widget.onTap,
          // しるしと NavigationDestination の英語の「Tab 1 of 5」は読まず、上の 1 つの文だけを読む
          child: ExcludeSemantics(
            child: NavigationDestination(
              icon: _icon(widget.destination.icon),
              selectedIcon: _icon(widget.destination.selectedIcon),
              label: widget.destination.label,
            ),
          ),
        ),
      ),
    );
  }
}

/// しるしのふち（background の 2px）
const double _badgeRing = 2;

/// 未読の数（badge の count）。danger.fg の面に on_primary の caption、100 以上は「99+」
class _CountBadge extends StatelessWidget {
  const _CountBadge(this.count);

  final int count;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: const BoxConstraints(
        minWidth: 20 + _badgeRing * 2,
        minHeight: 20 + _badgeRing * 2,
      ),
      padding: const EdgeInsets.symmetric(horizontal: RallySpacing.xs),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: RallyColors.danger.fg,
        borderRadius: BorderRadius.circular(RallyRadius.sm),
        border: Border.all(color: RallyColors.background, width: _badgeRing),
      ),
      child: Text(
        count > 99 ? '99+' : '$count',
        style: RallyTypography.caption.copyWith(
          color: RallyColors.onPrimary,
          height: 1,
        ),
      ),
    );
  }
}

/// 未読がある印（badge の dot）。danger.fg の 8px の丸
class _DotBadge extends StatelessWidget {
  const _DotBadge();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 8 + _badgeRing * 2,
      height: 8 + _badgeRing * 2,
      decoration: BoxDecoration(
        color: RallyColors.danger.fg,
        shape: BoxShape.circle,
        border: Border.all(color: RallyColors.background, width: _badgeRing),
      ),
    );
  }
}
