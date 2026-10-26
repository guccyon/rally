import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  // node 環境のテスト（apiSchema.test.ts）には DOM がない。
  if (typeof window === "undefined") return;
  cleanup();
  window.localStorage.clear();
});
