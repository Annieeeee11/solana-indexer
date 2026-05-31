# Live free demo deployment

This guide hosts a **read-only public API** so others can try the indexer without installing Rust. Your Yellowstone + RPC keys stay on the server; users only hit HTTP endpoints.

## Architecture

```
Fly.io / Railway / Render
    └── solana-stream-indexer start (24/7)
            ├── Yellowstone gRPC → slots
            ├── SOLANA_RPC_URL → enrichment + fallback
            └── DATABASE_URL → Supabase (free tier)
                    └── optional: public GET /slots/latest, /health
```

## 1. Supabase (free database)

1. [supabase.com](https://supabase.com) → New project
2. **Settings → Database → Connection string**
3. Prefer **Session pooler** (port 5432) for a long-running process
4. Append `?sslmode=require`
5. Migrations run automatically when the indexer starts — no manual SQL needed

## 2. RPC + Yellowstone (required for live slots)

Free public RPC is too rate-limited for a demo. Use at least one of:

| Provider | Free tier | Use for |
|----------|-----------|---------|
| [Helius](https://helius.dev) | Yes | `SOLANA_RPC_URL` |
| [QuickNode](https://quicknode.com) | Trial | RPC + Geyser |
| [RPCFast](https://rpcfast.io) | Trial | Yellowstone (you may already use this) |

Set in deploy secrets — never commit keys.

## 3. Deploy indexer (Fly.io example)

```bash
# Install flyctl, then from repo root:
fly launch --no-deploy
```

Set secrets:

```bash
fly secrets set \
  DATABASE_URL='postgresql://postgres.[ref]:[PASS]@...pooler.supabase.com:5432/postgres?sslmode=require' \
  SOLANA_RPC_URL='https://mainnet.helius-rpc.com/?api-key=YOUR_KEY' \
  YELLOWSTONE_GRPC_URL='https://your-geyser:443' \
  YELLOWSTONE_GRPC_TOKEN='your-token' \
  API_PORT='8080' \
  API_KEY='demo-read-only-key' \
  RUST_LOG='info,solana_stream_indexer=info'
```

`Dockerfile` (minimal):

```dockerfile
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/solana-stream-indexer /usr/local/bin/
CMD ["solana-stream-indexer", "start"]
```

Deploy:

```bash
fly deploy
fly open /health   # after mapping port 8080 in fly.toml
```

## 4. Public demo URLs

After deploy, share:

```bash
curl https://your-app.fly.dev/health
curl -H "X-API-Key: demo-read-only-key" https://your-app.fly.dev/slots/latest
```

Supabase **Table Editor** is a good free UI to show `slots` filling in real time during demos.

## 5. Cost expectations (free tier)

| Service | Free tier limits |
|---------|------------------|
| Supabase | 500 MB DB, 2 projects |
| Fly.io | Small VM free allowance (check current plan) |
| Helius | Limited free RPC calls |

## 6. What users install locally

For a **local** demo (no hosting):

```bash
cargo install solana-stream-indexer
cp .env.example .env
# Set DATABASE_URL to your Supabase URI
solana-stream-indexer start
```

Same Supabase project can back both your hosted demo and local testers (use separate projects for isolation).

## Security notes

- Always set `API_KEY` on public deployments
- Never expose Supabase **service_role** key in the browser
- RLS warnings in Supabase UI are fine for server-side `DATABASE_URL` access
- Rotate credentials if they appeared in logs or commits
