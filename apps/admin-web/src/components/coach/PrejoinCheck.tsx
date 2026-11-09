"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import { formatDateTime, formatTime } from "@/components/labels";
import { Button } from "@/components/ui/Button";
import { CameraPreview } from "@/components/ui/CameraPreview";
import { JoinBar } from "@/components/ui/JoinBar";
import { LevelMeter, LEVEL_METER_SEGMENTS } from "@/components/ui/LevelMeter";
import { MediaToggle } from "@/components/ui/MediaToggle";
import { Notice } from "@/components/ui/Notice";
import { Select } from "@/components/ui/Select";

import { bookingLessonMenuLabel } from "./bookingLessonMenu";
import type { DeviceList, LocalMedia, MediaKind, OpenFailure } from "./localMedia";
import { secondsUntilStart } from "./prejoinWindow";

type CoachBooking = Schemas["CoachBooking"];

/** 入室したときに通話へ引き継ぐカメラ・マイクの入・切（使えない・ないものは切）。 */
export type PrejoinMediaState = { camera: boolean; mic: boolean };

/** カメラかマイクの 1 つの状態。live は使っている、off は切った、waiting は許可を待っている。 */
type TrackStatus = "waiting" | "live" | "off" | OpenFailure;

type Track = {
  status: TrackStatus;
  stream: MediaStream | null;
  /** 選んだ機器（"" はブラウザーの既定） */
  deviceId: string;
  /** 選んだ機器に切り替えられなかったとき */
  error?: string;
  /** 使っていた機器が外れたとき */
  helper?: string;
};

const KIND_NAMES: Record<MediaKind, string> = { camera: "カメラ", mic: "マイク" };

const SELECT_FAILED: Record<MediaKind, string> = {
  camera: "このカメラを使えませんでした。ほかのアプリが使っていないか確かめてください。",
  mic: "このマイクを使えませんでした。ほかのアプリが使っていないか確かめてください。",
};

/**
 * カメラかマイクの 1 つを開き・切り替え・止める。画面を離れたら（アンマウント）必ず止める。
 * 開いている途中で画面を離れた・新しく開き直したときは、遅れて開いた流れをすぐ止める。
 */
function useLocalTrack(media: LocalMedia, kind: MediaKind) {
  const [track, setTrack] = useState<Track>({ status: "waiting", stream: null, deviceId: "" });
  const streamRef = useRef<MediaStream | null>(null);
  const requestRef = useRef(0);

  const release = useCallback(() => {
    if (streamRef.current) media.stop(streamRef.current);
    streamRef.current = null;
  }, [media]);

  /** deviceId を開く。古い要求・画面を離れたあとの結果は捨てて流れを止め、null を返す。 */
  const acquire = useCallback(
    async (deviceId: string) => {
      const request = ++requestRef.current;
      const result = await media.open(kind, deviceId || undefined);
      if (request !== requestRef.current) {
        if (result.ok) media.stop(result.stream);
        return null;
      }
      return result;
    },
    [media, kind],
  );

  /** はじめて開く・確かめ直す・既定の機器に戻す。開けなければ理由の状態にする。 */
  const start = useCallback(
    async (deviceId: string, helper?: string) => {
      const result = await acquire(deviceId);
      if (!result) return;
      release();
      if (result.ok) {
        streamRef.current = result.stream;
        setTrack({ status: "live", stream: result.stream, deviceId, helper });
      } else {
        setTrack({ status: result.reason, stream: null, deviceId: "" });
      }
    },
    [acquire, release],
  );

  const toggle = useCallback(async () => {
    if (track.status === "live") {
      requestRef.current++;
      release();
      setTrack((current) => ({ ...current, status: "off", stream: null, error: undefined }));
    } else if (track.status === "off") {
      await start(track.deviceId);
    }
  }, [track.status, track.deviceId, release, start]);

  /** 使う機器を選ぶ。切っているときは次に入れたときに使う。切り替えられなければ前の機器のまま。 */
  const select = useCallback(
    async (deviceId: string) => {
      if (track.status !== "live") {
        setTrack((current) => ({ ...current, deviceId, error: undefined, helper: undefined }));
        return;
      }
      const result = await acquire(deviceId);
      if (!result) return;
      if (result.ok) {
        release();
        streamRef.current = result.stream;
        setTrack({ status: "live", stream: result.stream, deviceId });
      } else {
        setTrack((current) => ({ ...current, error: SELECT_FAILED[kind], helper: undefined }));
      }
    },
    [track.status, acquire, release, kind],
  );

  // 画面を離れたら、開いている途中のものも含めて止める
  useEffect(
    () => () => {
      requestRef.current++;
      release();
    },
    [release],
  );

  return { track, start, toggle, select };
}

