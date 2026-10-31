import type { Page, Route } from "@playwright/test";

import { FIXTURES } from "./fixtures";

// ブラウザが呼ぶ /api/* を、バックエンドに届く前に決まった応答で返す（バックエンドも DB も要らない）。
// 応答のない操作は 404 not_found にする（その画面は「読めなかった」表示のままキャプチャされる）。

function matches(pattern: string, path: string): boolean {
  const re = new RegExp(`^${pattern.replace(/\{[^}]+\}/g, "[^/]+")}$`);
  return re.test(path);
}

export async function mockApi(page: Page): Promise<string[]> {
  const unmatched: string[] = [];
  await page.route("**/api/**", async (route: Route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname.replace(/^\/api/, "");
    const fixture = FIXTURES.find((f) => f.method === request.method() && matches(f.path, path));
    if (!fixture) {
      unmatched.push(`${request.method()} ${path}`);
      await route.fulfill({
        status: 404,
        contentType: "application/json",
        body: JSON.stringify({ code: "not_found", message: "no screenshot fixture" }),
      });
      return;
    }
    if (fixture.status === 204) {
      await route.fulfill({ status: 204 });
      return;
    }
    await route.fulfill({
      status: fixture.status ?? 200,
      contentType: "application/json",
      body: JSON.stringify(fixture.body),
    });
  });
  return unmatched;
}
