import type { Metadata } from "next";

import { DesignCatalog } from "@/components/ui/DesignCatalog";

export const metadata: Metadata = {
  title: "デザインシステムの部品 | Rally 管理画面",
  robots: { index: false },
};

// 本番のメニューには出さない（どこからもリンクしない）。開発と画面のキャプチャのためのページ。
export default function DesignPage() {
  return <DesignCatalog />;
}