/** 許可を断った・ない・使えないときの知らせ（文言は specs/ux/issue-1040.yaml の coachPrejoinCheck の more.copy）。 */
function deviceNotices(camera: TrackStatus, mic: TrackStatus): { message: string; note: string }[] {
  const notices: { message: string; note: string }[] = [];
  const both = (status: TrackStatus) => camera === status && mic === status;
  const one = (status: TrackStatus): MediaKind | null =>
    camera === status ? "camera" : mic === status ? "mic" : null;

  if (both("denied")) {
    notices.push({
      message: "カメラとマイクが使えません。ブラウザーの設定で許可すると確かめられます。",
      note: "アドレスバーの左の鍵のアイコンから、カメラとマイクを「許可」にしてください。許可しなくても、開始時刻から入室できます。",
    });
  } else {
    const denied = one("denied");
    if (denied) {
      const name = KIND_NAMES[denied];
      notices.push({
        message: `${name}が使えません。ブラウザーの設定で許可すると確かめられます。`,
        note: `アドレスバーの左の鍵のアイコンから、${name}を「許可」にしてください。許可しなくても、開始時刻から入室できます。`,
      });
    }
  }

  if (both("none")) {
    notices.push({
      message: "この PC にカメラとマイクが見つかりません。スピーカーは確かめられます。",
      note: "カメラとマイクをつないだら「もう一度確かめる」を押してください。なくても、開始時刻から入室できます。",
    });
  } else {
    const none = one("none");
    if (none) {
      const name = KIND_NAMES[none];
      const other = none === "camera" ? "マイク" : "カメラ";
      notices.push({
        message: `この PC に${name}が見つかりません。${other}とスピーカーは確かめられます。`,
        note: `${name}をつないだら「もう一度確かめる」を押してください。${name}なしでも、開始時刻から入室できます。`,
      });
    }
  }

  for (const kind of ["camera", "mic"] as const) {
    if ((kind === "camera" ? camera : mic) === "busy") {
      notices.push({
        message: `${KIND_NAMES[kind]}を使えませんでした。ほかのアプリが使っていないか確かめてください。`,
        note: "使えなくても、開始時刻から入室できます。",
      });
    }
  }
  return notices;
}

const MIC_LABELS: Record<Exclude<TrackStatus, "live">, string> = {
  waiting: "マイクの許可を待っています",
  off: "マイクを切っています",
  denied: "マイクが使えないため、音の大きさを出せません。ブラウザーの設定で許可すると確かめられます。",
  none: "マイクが見つからないため、音の大きさを出せません。",
  busy: "マイクを使えないため、音の大きさを出せません。",
};

/** 音が途切れてから「まだ音が届いていません」に戻すまで（読み上げがちらつかないように）。 */
const HEARD_HOLD_MS = 2000;
/** 音の大きさの書き換えの間隔（1 秒に 10 回まで。level_meter）。 */
const LEVEL_INTERVAL_MS = 100;

