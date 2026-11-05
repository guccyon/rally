// student-app の Web 版を出す小さなサーバー（出荷用イメージ。apps/student-app/Dockerfile）。依存なしの node だけで動く。
// - /api/* はバックエンド（RALLY_API_URL）へ中継する。/api を外したパスで渡す（admin-web の next.config.ts と同じ形）
// - それ以外は WEB_ROOT の静的ファイル。ないパスは index.html を返す（画面遷移は状態で決まるのでパスは使わないが、念のため）
// - /healthz は 200（compose の確かめ）
import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import http from "node:http";
import path from "node:path";

const root = path.resolve(process.env.WEB_ROOT ?? "build/web");
const api = new URL(process.env.RALLY_API_URL ?? "http://127.0.0.1:8080");
const port = Number(process.env.PORT ?? 3000);

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".otf": "font/otf",
  ".ttf": "font/ttf",
  ".frag": "text/plain",
};

/** /api/... をバックエンドの同じパス（/api を外す）へ。ヘッダーと本体はそのまま流す */
function proxy(req, res, rest) {
  const headers = { ...req.headers, host: api.host };
  const upstream = http.request(
    { protocol: api.protocol, hostname: api.hostname, port: api.port, method: req.method, path: rest, headers },
    (up) => {
      res.writeHead(up.statusCode ?? 502, up.headers);
      up.pipe(res);
    },
  );
  upstream.on("error", (e) => {
    if (!res.headersSent) res.writeHead(502, { "content-type": "text/plain; charset=utf-8" });
    res.end(`バックエンドにつながらない: ${e.message}`);
  });
  req.pipe(upstream);
}

/** WEB_ROOT の外は読まない。ディレクトリ・ないファイルは index.html */
async function file(urlPath) {
  const decoded = decodeURIComponent(urlPath);
  const full = path.resolve(root, `.${decoded}`);
  if (full !== root && !full.startsWith(root + path.sep)) return null;
  try {
    if ((await stat(full)).isFile()) return full;
  } catch {
    // ないパス
  }
  return path.join(root, "index.html");
}

export function handler(req, res) {
  const url = new URL(req.url ?? "/", "http://localhost");
  if (url.pathname === "/healthz") {
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("ok");
    return;
  }
  if (url.pathname === "/api" || url.pathname.startsWith("/api/")) {
    proxy(req, res, (url.pathname.slice(4) || "/") + url.search);
    return;
  }
  if (req.method !== "GET" && req.method !== "HEAD") {
    res.writeHead(405).end();
    return;
  }
  file(url.pathname).then((full) => {
    if (!full) {
      res.writeHead(404).end();
      return;
    }
    const ext = path.extname(full);
    // index.html と起動のスクリプトは版を替えたらすぐ効くよう毎回確かめさせる
    const cache = ext === ".html" || full.endsWith("flutter_bootstrap.js") ? "no-cache" : "public, max-age=3600";
    res.writeHead(200, { "content-type": TYPES[ext] ?? "application/octet-stream", "cache-control": cache });
    if (req.method === "HEAD") res.end();
    else createReadStream(full).on("error", () => res.end()).pipe(res);
  });
}

if (import.meta.url === `file://${process.argv[1]}`) {
  http.createServer(handler).listen(port, "0.0.0.0", () => {
    console.log(`student-web: ${root} を :${port} で出す（/api/* → ${api.origin}）`);
  });
}
