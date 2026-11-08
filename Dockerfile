FROM rust:1-bookworm AS wasm
WORKDIR /app
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-pack --version 0.15.0 --locked
COPY Cargo.toml Cargo.lock LICENSE ./
COPY crates ./crates
RUN wasm-pack build crates/monumla-wasm --target web --out-dir ../../src/lib/wasm/pkg --out-name monumla \
    && wasm-pack build crates/monumla-wasm --target nodejs --out-dir ../../server/wasm --out-name monumla

FROM node:24-bookworm AS build
WORKDIR /app
ENV CI=true
RUN npm install --global pnpm@11.7.0
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc ./
RUN pnpm install --frozen-lockfile
COPY . .
COPY --from=wasm /app/src/lib/wasm/pkg ./src/lib/wasm/pkg
COPY --from=wasm /app/server/wasm ./server/wasm
RUN node --input-type=module -e "import{readFileSync,writeFileSync}from'node:fs';const p='server/wasm/package.json';writeFileSync(p,JSON.stringify({...JSON.parse(readFileSync(p,'utf8')),type:'commonjs'}))" \
    && pnpm db:generate \
    && node scripts/source-archive.mjs \
    && NODE_ENV=production pnpm exec vite build \
    && pnpm prune --prod

FROM node:24-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000 DATABASE_URL=file:/app/data/monumla.db
RUN apt-get update \
    && apt-get install --yes --no-install-recommends openssl ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir /app/data \
    && chown node:node /app/data
COPY --from=build --chown=node:node /app/node_modules ./node_modules
COPY --from=build --chown=node:node /app/build ./build
COPY --from=build --chown=node:node /app/server ./server
COPY --from=build --chown=node:node /app/src/lib/platform/catalog.ts ./src/lib/platform/catalog.ts
COPY --from=build --chown=node:node /app/prisma ./prisma
COPY --from=build --chown=node:node /app/package.json /app/prisma.config.ts /app/tsconfig.server.json /app/server.mjs /app/LICENSE /app/monumla-source.tar.gz ./
USER node
EXPOSE 3000
VOLUME ["/app/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD node -e "fetch('http://127.0.0.1:3000/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
ENTRYPOINT ["/usr/bin/tini", "--", "/bin/sh", "server/entrypoint.sh"]
