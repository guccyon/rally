import 'dart:async';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:audioplayers/audioplayers.dart';
import 'package:camera/camera.dart';
import 'package:flutter/widgets.dart';
import 'package:record/record.dart';
import 'package:url_launcher/url_launcher.dart';

import 'local_media.dart';

/// 試し音の長さ（画面の仕様の「3 秒を 1 回」。同梱の assets/sounds/test_tone.wav の長さ）
const _toneLength = Duration(seconds: 3);

/// 音の大きさを計る間隔（level_meter の「1 秒に 10 回まで」）
const _levelInterval = Duration(milliseconds: 100);

/// マイクの音を受け取る形（16bit・1ch。音の大きさを計るだけなので低めの標本化で足りる）
const _micSampleRate = 16000;

/// iOS の音のセッション。マイクを開いたまま試し音を鳴らすので playAndRecord にし、
/// 受話口ではなくスピーカー（イヤホン・Bluetooth をつないでいればそちら）から鳴らす。record の既定の組と同じ
final _audioContext = AudioContext(
  iOS: AudioContextIOS(
    category: AVAudioSessionCategory.playAndRecord,
    options: const {
      AVAudioSessionOptions.defaultToSpeaker,
      AVAudioSessionOptions.allowBluetooth,
      AVAudioSessionOptions.allowBluetoothA2DP,
    },
  ),
);

/// iOS 版の [LocalMedia]。camera で前のカメラを開き（はじめては端末が許可を求める）、record でマイクの音を受け取って大きさを計り、
/// audioplayers で同梱の試し音を鳴らす。許可を断ったときはこのアプリの設定（app-settings:）を開ける。
/// カメラは映すだけ（撮影・録画をしない）、マイクの音は大きさを計ったら捨てる（ファイルにしない）。録らない・残さない・外へ送らない（governance の media-no-record）。
class IosLocalMedia implements LocalMedia {
  CameraController? _camera;

  /// 開いている途中のカメラ（二重に開かない）
  Future<MediaDeviceStatus>? _openingCamera;

  /// カメラを止めるたびに増やす（開いている途中で止めたら、開けたカメラをすぐ閉じる）
  var _cameraGeneration = 0;

  AudioRecorder? _recorder;
  StreamSubscription<Uint8List>? _micSamples;
  Timer? _levelTimer;
  final _meter = MicLevelMeter();
  final _levels = StreamController<double>.broadcast();

  /// 開いている途中のマイク（二重に開かない）
  Future<MediaDeviceStatus>? _openingMic;

  /// マイクを止めるたびに増やす（開いている途中で止めたら、受け取り始めた音をすぐ止める）
  var _micGeneration = 0;

  AudioPlayer? _player;
  StreamSubscription<void>? _toneDone;
  Timer? _toneTimer;
  Completer<void>? _tone;

  @override
  Future<MediaDeviceStatus> openCamera() {
    if (_camera != null) return Future.value(MediaDeviceStatus.ready);
    return _openingCamera ??= _openCamera().whenComplete(() {
      _openingCamera = null;
    });
  }

  Future<MediaDeviceStatus> _openCamera() async {
    final generation = _cameraGeneration;
    final List<CameraDescription> cameras;
    try {
      cameras = await availableCameras();
    } on CameraException {
      return MediaDeviceStatus.missing;
    }
    if (cameras.isEmpty) return MediaDeviceStatus.missing;
    final camera = cameras.firstWhere(
      (c) => c.lensDirection == CameraLensDirection.front,
      orElse: () => cameras.first,
    );
    // 音は撮らない（マイクは record で別に開く）
    final controller = CameraController(
      camera,
      ResolutionPreset.medium,
      enableAudio: false,
    );
    try {
      await controller.initialize();
    } on CameraException {
      await controller.dispose();
      // 許可がない（CameraAccessDenied・CameraAccessDeniedWithoutPrompt・CameraAccessRestricted）ほか、
      // ほかのアプリが使っていて開けないときも、使えないとして扱う
      return MediaDeviceStatus.denied;
    }
    if (generation != _cameraGeneration) {
      // 開いている途中で止められた（画面を離れた・裏に回った）
      await controller.dispose();
      return MediaDeviceStatus.ready;
    }
    _camera = controller;
    return MediaDeviceStatus.ready;
  }

  @override
  Future<MediaDeviceStatus> openMic() {
    if (_recorder != null) return Future.value(MediaDeviceStatus.ready);
    return _openingMic ??= _openMic().whenComplete(() {
      _openingMic = null;
    });
  }

