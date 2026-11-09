import { describe, expect, test } from "vitest";

import { clockOffsetMs } from "@/api/serverClock";

describe("clockOffsetMs", () => {
  const SERVER = Date.parse("2026-11-08T03:00:00Z");
  const DATE = "Sun, 08 Nov 2026 03:00:00 GMT";

  test("送った時刻と受け取った時刻の中間を端末の時刻とみなし、サーバー − 端末のずれを出す", () => {
    // 端末は 2 分遅れ。往復 400ms の中間がサーバーの時刻にあたる
    expect(clockOffsetMs(DATE, SERVER - 120_000 - 200, SERVER - 120_000 + 200)).toBe(120_000);
    // 端末は 2 分進み
    expect(clockOffsetMs(DATE, SERVER + 120_000, SERVER + 120_000)).toBe(-120_000);
  });

  test("Date がない・読めないときは undefined（端末の時計のまま）", () => {
    expect(clockOffsetMs(null, SERVER, SERVER)).toBeUndefined();
    expect(clockOffsetMs(undefined, SERVER, SERVER)).toBeUndefined();
    expect(clockOffsetMs("", SERVER, SERVER)).toBeUndefined();
    expect(clockOffsetMs("not a date", SERVER, SERVER)).toBeUndefined();
  });
});
