import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";

import { studentNamesFromBookings } from "./studentNames";

type CoachVideo = Schemas["CoachVideo"];
type CoachBooking = Schemas["CoachBooking"];
type MessageThread = Schemas["MessageThread"];

export type PendingTasksApi = Pick<
  CoachApi,
  "listMyCoachVideos" | "listMyCoachBookings" | "listMessageThreads"
>;

/** コーチの対応待ち。一覧の API ごとに読めたか・読めなかったかを持つ（1 つが失敗しても残りは出す）。 */
export type PendingTasks = {
  /** 添削待ちの動画（API の順＝送られた時刻の古い順）。 */
  videos: Result<CoachVideo[]>;
  /** 確定待ち（`pending`）でまだ終わっていない予約（API の順＝開始時刻の古い順）。 */
  bookings: Result<CoachBooking[]>;
  /** 未読のあるスレッド（API の順＝最後の出来事の新しい順）。 */
  threads: Result<MessageThread[]>;
  /** 生徒の ID → 表示名。スレッドは生徒の ID しか持たないので、予約の一覧の表示名から引く。 */
  names: Map<string, string>;
};

/** 一覧の API の結果から、対応待ちの項目だけを API の順のまま残す。 */
function select<R, T>(
  response: Result<R>,
  items: (data: R) => T[],
  pick: (item: T) => boolean,
): Result<T[]> {
  return response.ok ? { ok: true, data: items(response.data).filter(pick) } : response;
}

/**
 * 今の API（listMyCoachVideos / listMyCoachBookings / listMessageThreads）から、コーチの対応待ちを集める。
 * 確定待ちの予約は、終了時刻が `now` より後のものだけ（終わった予約は対応できないので数えない）。
 */
export async function loadPendingTasks(api: PendingTasksApi, now: number): Promise<PendingTasks> {
  const [videos, bookings, threads] = await Promise.all([
    api.listMyCoachVideos(),
    api.listMyCoachBookings(),
    api.listMessageThreads(),
  ]);
  const names = bookings.ok ? studentNamesFromBookings(bookings.data.bookings) : new Map<string, string>();
  return {
    videos: select(
      videos,
      (data) => data.videos,
      (video) => video.review_status === "in_progress",
    ),
    bookings: select(
      bookings,
      (data) => data.bookings,
      (item) => item.booking.status === "pending" && new Date(item.slot.ends_at).getTime() > now,
    ),
    threads: select(
      threads,
      (data) => data.threads,
      (thread) => thread.unread_count > 0,
    ),
    names,
  };
}

/** 未対応の件数（添削待ちの動画＋確定待ちの予約＋未読のあるスレッド）。読めなかった一覧は 0 件として数える。 */
export function countPendingTasks(tasks: PendingTasks): number {
  const size = (part: Result<unknown[]>) => (part.ok ? part.data.length : 0);
  return size(tasks.videos) + size(tasks.bookings) + size(tasks.threads);
}
