import 'package:flutter/widgets.dart';

/// [context] のダイアログやボトムシートを [result] で閉じる。すでに閉じ始めていたら（すばやい二度押しの 2 回目）何もしない。
/// 画面の読み上げや Web 版のセマンティクスの要素からの押下は、閉じた直後の押下を捨てる Navigator の仕組みを通らないので、
/// 閉じ始めたダイアログのボタンがもう一度呼ばれることがある。そのまま pop すると下の画面のルートを閉じて画面が真っ白になる（#1513）。
/// 選択や確定のボタンはこれで閉じる。
void popOnce<T>(BuildContext context, [T? result]) {
  final route = ModalRoute.of(context);
  if (route == null || !route.isCurrent) return;
  Navigator.of(context).pop<T>(result);
}
