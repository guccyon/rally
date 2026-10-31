import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { test } from "@playwright/test";

import { NOW } from "./fixtures";
import { mockApi } from "./mockApi";
import { SCREENS } from "./screens";

// 画面ごとに 1 枚、全体のキャプチャを SCREENSHOT_DIR（既定 screenshots/out）に <id>.png で書く。
// 横に <id>.json（題名・取れたか・API の応答がなかった操作）を置き、比較の道具（tools/screenshots）が読む。

const OUT = process.env.SCREENSHOT_DIR ?? join(process.cwd(), "screenshots", "out");
const SESSION_KEY = "rally.admin-web.session";

mkdirSync(OUT, { recursive: true });

for (const screen of SCREENS) {
  test(screen.id, async ({ page }) => {
    const meta = { id: screen.id, title: screen.title, ok: false, unmatched: [] as string[] };
    const writeMeta = () => writeFileSync(join(OUT, `${screen.id}.json`), JSON.stringify(meta, null, 2));
    writeMeta();

    await page.clock.setFixedTime(NOW);
    meta.unmatched = await mockApi(page);
    if (screen.role) {
      const session = JSON.stringify({ role: screen.role, token: `mock.${screen.role}.screenshot` });
      await page.addInitScript(([key, value]) => window.localStorage.setItem(key!, value!), [SESSION_KEY, session]);
    }
    await page.goto(screen.path);
    await page.waitForLoadState("networkidle");
    if (screen.open) {
      await screen.open(page);
      await page.waitForLoadState("networkidle");
    }
    // 画面の読み込み中の表示が消えるのを待つ（API の応答を描き終えるまで）
    await page.waitForFunction(() => !document.body.innerText.includes("読み込み中"), undefined, { timeout: 5_000 }).catch(() => {});
    await page.screenshot({ path: join(OUT, `${screen.id}.png`), fullPage: true, animations: "disabled", caret: "hide" });
    meta.ok = true;
    writeMeta();
  });
}
