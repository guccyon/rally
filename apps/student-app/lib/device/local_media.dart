import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'local_media_unsupported.dart'
    if (dart.library.js_interop) 'web_local_media.dart';

/// 端末のカメラ・マイクを開いた結果（カメラとマイクは別々に扱う）。
enum MediaDeviceStatus {
  /// 使える（開いた）
  ready,

  /// 許可を断られた・ほかのアプリが使っていて使えない
  denied,

  /// 端末にない
  missing,
}

/// 端末のカメラ・マイク・スピーカーを触る所（入室前チェックのポート）。本物は [createLocalMedia]（Web 版はブラウザー）、
/// テストは作り物に差し替える（[localMediaProvider]）。
/// 映像と音は端末の中で見せて聞かせるだけで、録らない・残さない・外へ送らない（governance の media-no-record）。
/// 通話サービスにはつながない。
abstract class LocalMedia {
  /// 前のカメラを開く。はじめては端末が許可を求める。開いたら [stopCamera] か [stopAll] まで使い続ける
  Future<MediaDeviceStatus> openCamera();

  /// マイクを開き、音の大きさを [micLevels] に流し始める。はじめては端末が許可を求める
  Future<MediaDeviceStatus> openMic();

  /// カメラを止める（端末のカメラの使用中の印も消える）
  void stopCamera();

  /// マイクを止める
  void stopMic();

  /// 開いたカメラの映像（開いていなければ null）。枠に合わせて切り抜くのは呼ぶ側
  Widget? cameraView();

  /// マイクの音の大きさ（0.0〜1.0）。開いているあいだ 1 秒に 10 回まで流す
  Stream<double> get micLevels;

  /// 同梱の試し音を 1 回（3 秒）鳴らす。鳴り終わるか [stopTestTone] で完了する。鳴らせなければ例外を投げる
  Future<void> playTestTone();

  /// 鳴っている試し音を止める
  void stopTestTone();

  /// 端末のこのアプリの設定を開けるか（Web 版のブラウザーは開けない）
  bool get canOpenSettings;

  /// 端末のこのアプリの設定を開く
  Future<void> openSettings();

  /// カメラ・マイク・試し音をすべて止める（画面を離れたとき）
  void stopAll();
}

/// 入室前チェックが使う端末のカメラ・マイク・スピーカー。テストでは作り物に差し替える。
final localMediaProvider = Provider<LocalMedia>((ref) {
  final media = createLocalMedia();
  ref.onDispose(media.stopAll);
  return media;
});
