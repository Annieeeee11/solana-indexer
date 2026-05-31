# Commands Reference

Binary name: `solana-stream-indexer`

During development, prefix with `cargo run --`:

```bash
cargo run -- <subcommand>
```

After a release build:

```bash
./target/release/solana-stream-indexer <subcommand>
```

---

## Global

```bash
solana-stream-indexer --help
```

---

## `start`

Start the slot pipeline and account watcher in parallel. If `API_PORT` is set in `.env`, the HTTP query API starts in the same process.

```bash
solana-stream-indexer start
```

**Relevant env:** `YELLOWSTONE_GRPC_URL`, `SOLANA_RPC_URL`, `WATCH_ACCOUNTS`, `API_PORT`, `API_KEY`

---

## `track slots`

Stream and persist slots (Yellowstone gRPC → RPC fallback). Does not start the full `start` runtime unless `--watch-accounts` is used.

```bash
solana-stream-indexer track slots
solana-stream-indexer track slots --leaders
solana-stream-indexer track slots --transactions
solana-stream-indexer track slots --watch-accounts
solana-stream-indexer track slots --leaders --transactions --watch-accounts
```

| Flag | Description |
|------|-------------|
| `-l`, `--leaders` | Show slot leaders in CLI output |
| `-t`, `--transactions` | Include transactions in output |
| `-w`, `--watch-accounts` | Also watch active wallets + `WATCH_ACCOUNTS` (like `start`) |

---

## `track wallets`

Manage wallets stored in the database (watched on `start` when active).

### Add

```bash
solana-stream-indexer track wallets add -a <ADDRESS>
solana-stream-indexer track wallets add -a <ADDRESS> -n "My Wallet"
```

### List

```bash
solana-stream-indexer track wallets list
solana-stream-indexer track wallets list --detailed
```

### Remove

```bash
solana-stream-indexer track wallets remove -a <ADDRESS>
```

### Watch only (no slot pipeline)

```bash
solana-stream-indexer track wallets watch
```

---

## `watch`

Poll a single account for balance/data changes (does not require a wallet DB entry).

```bash
solana-stream-indexer watch <ACCOUNT_ADDRESS>
```

Example:

```bash
solana-stream-indexer watch So11111111111111111111111111111111111111112
```

---

## `query`

Read indexed data via MultiCache (L1/L2/L3 → DB fallback). Same data paths as the HTTP API.

```bash
solana-stream-indexer query latest
solana-stream-indexer query slot <SLOT_NUMBER>
solana-stream-indexer query tx <SIGNATURE>
solana-stream-indexer query account <ADDRESS>
```

Examples:

```bash
solana-stream-indexer query latest
solana-stream-indexer query slot 280000000
solana-stream-indexer query account So11111111111111111111111111111111111111112
```

---

## `serve`

Start the HTTP query API only (read-only over cache/DB; does not run the slot pipeline).

```bash
solana-stream-indexer serve
solana-stream-indexer serve --port 3000
```

Default port: `8080` (or `API_PORT` from `.env` if `--port` is omitted).

---

## HTTP API endpoints

When `serve` is running, or when `start` runs with `API_PORT` set:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness check |
| GET | `/ready` | Readiness (DB, RPC, optional Yellowstone) |
| GET | `/slots/latest` | Latest indexed slot |
| GET | `/slots/{number}` | Slot by number |
| GET | `/transactions/{signature}` | Transaction by signature |
| GET | `/accounts/{address}` | Account state |

### Examples

```bash
curl http://localhost:8080/health
curl http://localhost:8080/ready
curl http://localhost:8080/slots/latest
```

With `API_KEY` set:

```bash
curl -H "X-API-Key: your-key" http://localhost:8080/slots/latest
curl -H "Authorization: Bearer your-key" http://localhost:8080/slots/latest
```

---

## Common workflows

### Local dev (RPC only)

```bash
cp .env.example .env
# Set SOLANA_RPC_URL only; leave YELLOWSTONE_GRPC_URL unset

solana-stream-indexer start
# Ctrl+C after a few slots

solana-stream-indexer query latest
```

### Index + API in one process

```bash
# In .env: API_PORT=8080
solana-stream-indexer start

curl http://localhost:8080/health
```

### Hosted demo

See [DEMO.md](DEMO.md).
