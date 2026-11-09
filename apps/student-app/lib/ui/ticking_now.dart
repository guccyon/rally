import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';

/// 今の時刻（[clockProvider]）を 1 秒ごとに読み直して [builder] に渡す。開いたまま時刻をまたいだら、開き直さずに表示を切り替えるのに使う
/// （入室前チェックを開ける時刻・入室できる時刻・残り時間）。
class TickingNow extends ConsumerStatefulWidget {
  const TickingNow({super.key, required this.builder});

  final Widget Function(BuildContext context, DateTime now) builder;

  @override
  ConsumerState<TickingNow> createState() => _TickingNowState();
}

class _TickingNowState extends ConsumerState<TickingNow> {
  late final Timer _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => setState(() {}));
  }

  @override
  void dispose() {
    _timer.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      widget.builder(context, ref.watch(clockProvider)());
}
