import { defineConfig, devices } from "@playwright/test";

// 画面のキャプチャ（npm run screenshots）。テストではなく、PR の前後で見た目を比べるための撮影。
// 作った本番のアプリ（next build → next start）を撮る。開発サーバーは右下に開発用の印が出るので使わない。
// 撮る先を変えるときは SCREENSHOT_BASE_URL を渡すと、サーバーを立てずにそこを撮る。

const port = Number(process.env.SCREENSHOT_PORT ?? 3100);
const external = process.env.SCREENSHOT_BASE_URL;

export default defineConfig({
  testDir: ".",
  testMatch: "capture.screens.ts",
  fullyParallel: true,
  retries: 1,
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: external ?? `http://127.0.0.1:${port}`,
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    locale: "ja-JP",
    timezoneId: "Asia/Tokyo",
    colorScheme: "light",
    ...(process.env.PLAYWRIGHT_CHROMIUM_PATH
      ? { launchOptions: { executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH } }
      : {}),
  },
  webServer: external
    ? undefined
    : {
        command: `npx next start -p ${port}`,
        cwd: "..",
        url: `http://127.0.0.1:${port}`,
        reuseExistingServer: false,
        timeout: 60_000,
      },
});
