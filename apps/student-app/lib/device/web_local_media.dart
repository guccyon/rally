import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

import 'local_media.dart';

/// Web 版の [LocalMedia]。ブラウザーの getUserMedia でカメラ・マイクを開き（ブラウザーが許可を求める）、
/// 映像は video 要素で映し、音の大きさは Web Audio の AnalyserNode で計り、試し音は Web Audio で作って鳴らす。
/// stream は video 要素と AnalyserNode にだけ渡す（録らない・残さない・外へ送らない。governance の media-no-record）。
LocalMedia createLocalMedia() => _WebLocalMedia();

/// 試し音の長さ（画面の仕様の「3 秒を 1 回」）
const _toneLength = Duration(seconds: 3);

/// 音の大きさを計る間隔（level_meter の「1 秒に 10 回まで」）
const _levelInterval = Duration(milliseconds: 100);

class _WebLocalMedia implements LocalMedia {
  web.MediaStream? _camera;
  web.MediaStream? _mic;
  web.AudioContext? _audio;
  web.MediaStreamAudioSourceNode? _micSource;
  Timer? _levelTimer;
  final _levels = StreamController<double>.broadcast();

  /// カメラを開き直すたびに増やし、映像の要素を作り直す
  var _cameraGeneration = 0;

  final _toneNodes = <web.OscillatorNode>[];
  Timer? _toneTimer;
  Completer<void>? _tone;

  web.MediaDevices? get _devices {
    final navigator = web.window.navigator;
    // http（localhost 以外）ではブラウザーが mediaDevices を出さない
    if (!navigator.hasProperty('mediaDevices'.toJS).toDart) return null;
    return navigator.mediaDevices;
  }

  Future<(MediaDeviceStatus, web.MediaStream?)> _open({
    required bool video,
  }) async {
    final devices = _devices;
    if (devices == null) return (MediaDeviceStatus.denied, null);
    try {
      final stream = await devices
          .getUserMedia(
            web.MediaStreamConstraints(
              video: video ? {'facingMode': 'user'}.jsify()! : false.toJS,
              audio: (!video).toJS,
            ),
          )
          .toDart;
      return (MediaDeviceStatus.ready, stream);
    } catch (e) {
      // DOMException の name で分ける（NotFoundError は端末にない。NotAllowedError・NotReadableError などは使えない）
      final text = e.toString();
      final missing =
          text.contains('NotFoundError') ||
          text.contains('DevicesNotFoundError') ||
          text.contains('OverconstrainedError');
      return (
        missing ? MediaDeviceStatus.missing : MediaDeviceStatus.denied,
        null,
      );
    }
  }

  @override
  Future<MediaDeviceStatus> openCamera() async {
    if (_camera != null) return MediaDeviceStatus.ready;
    final (status, stream) = await _open(video: true);
    if (stream != null) {
      _camera = stream;
      _cameraGeneration++;
    }
    return status;
  }

  @override
  Future<MediaDeviceStatus> openMic() async {
    if (_mic != null) return MediaDeviceStatus.ready;
    final (status, stream) = await _open(video: false);
    if (stream == null) return status;
    _mic = stream;
    final audio = _audioContext();
    final analyser = audio.createAnalyser()..fftSize = 1024;
    _micSource = audio.createMediaStreamSource(stream)..connect(analyser);
    final samples = JSFloat32Array.withLength(analyser.fftSize);
    _levelTimer = Timer.periodic(_levelInterval, (_) {
      analyser.getFloatTimeDomainData(samples);
      var sum = 0.0;
      for (final s in samples.toDart) {
        sum += s * s;
      }
      final rms = math.sqrt(sum / analyser.fftSize);
      // -60dB を 0、-10dB を 1 にする（ふつうの声が中ほどに来る）
      final db = 20 * math.log(math.max(rms, 1e-8)) / math.ln10;
      _levels.add(((db + 60) / 50).clamp(0.0, 1.0));
    });
    return status;
  }

  web.AudioContext _audioContext() {
    final audio = _audio ??= web.AudioContext();
    // Safari は押した操作の中でないと止まったまま始まるので、使うたびに起こす
    audio.resume();
    return audio;
  }

  void _stopTracks(web.MediaStream stream) {
    for (final track in stream.getTracks().toDart) {
      track.stop();
    }
  }

  @override
  void stopCamera() {
    final camera = _camera;
    if (camera == null) return;
    _stopTracks(camera);
    _camera = null;
  }

  @override
  void stopMic() {
    _levelTimer?.cancel();
    _levelTimer = null;
    _micSource?.disconnect();
    _micSource = null;
    final mic = _mic;
    if (mic == null) return;
    _stopTracks(mic);
    _mic = null;
  }

  @override
  Widget? cameraView() {
    final camera = _camera;
    if (camera == null) return null;
    // 映りの枠（RallyCameraPreview）が cover で切り抜くので、4:3 の大きさを持たせる
    return SizedBox(
      width: 640,
      height: 480,
      child: HtmlElementView.fromTagName(
        key: ValueKey('camera-$_cameraGeneration'),
        tagName: 'video',
        onElementCreated: (element) {
          final video = element as web.HTMLVideoElement
            ..autoplay = true
            ..muted = true
            ..playsInline = true
            ..srcObject = camera;
          video.style
            ..width = '100%'
            ..height = '100%'
            ..objectFit = 'cover'
            ..pointerEvents = 'none';
        },
      ),
    );
  }

  @override
  Stream<double> get micLevels => _levels.stream;

  @override
  Future<void> playTestTone() async {
    stopTestTone();
    final audio = _audioContext();
    await audio.resume().toDart;
    final start = audio.currentTime;
    // 0.5 秒ごとに 2 つの高さを交互に 6 回（3 秒）。音の始まりと終わりを短く絞って、ぷつっと鳴らないようにする
    for (var i = 0; i < 6; i++) {
      final at = start + i * 0.5;
      final gain = audio.createGain()..connect(audio.destination);
      gain.gain
        ..setValueAtTime(0, at)
        ..linearRampToValueAtTime(0.25, at + 0.03)
        ..linearRampToValueAtTime(0, at + 0.4);
      final oscillator = audio.createOscillator()
        ..type = 'sine'
        ..connect(gain);
      oscillator.frequency.value = i.isEven ? 660 : 880;
      oscillator
        ..start(at)
        ..stop(at + 0.45);
      _toneNodes.add(oscillator);
    }
    final tone = _tone = Completer<void>();
    _toneTimer = Timer(_toneLength, stopTestTone);
    return tone.future;
  }

  @override
  void stopTestTone() {
    _toneTimer?.cancel();
    _toneTimer = null;
    for (final node in _toneNodes) {
      try {
        node.stop();
      } catch (_) {
        // もう止まっている
      }
      node.disconnect();
    }
    _toneNodes.clear();
    final tone = _tone;
    _tone = null;
    if (tone != null && !tone.isCompleted) tone.complete();
  }

  @override
  bool get canOpenSettings => false;

  @override
  Future<void> openSettings() async {}

  @override
  void stopAll() {
    stopCamera();
    stopMic();
    stopTestTone();
  }
}
