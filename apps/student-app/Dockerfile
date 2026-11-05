# student-app の Web 版の出荷用イメージ（factory-engine の ADR 0012・ADR 0028 の touchable_env）。工場の shipper が Release の先頭のコミットから作る。
#   docker build -t rally-student-web:<版> apps/student-app
# 人がスマートフォンのブラウザで触れるように、Flutter の Web 版を作り、同じオリジンの /api/* をバックエンドへ中継する小さな node のサーバーで出す
# （CORS を要らなくする。admin-web の next.config.ts と同じ形）。
# CanvasKit などは CDN から取らずイメージに入れる（--no-web-resources-cdn。閉じた網でも開けるように）。iOS のビルドは Mac ランナー（ADR 0028 の mac_runner）の側で作る。
# ベースのイメージは引数で差し替えられる（backend・admin-web の Dockerfile と同じ。工場は手元の factory-worker:flutter で作る）
ARG FLUTTER_IMAGE=ghcr.io/cirruslabs/flutter:stable
ARG NODE_IMAGE=node:22-bookworm-slim
FROM ${FLUTTER_IMAGE} AS build
# factory-worker:flutter は worker の利用者で flutter を持つので、その利用者のまま作る（root で動かすと flutter の git が持ち主の違いで止まる）
ARG FLUTTER_USER=root
USER root
RUN mkdir -p /build && chown "${FLUTTER_USER}" /build
USER ${FLUTTER_USER}
WORKDIR /build
COPY --chown=${FLUTTER_USER} pubspec.yaml pubspec.lock ./
RUN flutter config --enable-web >/dev/null && flutter pub get
COPY --chown=${FLUTTER_USER} lib lib
COPY --chown=${FLUTTER_USER} analysis_options.yaml ./
# web/ はリポジトリに置いていない（ios/ android/ と同じ）。作るときに雛形を足す（lib は上書きしない）
RUN flutter create --platforms=web --project-name rally_student_app --org jp.rally . >/dev/null \
 && flutter build web --release --no-web-resources-cdn --dart-define=RALLY_API_URL=/api

FROM ${NODE_IMAGE}
USER root
WORKDIR /app
ENV NODE_ENV=production PORT=3000 RALLY_API_URL=http://backend:8080 WEB_ROOT=/app/web
COPY deploy/serve.mjs ./
COPY --from=build /build/build/web ./web
EXPOSE 3000
USER node
ENTRYPOINT []
CMD ["node", "serve.mjs"]
