import type { NextConfig } from "next";

// ブラウザからは同じオリジンの /api/* を呼び、Next.js がバックエンドへ中継する（CORS を要らなくする）。
// バックエンドの場所は RALLY_API_URL（既定はバックエンドの既定の待ち受け 127.0.0.1:8080）。
const apiUrl = process.env.RALLY_API_URL ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  async rewrites() {
    return [{ source: "/api/:path*", destination: `${apiUrl}/:path*` }];
  },
};

export default nextConfig;
