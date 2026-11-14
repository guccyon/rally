import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../device/local_media.dart';
import 'student_flow.dart';

/// 入室前チェックを開ける、開始時刻までの長さ（specs/glossary.yaml の prejoin_check の 5 分）。この 1 か所だけを参照する。
const prejoinCheckLeadTime = Duration(minutes: 5);

/// 入室前チェックを開けるようになる時刻（開始 5 分前）
DateTime prejoinCheckOpensAt(DateTime startsAt) =>
    startsAt.subtract(prejoinCheckLeadTime);

/// [now] に入室前チェックを開けるか（開始 5 分前ちょうどから）
bool canOpenPrejoinCheck(DateTime startsAt, DateTime now) =>
    !now.isBefore(prejoinCheckOpensAt(startsAt));

/// [now] に入室できるか（開始時刻ちょうどから。#921 の slot_not_started と同じ境目）
bool canJoinLiveLesson(DateTime startsAt, DateTime now) =>
    !now.isBefore(startsAt);

/// 音が届いていないとみなすまでの、静かな計測の回数（1 秒に 10 回 × 2 秒。level_meter の「label は 2 秒続いたら替える」）
const _silentSamplesToReset = 20;

/// 音が届いたとみなす段（10 段のうち）
const _heardSegments = 2;

/// 入室前チェックの状態。段の移動と busy・入室の失敗の文言は [StudentFlowState] に置く。
class PrejoinCheckState {
  const PrejoinCheckState({
    this.camera,
    this.mic,
    this.cameraOn = true,
    this.micOn = true,
    this.micLevel = 0,
    this.micHeard = false,
    this.tonePlaying = false,
    this.toneFailed = false,
    this.carried,
  });

  /// カメラを開いた結果。null は開いているところ（許可を待っている）
  final MediaDeviceStatus? camera;

  /// マイクを開いた結果。null は開いているところ（許可を待っている）
  final MediaDeviceStatus? mic;

  /// カメラの入・切（既定は入。入室に引き継ぐ）
  final bool cameraOn;

  /// マイクの入・切（既定は入。入室に引き継ぐ）
  final bool micOn;

  /// マイクの音の大きさ（届いた段の数。0〜10）
  final int micLevel;

  /// 音が届いているか（届いてから 2 秒静かになるまで true）
  final bool micHeard;

  /// 試し音を鳴らしているか
  final bool tonePlaying;

  /// 直前の試し音を鳴らせなかったか
  final bool toneFailed;

  /// 入室したときのカメラ・マイクの入・切（ビデオ通話の画面に出す）。入室していなければ null
  final ({bool camera, bool mic})? carried;

  bool get cameraReady => camera == MediaDeviceStatus.ready;
  bool get micReady => mic == MediaDeviceStatus.ready;

  PrejoinCheckState copyWith({
    MediaDeviceStatus? camera,
    MediaDeviceStatus? mic,
    bool? cameraOn,
    bool? micOn,
    int? micLevel,
    bool? micHeard,
    bool? tonePlaying,
    bool? toneFailed,
    ({bool camera, bool mic})? carried,
  }) => PrejoinCheckState(
    camera: camera ?? this.camera,
    mic: mic ?? this.mic,
    cameraOn: cameraOn ?? this.cameraOn,
    micOn: micOn ?? this.micOn,
    micLevel: micLevel ?? this.micLevel,
    micHeard: micHeard ?? this.micHeard,
    tonePlaying: tonePlaying ?? this.tonePlaying,
    toneFailed: toneFailed ?? this.toneFailed,
    carried: carried ?? this.carried,
  );
}

final prejoinCheckProvider =
    NotifierProvider<PrejoinCheckNotifier, PrejoinCheckState>(
      PrejoinCheckNotifier.new,
    );

/// 入室前チェックの操作。端末のカメラ・マイク・スピーカーは [localMediaProvider] を通してだけ触る。
/// API を呼ぶのは「入室する」（[StudentFlowNotifier.joinLiveLesson]）だけで、映像と音は端末の外に出さない。
class PrejoinCheckNotifier extends Notifier<PrejoinCheckState> {
  /// [initial] はテストで途中の状態から始めるときに渡す。
  PrejoinCheckNotifier([PrejoinCheckState? initial])
    : _initial = initial ?? const PrejoinCheckState();

  final PrejoinCheckState _initial;

  StreamSubscription<double>? _levels;
  var _silentSamples = 0;

  /// 試し音を鳴らすたびに増やす（止めたあとに前の試し音が終わっても状態を戻さない）
  var _toneGeneration = 0;

  /// 「もう一度確かめる」の二重押しを 1 回にまとめる
  Future<void>? _checking;

  @override
  PrejoinCheckState build() {
    ref.onDispose(() => _levels?.cancel());
    return _initial;
  }

  LocalMedia get _media => ref.read(localMediaProvider);

  /// 予約の確定の「ビデオ通話で受ける」。確定済みの予約でないか、開始 5 分前より早ければ何もしない（ボタンも押せない）。
  /// 開けたら前のチェックの状態を捨てて入室前チェックへ（カメラ・マイクは画面が開いてから [start] で開く）。
  void open() {
    final flow = ref.read(studentFlowProvider);
    final booking = flow.booking;
    // 入室は読み直した確定済みの予約からだけ（キャンセル済み・確定待ち・読み直す前の予約からは開かない。#1364）
    if (booking == null ||
        !ref.read(studentFlowProvider.notifier).canChooseLesson) {
      return;
    }
    if (!canOpenPrejoinCheck(booking.startsAt, ref.read(clockProvider)())) {
      return;
    }
    _stopDevices();
    state = const PrejoinCheckState();
    ref.read(studentFlowProvider.notifier).moveTo(FlowStep.prejoinCheck);
  }