  Future<MediaDeviceStatus> _openMic() async {
    final generation = _micGeneration;
    final recorder = AudioRecorder();
    try {
      // はじめては端末が許可を求める。断られていれば false
      if (!await recorder.hasPermission()) {
        await recorder.dispose();
        return MediaDeviceStatus.denied;
      }
      if ((await recorder.listInputDevices()).isEmpty) {
        await recorder.dispose();
        return MediaDeviceStatus.missing;
      }
      // ファイルにせず、音を受け取って大きさを計るだけ
      final samples = await recorder.startStream(
        const RecordConfig(
          encoder: AudioEncoder.pcm16bits,
          sampleRate: _micSampleRate,
          numChannels: 1,
        ),
      );
      if (generation != _micGeneration) {
        await recorder.dispose();
        return MediaDeviceStatus.ready;
      }
      _recorder = recorder;
      _meter.reset();
      _micSamples = samples.listen(_meter.add);
      _levelTimer = Timer.periodic(_levelInterval, (_) {
        final level = _meter.take();
        if (level != null) _levels.add(level);
      });
      return MediaDeviceStatus.ready;
    } on Object {
      // ほかのアプリ（通話など）が使っていて開けない
      await recorder.dispose();
      return MediaDeviceStatus.denied;
    }
  }

  @override
  void stopCamera() {
    _cameraGeneration++;
    final camera = _camera;
    _camera = null;
    camera?.dispose();
  }

  @override
  void stopMic() {
    _micGeneration++;
    _levelTimer?.cancel();
    _levelTimer = null;
    _micSamples?.cancel();
    _micSamples = null;
    _meter.reset();
    final recorder = _recorder;
    _recorder = null;
    // dispose は録っている途中なら止めてから閉じる
    recorder?.dispose();
  }

  @override
  Widget? cameraView() {
    final camera = _camera;
    if (camera == null || !camera.value.isInitialized) return null;
    // previewSize は横長（端末を横にした向き）で届く。アプリは縦で使うので縦長の大きさを持たせ、映りの枠（RallyCameraPreview）が cover で切り抜く
    final size = camera.value.previewSize ?? const Size(640, 480);
    final preview = SizedBox(
      width: size.shortestSide,
      height: size.longestSide,
      child: CameraPreview(camera),
    );
    // iOS の camera は前のカメラの映像をはじめから左右反転して出す。枠が左右反転するので、ここで戻して二重に反転しない
    return camera.description.lensDirection == CameraLensDirection.front
        ? Transform.flip(flipX: true, child: preview)
        : preview;
  }

  @override
  Stream<double> get micLevels => _levels.stream;

  @override
  Future<void> playTestTone() async {
    stopTestTone();
    final player = _player ??= AudioPlayer();
    final tone = _tone = Completer<void>();
    _toneDone = player.onPlayerComplete.listen((_) => stopTestTone());
    // 鳴り終わりが届かなくても、試し音の長さで終える
    _toneTimer = Timer(_toneLength + const Duration(seconds: 1), stopTestTone);
    try {
      await player.play(
        AssetSource('sounds/test_tone.wav'),
        ctx: _audioContext,
      );
    } on Object {
      if (identical(_tone, tone)) {
        _clearTone();
        _tone = null;
      }
      rethrow;
    }
    return tone.future;
  }

  void _clearTone() {
    _toneTimer?.cancel();
    _toneTimer = null;
    _toneDone?.cancel();
    _toneDone = null;
  }

  @override
  void stopTestTone() {
    _clearTone();
    final tone = _tone;
    _tone = null;
    if (tone == null) return;
    _player?.stop();
    if (!tone.isCompleted) tone.complete();
  }

  @override
  bool get canOpenSettings => true;

  @override
  Future<void> openSettings() async {
    // iOS の UIApplication.openSettingsURLString（このアプリの設定）
    await launchUrl(Uri.parse('app-settings:'));
  }

  @override
  void stopAll() {
    stopCamera();
    stopMic();
    stopTestTone();
  }
}

/// マイクから届いた音（16bit・リトルエンディアンの PCM）をためて、[take] で 0.0〜1.0 の音の大きさにする。
/// 目盛りは Web 版と同じ（-60dB を 0、-10dB を 1。ふつうの声が中ほどに来る）。音はためた二乗の和だけを持ち、音そのものは残さない。
class MicLevelMeter {
  var _sumOfSquares = 0.0;
  var _count = 0;

  /// 届いた音を足す（端の 1 バイトは捨てる）
  void add(Uint8List pcm16le) {
    final data = ByteData.sublistView(pcm16le);
    for (var i = 0; i + 1 < pcm16le.length; i += 2) {
      final s = data.getInt16(i, Endian.little) / 32768;
      _sumOfSquares += s * s;
    }
    _count += pcm16le.length ~/ 2;
  }

  /// 前に取ってから届いた音の大きさ。届いていなければ null
  double? take() {
    if (_count == 0) return null;
    final rms = math.sqrt(_sumOfSquares / _count);
    reset();
    final db = 20 * math.log(math.max(rms, 1e-8)) / math.ln10;
    return ((db + 60) / 50).clamp(0.0, 1.0);
  }

  /// ためた音を捨てる
  void reset() {
    _sumOfSquares = 0;
    _count = 0;
  }
}
