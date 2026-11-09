import { vi } from "vitest";

import type { LocalMedia, MediaDevice, MediaKind, OpenFailure } from "@/components/coach/localMedia";

// 入室前チェックのテスト用の作り物のカメラ・マイク・スピーカー（src/components/coach/localMedia.ts の LocalMedia）。
// jsdom には MediaStream がないので、止めたかだけを持つ流れを返す。

export type FakeStream = MediaStream & { kind: MediaKind; deviceId: string; stopped: boolean };

type Outcome = "ok" | OpenFailure;

export type FakeMediaOptions = {
  camera?: Outcome;
  mic?: Outcome;
  cameras?: MediaDevice[];
  mics?: MediaDevice[];
  speakers?: MediaDevice[];
  canChooseSpeaker?: boolean;
  /** マイクの音の大きさ（0〜1） */
  level?: number;
  /** 試し音の結果。manual は止めるまで鳴り続ける */
  sound?: "ended" | "failed" | "manual";
};

export function fakeLocalMedia(options: FakeMediaOptions = {}) {
  const state = {
    camera: options.camera ?? "ok",
    mic: options.mic ?? "ok",
    cameras: options.cameras ?? [{ deviceId: "cam-1", label: "内蔵カメラ" }],
    mics: options.mics ?? [{ deviceId: "mic-1", label: "内蔵マイク" }],
    speakers: options.speakers ?? [{ deviceId: "spk-1", label: "内蔵スピーカー" }],
    level: options.level ?? 0,
    /** 開けない機器の ID（選んだら busy） */
    broken: new Set<string>(),
  };
  const streams: FakeStream[] = [];
  const sounds: { speakerId?: string; stopped: boolean; finish: (result: "ended" | "failed") => void }[] = [];
  const probes: { closed: boolean }[] = [];
  let deviceChange: (() => void) | null = null;

  function makeStream(kind: MediaKind, deviceId: string): FakeStream {
    const stream = {
      kind,
      deviceId,
      stopped: false,
      getTracks: () => [{ readyState: stream.stopped ? "ended" : "live", stop: () => (stream.stopped = true) }],
    } as unknown as FakeStream;
    streams.push(stream);
    return stream;
  }

  const media: LocalMedia = {
    open: vi.fn(async (kind: MediaKind, deviceId?: string) => {
      if (deviceId && state.broken.has(deviceId)) return { ok: false as const, reason: "busy" as const };
      const outcome = state[kind];
      if (outcome !== "ok") return { ok: false as const, reason: outcome };
      const list = kind === "camera" ? state.cameras : state.mics;
      return { ok: true as const, stream: makeStream(kind, deviceId ?? list[0]?.deviceId ?? "") };
    }),
    stop: (stream) => {
      (stream as FakeStream).stopped = true;
    },
    listDevices: async () => ({
      cameras: state.camera === "ok" ? state.cameras : [],
      mics: state.mic === "ok" ? state.mics : [],
      speakers: state.speakers,
    }),
    canChooseSpeaker: () => options.canChooseSpeaker ?? true,
    probeLevel: () => {
      const probe = { closed: false };
      probes.push(probe);
      return {
        read: () => state.level,
        close: () => {
          probe.closed = true;
        },
      };
    },
    playTestSound: vi.fn((speakerId?: string) => {
      let resolve: (value: "ended" | "stopped" | "failed") => void = () => {};
      const done = new Promise<"ended" | "stopped" | "failed">((r) => (resolve = r));
      const sound = { speakerId, stopped: false, finish: (result: "ended" | "failed") => resolve(result) };
      sounds.push(sound);
      if (options.sound === "failed") resolve("failed");
      if (options.sound === "ended") resolve("ended");
      return {
        done,
        stop: () => {
          sound.stopped = true;
          resolve("stopped");
        },
      };
    }),
    onDeviceChange: (listener) => {
      deviceChange = listener;
      return () => {
        if (deviceChange === listener) deviceChange = null;
      };
    },
  };

  return {
    media,
    state,
    streams,
    sounds,
    probes,
    /** 開いていて止めていない流れ */
    live: () => streams.filter((stream) => !stream.stopped),
    /** 機器のつなぎ外しを起こす */
    changeDevices: () => deviceChange?.(),
  };
}
