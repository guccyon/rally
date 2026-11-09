// 入室前チェックが触るブラウザーのカメラ・マイク・スピーカー（getUserMedia・enumerateDevices・Web Audio・試し音）の 1 か所。
// 画面はここの LocalMedia だけを使い、テストでは作り物に差し替える。
// 映像と音はブラウザーの中で見せて聞かせるだけ。録画・録音・静止画にせず、サーバーや通話サービスに送らない
// （glossary の prejoin_check、governance の lint の media-no-record）。

/** カメラかマイク。 */
export type MediaKind = "camera" | "mic";

/** 選べる機器（ブラウザーが返す ID とラベル）。 */
export type MediaDevice = { deviceId: string; label: string };

export type DeviceList = { cameras: MediaDevice[]; mics: MediaDevice[]; speakers: MediaDevice[] };

/** 開けなかった理由。denied は許可を断られた、none は機器がない、busy はほかのアプリが使っているなどで開けない。 */
export type OpenFailure = "denied" | "none" | "busy";

export type OpenResult = { ok: true; stream: MediaStream } | { ok: false; reason: OpenFailure };

/** マイクの音の大きさを読む口。read は 0〜1。 */
export type LevelProbe = { read: () => number; close: () => void };

/** 鳴らした試し音。done は鳴り終わった（ended）・止めた（stopped）・鳴らせなかった（failed）で解決する。 */
export type TestSound = { done: Promise<"ended" | "stopped" | "failed">; stop: () => void };

export type LocalMedia = {
  /** カメラかマイクを開く（deviceId を省くとブラウザーの既定の機器）。はじめてならブラウザーが許可を求める。 */
  open: (kind: MediaKind, deviceId?: string) => Promise<OpenResult>;
  /** 開いた流れを止める（カメラ・マイクの使用中の印も消える）。 */
  stop: (stream: MediaStream) => void;
  /** 機器の一覧。ラベルは許可の後に読める。 */
  listDevices: () => Promise<DeviceList>;
  /** 試し音の出力先を選べるブラウザーか（setSinkId があるか）。 */
  canChooseSpeaker: () => boolean;
  probeLevel: (stream: MediaStream) => LevelProbe;
  /** 試し音を 1 回鳴らす（speakerId を省くとブラウザーの既定の出力先）。 */
  playTestSound: (speakerId?: string) => TestSound;
  /** 機器のつなぎ外し（devicechange）を聞く。戻り値で聞くのをやめる。 */
  onDeviceChange: (listener: () => void) => () => void;
};

/** 同梱の 3 秒の試し音（同じオリジン。public/sounds/README.md）。 */
export const TEST_SOUND_URL = "/sounds/speaker-test.wav";

function mediaDevices(): MediaDevices | undefined {
  return typeof navigator === "undefined" ? undefined : navigator.mediaDevices;
}

function failure(error: unknown): OpenFailure {
  const name = error instanceof DOMException || error instanceof Error ? error.name : "";
  if (name === "NotAllowedError" || name === "SecurityError") return "denied";
  if (name === "NotFoundError" || name === "OverconstrainedError") return "none";
  return "busy";
}

// Chrome の「既定」「通信」は実際の機器の別名なので、選べる機器に数えない（既定は選択欄の先頭の行で選ぶ）。
const ALIASES = new Set(["default", "communications", ""]);

function devicesOf(all: MediaDeviceInfo[], kind: MediaDeviceKind, name: string): MediaDevice[] {
  return all
    .filter((device) => device.kind === kind && !ALIASES.has(device.deviceId))
    .map((device, index) => ({ deviceId: device.deviceId, label: device.label || `${name} ${index + 1}` }));
}

// 音の大きさ（RMS）を dB にし、-60 dB〜-10 dB を 0〜1 に当てる。
const FLOOR_DB = -60;
const CEILING_DB = -10;

type SinkableAudio = HTMLAudioElement & { setSinkId?: (sinkId: string) => Promise<void> };

/** ブラウザーの本物のカメラ・マイク・スピーカー。 */
export const browserLocalMedia: LocalMedia = {
  async open(kind, deviceId) {
    const devices = mediaDevices();
    // 安全でない接続（http）などで mediaDevices がないときは、機器がないものとして扱う
    if (!devices?.getUserMedia) return { ok: false, reason: "none" };
    const constraint = deviceId ? { deviceId: { exact: deviceId } } : true;
    try {
      const stream = await devices.getUserMedia(kind === "camera" ? { video: constraint } : { audio: constraint });
      return { ok: true, stream };
    } catch (error) {
      return { ok: false, reason: failure(error) };
    }
  },

  stop(stream) {
    for (const track of stream.getTracks()) track.stop();
  },

  async listDevices() {
    const devices = mediaDevices();
    if (!devices?.enumerateDevices) return { cameras: [], mics: [], speakers: [] };
    const all = await devices.enumerateDevices();
    return {
      cameras: devicesOf(all, "videoinput", "カメラ"),
      mics: devicesOf(all, "audioinput", "マイク"),
      speakers: devicesOf(all, "audiooutput", "スピーカー"),
    };
  },

  canChooseSpeaker() {
    return typeof HTMLMediaElement !== "undefined" && "setSinkId" in HTMLMediaElement.prototype;
  },

  probeLevel(stream) {
    if (typeof AudioContext === "undefined") return { read: () => 0, close: () => {} };
    const context = new AudioContext();
    const source = context.createMediaStreamSource(stream);
    const analyser = context.createAnalyser();
    analyser.fftSize = 1024;
    source.connect(analyser);
    void context.resume().catch(() => {});
    const samples = new Float32Array(analyser.fftSize);
    return {
      read() {
        analyser.getFloatTimeDomainData(samples);
        let sum = 0;
        for (const sample of samples) sum += sample * sample;
        const rms = Math.sqrt(sum / samples.length);
        if (rms <= 0) return 0;
        const db = 20 * Math.log10(rms);
        return Math.min(1, Math.max(0, (db - FLOOR_DB) / (CEILING_DB - FLOOR_DB)));
      },
      close() {
        source.disconnect();
        void context.close().catch(() => {});
      },
    };
  },

  playTestSound(speakerId) {
    if (typeof Audio === "undefined") return { done: Promise.resolve("failed"), stop: () => {} };
    const audio: SinkableAudio = new Audio(TEST_SOUND_URL);
    let settled = false;
    let resolveDone: (value: "ended" | "stopped" | "failed") => void = () => {};
    const done = new Promise<"ended" | "stopped" | "failed">((resolve) => {
      resolveDone = resolve;
    });
    const settle = (value: "ended" | "stopped" | "failed") => {
      if (settled) return;
      settled = true;
      if (value !== "ended") audio.pause();
      resolveDone(value);
    };
    audio.addEventListener("ended", () => settle("ended"));
    audio.addEventListener("error", () => settle("failed"));
    void (async () => {
      try {
        if (speakerId && audio.setSinkId) await audio.setSinkId(speakerId);
        // 出力先を選ぶあいだに止めたら鳴らさない
        if (settled) return;
        await audio.play();
        if (settled) audio.pause();
      } catch {
        settle("failed");
      }
    })();
    return {
      done,
      stop() {
        settle("stopped");
      },
    };
  },

  onDeviceChange(listener) {
    const devices = mediaDevices();
    if (!devices?.addEventListener) return () => {};
    devices.addEventListener("devicechange", listener);
    return () => devices.removeEventListener("devicechange", listener);
  },
};