  /// 画面を開いた・アプリが前に戻った。入にしているカメラ、続けてマイクを開く（はじめては端末が許可を求める）
  Future<void> start() async {
    if (state.cameraOn && !state.cameraReady) {
      state = PrejoinCheckState(
        mic: state.mic,
        cameraOn: state.cameraOn,
        micOn: state.micOn,
        tonePlaying: state.tonePlaying,
        toneFailed: state.toneFailed,
        carried: state.carried,
      );
      final camera = await _media.openCamera();
      if (!ref.mounted) return;
      state = state.copyWith(camera: camera);
    }
    if (state.micOn && !state.micReady) {
      final mic = await _media.openMic();
      if (!ref.mounted) return;
      state = state.copyWith(mic: mic);
      if (mic == MediaDeviceStatus.ready) _listenLevels();
    }
  }

  /// 「もう一度確かめる」。使えないカメラ・マイクを開き直す（二重押しは 1 回にまとめる）
  Future<void> recheck() => _checking ??= start().whenComplete(() {
    _checking = null;
  });

  /// 「端末の設定を開く」。戻ってきたら画面が [start] で確かめ直す
  Future<void> openSettings() => _media.openSettings();

  /// カメラを切る・入れる（確認しない）。切るとカメラを止める
  Future<void> toggleCamera() async {
    if (state.cameraOn) {
      _media.stopCamera();
      state = state.copyWith(cameraOn: false);
      return;
    }
    state = state.copyWith(cameraOn: true);
    final camera = await _media.openCamera();
    if (ref.mounted) state = state.copyWith(camera: camera);
  }

  /// マイクを切る・入れる（確認しない）。切るとマイクを止め、音の大きさを 0 に
  Future<void> toggleMic() async {
    if (state.micOn) {
      _stopMic();
      state = state.copyWith(micOn: false, micLevel: 0, micHeard: false);
      return;
    }
    state = state.copyWith(micOn: true);
    final mic = await _media.openMic();
    if (!ref.mounted) return;
    state = state.copyWith(mic: mic);
    if (mic == MediaDeviceStatus.ready) _listenLevels();
  }

  /// 「スピーカーを試す」⇄「試し音を止める」。鳴らすのは 1 回（3 秒）で、二重に鳴らさない
  Future<void> toggleTestTone() async {
    final generation = ++_toneGeneration;
    if (state.tonePlaying) {
      _media.stopTestTone();
      state = state.copyWith(tonePlaying: false);
      return;
    }
    state = state.copyWith(tonePlaying: true, toneFailed: false);
    var failed = false;
    try {
      await _media.playTestTone();
    } catch (_) {
      failed = true;
    }
    if (!ref.mounted || generation != _toneGeneration) return;
    state = state.copyWith(tonePlaying: false, toneFailed: failed);
  }

  /// 「入室する」。入れたらカメラとマイクを止め、入・切をビデオ通話に引き継ぐ。失敗の文言は [StudentFlowState.errorMessage]
  Future<void> join() async {
    final joined = await ref
        .read(studentFlowProvider.notifier)
        .joinLiveLesson();
    if (!joined || !ref.mounted) return;
    _stopDevices();
    state = state.copyWith(
      carried: (
        camera: state.cameraOn && state.cameraReady,
        mic: state.micOn && state.micReady,
      ),
      tonePlaying: false,
      micLevel: 0,
      micHeard: false,
    );
  }

  /// 戻る（確認しない）。カメラとマイクをすぐ止めて予約の確定へ
  void leave() {
    _stopDevices();
    ref.read(studentFlowProvider.notifier).moveTo(FlowStep.bookingPending);
  }

  /// アプリが裏に回った。カメラ・マイク・試し音を止める（入・切は変えない。前に戻ったら [start] で入のものだけ開き直す）
  void pause() {
    _stopDevices();
    state = PrejoinCheckState(
      // 入にしていて使っていたものだけ、前に戻ったら開き直す（それまでは許可を待つ表示）
      camera: state.cameraReady && state.cameraOn ? null : state.camera,
      mic: state.micReady && state.micOn ? null : state.mic,
      cameraOn: state.cameraOn,
      micOn: state.micOn,
      carried: state.carried,
    );
  }

  /// 画面を片付ける（ほかの段に移った）。カメラ・マイク・試し音を止める。片付けの途中で呼ぶので状態は変えない
  void release() => _stopDevices();

  void _stopDevices() {
    _toneGeneration++;
    _stopMic();
    _media.stopAll();
  }

  void _stopMic() {
    _levels?.cancel();
    _levels = null;
    _silentSamples = 0;
    _media.stopMic();
  }

  /// マイクが開いていると、1 秒に 10 回まで音の大きさが届く。10 段にまとめ、届いた・届いていないは 2 秒続いたら替える
  void _listenLevels() {
    _levels?.cancel();
    _silentSamples = 0;
    _levels = _media.micLevels.listen((level) {
      final segments = (level.clamp(0.0, 1.0) * 10).round();
      final heard = segments >= _heardSegments;
      _silentSamples = heard ? 0 : _silentSamples + 1;
      state = state.copyWith(
        micLevel: segments,
        micHeard: heard
            ? true
            : _silentSamples >= _silentSamplesToReset
            ? false
            : state.micHeard,
      );
    });
  }
}
