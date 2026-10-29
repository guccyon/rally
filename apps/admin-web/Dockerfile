# admin-web の出荷用イメージ（factory-engine の ADR 0012）。工場の shipper が Release の先頭のコミットから作る。
#   docker build -t rally-admin-web:<版> apps/admin-web
# /api/* の中継先（next.config.ts の RALLY_API_URL）は作るときに決まるので、出荷用の compose の backend を既定にする
# ベースのイメージは引数で差し替えられる（backend の Dockerfile と同じ）
ARG NODE_IMAGE=node:22-bookworm-slim
FROM ${NODE_IMAGE} AS build
USER root
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
ARG RALLY_API_URL=http://backend:8080
ENV RALLY_API_URL=$RALLY_API_URL NEXT_TELEMETRY_DISABLED=1
RUN npm run build && npm prune --omit=dev

FROM ${NODE_IMAGE}
USER root
WORKDIR /app
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 PORT=3000 HOSTNAME=0.0.0.0
COPY --from=build /app/package.json /app/next.config.ts ./
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/.next ./.next
EXPOSE 3000
USER node
ENTRYPOINT []
CMD ["npx", "next", "start"]