/** 使っているマイクの音の大きさ（0〜10 段）と、音が届いているか。 */
function useMicLevel(media: LocalMedia, stream: MediaStream | null) {
  const [level, setLevel] = useState({ segments: 0, heard: false });
  useEffect(() => {
    if (!stream) return;
    const probe = media.probeLevel(stream);
    let lastHeard = -Infinity;
    const timer = window.setInterval(() => {
      const segments = Math.round(probe.read() * LEVEL_METER_SEGMENTS);
      const now = Date.now();
      if (segments > 0) lastHeard = now;
      setLevel({ segments, heard: now - lastHeard < HEARD_HOLD_MS });
    }, LEVEL_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      probe.close();
      setLevel({ segments: 0, heard: false });
    };
  }, [media, stream]);
  return level;
}

type SpeakerState = "idle" | "playing" | "failed";

/**
 * コーチの入室前チェック（specs/ux/issue-1040.yaml の coachPrejoinCheck）。開始 5 分前から開き、本物のカメラの映り（左右反転）・
 * マイクの音の大きさ・スピーカーの試し音を確かめ、開始時刻になったら「入室する」で onJoin を呼ぶ。
 * API は呼ばない（入室は呼ぶ側）。閉じたら（アンマウント）カメラ・マイク・試し音を必ず止める。
 */
