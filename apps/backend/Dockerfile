# backend の出荷用イメージ（factory-engine の ADR 0012）。工場の shipper が Release の先頭のコミットから作る。
#   docker build -t rally-backend:<版> apps/backend
# 起動時の環境変数は README の「起動」（DATABASE_URL・APP_VERSION など）。待ち受けは 0.0.0.0:8080
# ベースのイメージは引数で差し替えられる（取得できない端末では、手元にある同じ Debian bookworm のイメージを .factory-config/ship.yaml の build_args で渡す）
ARG BUILD_IMAGE=rust:1-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim
FROM ${BUILD_IMAGE} AS build
USER root
WORKDIR /src
COPY . .
# rust-toolchain.toml の stable を取りに行かず、イメージのツールチェーンで作る
RUN rm -f rust-toolchain.toml
RUN --mount=type=cache,target=/src/target \
    cargo build --release --locked -p server && cp target/release/server /server

FROM ${RUNTIME_IMAGE}
USER root
# curl は起動の確かめ（compose の healthcheck と shipper）に使う
RUN command -v curl >/dev/null || (apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*)
COPY --from=build /server /usr/local/bin/server
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
USER nobody
ENTRYPOINT []
CMD ["server"]
