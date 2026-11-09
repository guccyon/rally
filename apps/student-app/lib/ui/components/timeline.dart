import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';
import 'notice.dart';
import 'section_loading.dart';

/// 経緯の 1 つの出来事。
class TimelineEvent {
  const TimelineEvent({
    required this.time,
    required this.label,
    this.status,
    this.meta,
  });

  /// 「10/27（火）9:12」
  final String time;

  /// 状態なら glossary の呼び名（「確定済み」）、ほかは「〜した」（「取り下げた」）
  final String label;

  /// 状態の出来事だけ。[RallyBookingState.status]・[RallySlotState.status] の対応の色を渡す。
  /// 渡すと [label] を status_tag の字で、渡さないと body 700 で出し、点は border_strong にする
  final RallyStatus? status;

  /// 「本部: 鈴木」「理由: コーチのお休み」
  final String? meta;

  /// 出来事を 1 つの文で読む（「10/27（火）9:12、確定済み、本部: 鈴木」）
  String get sentence => [time, label, ?meta].join('、');
}

/// 経緯の並び（components.yaml の timeline。0.8.0）。1 件に起きた出来事を古い順に、点と線でつないで縦に並べる。
/// 出来事は渡した順（API の順）のまま、動きは付けない。rail（幅 16px・点 8px・border の 2px の線）は読み上げず、
/// 出来事を 1 つの文で読む。読んでいるあいだは [loading]（section_text）、読めなかったときは [error]（notice の inline）、
/// 0 件は [empty] の文を出す。11 件以上か期間で絞る記録には使わない。
class Timeline extends StatelessWidget {
  const Timeline({
    super.key,
    required this.semanticsLabel,
    required this.events,
    required this.empty,
    this.loading,
    this.error,
  });

  /// 並びの名前（「状態が変わった記録」）
  final String semanticsLabel;
  final List<TimelineEvent> events;

  /// 0 件のときの文（「状態が変わった記録はありません」）
  final String empty;
  final String? loading;
  final String? error;

  @override
  Widget build(BuildContext context) {
    if (loading case final message?) return SectionLoading(message: message);
    if (error case final message?) {
      return Notice(message: message, status: RallyStatus.danger);
    }
    if (events.isEmpty) {
      return Text(
        empty,
        style: RallyTypography.body.copyWith(color: RallyColors.text),
      );
    }
    return Semantics(
      container: true,
      label: semanticsLabel,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final (index, event) in events.indexed)
            _Event(
              event: event,
              first: index == 0,
              last: index == events.length - 1,
            ),
        ],
      ),
    );
  }
}

class _Event extends StatelessWidget {
  const _Event({required this.event, required this.first, required this.last});

  final TimelineEvent event;
  final bool first;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final status = event.status;
    final muted = status == null || status == RallyStatus.neutral;
    return Semantics(
      label: event.sentence,
      excludeSemantics: true,
      child: IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          spacing: RallySpacing.sm,
          children: [
            // rail。幅 16px に 8px の点、前後の出来事へ border の 2px の線
            SizedBox(
              width: RallySpacing.md,
              child: Column(
                children: [
                  Container(
                    width: 2,
                    height: RallySpacing.sm,
                    color: first ? null : RallyColors.border,
                  ),
                  Container(
                    key: const ValueKey('timeline-dot'),
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: (status ?? RallyStatus.neutral).colors.fg,
                      borderRadius: BorderRadius.circular(RallyRadius.pill),
                    ),
                  ),
                  if (!last)
                    Expanded(
                      child: Container(width: 2, color: RallyColors.border),
                    ),
                ],
              ),
            ),
            Expanded(
              child: Padding(
                padding: EdgeInsets.only(bottom: last ? 0 : RallySpacing.md),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  spacing: RallySpacing.xs,
                  children: [
                    Text(
                      event.time,
                      style: RallyTypography.small.copyWith(
                        color: RallyColors.textMuted,
                      ),
                    ),
                    Text(
                      event.label,
                      style: status == null
                          ? RallyTypography.body.copyWith(
                              fontWeight: FontWeight.w700,
                              color: RallyColors.text,
                            )
                          : RallyTypography.small.copyWith(
                              fontWeight: FontWeight.w700,
                              color: muted
                                  ? RallyColors.textMuted
                                  : RallyColors.text,
                            ),
                    ),
                    if (event.meta case final meta?)
                      Text(
                        meta,
                        style: RallyTypography.small.copyWith(
                          color: RallyColors.textMuted,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