export function PrejoinCheck({
  booking,
  media,
  now,
  onJoin,
  onClose,
}: {
  booking: CoachBooking;
  media: LocalMedia;
  /** 今の時刻（epoch ミリ秒。サーバーに合わせた時計）。開いたまま開始時刻をまたいだら「入室する」を押せるようにする */
  now: number;
  /** 入室する。失敗したら表示する文言を、成功したら null を返す（成功したら呼ぶ側がこの画面を閉じる） */
  onJoin: (state: PrejoinMediaState) => Promise<string | null>;
  onClose: () => void;
}) {
  const camera = useLocalTrack(media, "camera");
  const mic = useLocalTrack(media, "mic");
  const level = useMicLevel(media, mic.track.status === "live" ? mic.track.stream : null);
  const [devices, setDevices] = useState<DeviceList>({ cameras: [], mics: [], speakers: [] });
  const [speakerId, setSpeakerId] = useState("");
  const [speaker, setSpeaker] = useState<SpeakerState>("idle");
  const soundRef = useRef<{ stop: () => void } | null>(null);
  const [rechecking, setRechecking] = useState(false);
  const [joining, setJoining] = useState(false);
  const [joinError, setJoinError] = useState<string | undefined>();
  const aliveRef = useRef(true);

  const refreshDevices = useCallback(async () => {
    const next = await media.listDevices();
    if (aliveRef.current) setDevices(next);
    return next;
  }, [media]);

  const { start: startCamera } = camera;
  const { start: startMic } = mic;

  // 開いたらカメラとマイクを開く（ブラウザーが許可を求める）。開けたら機器の一覧（ラベル）を読み直す。
  useEffect(() => {
    aliveRef.current = true;
    void Promise.all([startCamera(""), startMic("")]).then(() => refreshDevices());
    return () => {
      aliveRef.current = false;
      soundRef.current?.stop();
      soundRef.current = null;
    };
  }, [startCamera, startMic, refreshDevices]);

  // 機器のつなぎ外し。使っていた機器が外れたら既定に戻し、断った・ないものは確かめ直す。
  const cameraTrack = camera.track;
  const micTrack = mic.track;
  const latest = useRef({ cameraTrack, micTrack, devices });
  useEffect(() => {
    latest.current = { cameraTrack, micTrack, devices };
  }, [cameraTrack, micTrack, devices]);
  useEffect(
    () =>
      media.onDeviceChange(() => {
        void refreshDevices().then((next) => {
          const pairs = [
            [latest.current.cameraTrack, next.cameras, startCamera],
            [latest.current.micTrack, next.mics, startMic],
          ] as const;
          for (const [track, list, start] of pairs) {
            if (track.status === "live") {
              const ended = track.stream?.getTracks().some((item) => item.readyState === "ended") ?? false;
              const gone = track.deviceId !== "" && !list.some((device) => device.deviceId === track.deviceId);
              if (gone) {
                // 外れた機器のラベルは前の一覧から引く
                const { cameras, mics } = latest.current.devices;
                const label =
                  [...cameras, ...mics].find((device) => device.deviceId === track.deviceId)?.label ?? "使っていた機器";
                void start("", `${label} が外れたため、既定の機器に切り替えました。`);
              } else if (ended) {
                void start("");
              }
            } else if (track.status !== "off" && track.status !== "waiting") {
              void start("");
            }
          }
        });
      }),
    [media, refreshDevices, startCamera, startMic],
  );

  async function recheck() {
    if (rechecking) return;
    setRechecking(true);
    const waits: Promise<void>[] = [];
    if (!["live", "off", "waiting"].includes(camera.track.status)) waits.push(startCamera(""));
    if (!["live", "off", "waiting"].includes(mic.track.status)) waits.push(startMic(""));
    await Promise.all(waits);
    await refreshDevices();
    if (aliveRef.current) setRechecking(false);
  }

  function toggleSpeaker() {
    if (speaker === "playing") {
      soundRef.current?.stop();
      return;
    }
    const sound = media.playTestSound(speakerId || undefined);
    soundRef.current = sound;
    setSpeaker("playing");
    void sound.done.then((result) => {
      if (!aliveRef.current || soundRef.current !== sound) return;
      soundRef.current = null;
      setSpeaker(result === "failed" ? "failed" : "idle");
    });
  }

  function selectSpeaker(deviceId: string) {
    // 鳴っているあいだに出力先を変えたら止める（次に押すと新しい出力先で鳴る）
    soundRef.current?.stop();
    setSpeakerId(deviceId);
  }

  async function join() {
    if (joining) return;
    setJoining(true);
    setJoinError(undefined);
    const error = await onJoin({
      camera: camera.track.status === "live",
      mic: mic.track.status === "live",
    });
    if (!aliveRef.current) return;
    setJoining(false);
    if (error) setJoinError(error);
  }

  const notices = deviceNotices(camera.track.status, mic.track.status);
  const remaining = secondsUntilStart(booking.slot.starts_at, now);
  const studentName = booking.student_display_name ?? "（名前を表示できません）";
  const menu = booking.booking.lesson_menu ? bookingLessonMenuLabel(booking.booking.lesson_menu) : null;
  const usable = (status: TrackStatus) => status === "live" || status === "off";
  const showMicToggle = usable(mic.track.status) || mic.track.status === "waiting";
  const showCameraToggle = usable(camera.track.status) || camera.track.status === "waiting";
  const showCameraSelect = devices.cameras.length >= 2 && usable(camera.track.status);
  const showMicSelect = devices.mics.length >= 2 && usable(mic.track.status);
  const showSpeakerSelect = media.canChooseSpeaker() && devices.speakers.length >= 2;

  return (
    <div className="space-y-6">
      <Button variant="text" onClick={onClose}>
        通話への参加に戻る
      </Button>
      <div className="space-y-1">
        <h3 className="text-xl font-black">入室前チェック</h3>
        <p>
          {formatDateTime(booking.slot.starts_at)}〜{formatTime(booking.slot.ends_at)}・生徒: {studentName}
          {menu && `・メニュー: ${menu}`}
        </p>
      </div>

      {notices.length > 0 && (
        <div className="space-y-2">
          {notices.map((notice) => (
            <div key={notice.message} className="space-y-1">
              <Notice message={notice.message} status="warning" />
              <p className="text-sm text-text-muted">{notice.note}</p>
            </div>
          ))}
          <Button variant="secondary" loading={rechecking} onClick={() => void recheck()}>
            もう一度確かめる
          </Button>
        </div>
      )}

      <div className="space-y-2">
        <CameraPreview
          state={camera.track.status === "busy" ? "denied" : camera.track.status}
          stream={camera.track.stream}
        />
        {(showMicToggle || showCameraToggle) && (
          <div className="flex max-w-128 gap-2">
            {showMicToggle && (
              <MediaToggle
                kind="mic"
                on={mic.track.status !== "off"}
                loading={mic.track.status === "waiting"}
                onToggle={() => void mic.toggle()}
                className="flex-1"
              />
            )}
            {showCameraToggle && (
              <MediaToggle
                kind="camera"
                on={camera.track.status !== "off"}
                loading={camera.track.status === "waiting"}
                onToggle={() => void camera.toggle()}
                className="flex-1"
              />
            )}
          </div>
        )}
      </div>

      <section aria-label="マイク" className="max-w-128 space-y-1">
        <h4 className="font-bold">マイク</h4>
        <LevelMeter
          level={level.segments}
          stopped={mic.track.status !== "live"}
          label={
            mic.track.status === "live"
              ? level.heard
                ? "音が届いています"
                : "まだ音が届いていません"
              : MIC_LABELS[mic.track.status]
          }
        />
        {mic.track.status === "live" && <p className="text-sm text-text-muted">声を出すとバーが動きます。</p>}
      </section>

      <section aria-label="スピーカー" className="space-y-2">
        <h4 className="font-bold">スピーカー</h4>
        <MediaToggle kind="speaker_test" on={speaker === "playing"} onToggle={toggleSpeaker} />
        {speaker === "failed" ? (
          <p role="alert" className="text-sm text-danger">
            試し音を鳴らせませんでした。PC の音量と、音の出る先を確かめてください。
          </p>
        ) : (
          <p className="text-sm text-text-muted">
            {speaker === "playing"
              ? "試し音を鳴らしています。聞こえないときは PC の音量を上げてください。"
              : "ヘッドセットをつないでいれば、ヘッドセットから鳴ります。"}
          </p>
        )}
      </section>

      {(showCameraSelect || showMicSelect || showSpeakerSelect) && (
        <section aria-label="使う機器" className="space-y-2">
          <h4 className="font-bold">使う機器</h4>
          <div className="flex flex-wrap gap-4">
            {showCameraSelect && (
              <Select
                label="カメラ"
                value={camera.track.deviceId}
                error={camera.track.error}
                helper={camera.track.helper}
                onChange={(event) => void camera.select(event.target.value)}
              >
                <option value="">ブラウザーの既定</option>
                {devices.cameras.map((device) => (
                  <option key={device.deviceId} value={device.deviceId}>
                    {device.label}
                  </option>
                ))}
              </Select>
            )}
            {showMicSelect && (
              <Select
                label="マイク"
                value={mic.track.deviceId}
                error={mic.track.error}
                helper={mic.track.helper}
                onChange={(event) => void mic.select(event.target.value)}
              >
                <option value="">ブラウザーの既定</option>
                {devices.mics.map((device) => (
                  <option key={device.deviceId} value={device.deviceId}>
                    {device.label}
                  </option>
                ))}
              </Select>
            )}
            {showSpeakerSelect && (
              <Select label="スピーカー" value={speakerId} onChange={(event) => selectSpeaker(event.target.value)}>
                <option value="">ブラウザーの既定</option>
                {devices.speakers.map((device) => (
                  <option key={device.deviceId} value={device.deviceId}>
                    {device.label}
                  </option>
                ))}
              </Select>
            )}
          </div>
          <p className="text-sm text-text-muted">
            2 つ以上あるものだけ選べます。選んだものは、このブラウザーの中だけで使います。
          </p>
        </section>
      )}

      <JoinBar
        startTime={formatTime(booking.slot.starts_at)}
        remainingSeconds={remaining}
        meta={menu ? `${studentName}・${menu}` : studentName}
        reason={
          remaining > 0
            ? undefined
            : `入室すると、${booking.student_display_name ? `${booking.student_display_name}さん` : "生徒"}とのビデオ通話が始まります。`
        }
        loading={joining}
        error={joinError}
        onJoin={() => void join()}
        secondaryAction={{ label: "チェックをやめる", onClick: onClose }}
      />
    </div>
  );
}
