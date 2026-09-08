# C1 — version-store indexes

Branch: `perf/version-store-indexes`. Baseline commit:
`743ab4c18565a9ca774fc447ce032cf39aa33eea`. Scope is **C1 only**.
PR branch: `perf/version-store-indexes-pr`, based directly on `master` at
`d7717d3c280d2a506e1659bd4b67181101bfeb4f` to exclude four unrelated Live Activity/CI
commits from the original checkout. The recorded measurements and full validation
below were run on the original baseline plus C1; they are not reruns on this PR base.
The C1 production files apply unchanged. This report is the scoped audit tracker;
pre-existing untracked audit reports and IDE edits are excluded from the PR.

Status: implemented and measured; targeted correctness and live C1 checks pass.
Not marked closed: the broader native AI golden fixture and an additional selected-
feature strict lint check have existing failures described below. These are not
silently included as unrelated fixes.

## Implementation and invariants

- `crates/sockudo-core/src/version_store/memory.rs`: each chain keeps an explicit
  greatest-version pointer, version-identity set, first-inserted operation receipt
  index and retained append count. Channel open-stream counts track changes in
  greatest-version state. Incoming records validate identity and indexed uniqueness;
  mutations no longer rehash or scan every predecessor. The existing channel-wide
  delivery replay index continues enforcing delivery uniqueness and continuity.
- The existing store-wide lock remains. Chain, indexes, replay, timestamps and
  allocator change together under that lock. There is no new lock ordering or
  replacement queue/task/cache. Payload copying and occasional vector growth remain.
  Purge rebuilds the affected chain's indexes, preserving its surviving import order
  and first matching receipt. Purge still has revision-dependent maintenance cost.
- `crates/sockudo-core/src/versioned_messages.rs`: the existing identity validator
  is shared internally; the public full-chain validator remains available unchanged
  for import/repair/conformance. Imported vector order never determines latest.
- PostgreSQL/MySQL `history/*/version_store/append_counts.rs`, `mod.rs` and
  `store_impl.rs`: a retained append counter table is maintained by entry insert/delete
  triggers. Successful imports, ignored duplicates, transaction rollback and purge
  therefore keep counters aligned with retained rows, including writes from older
  binaries. Only capped appends read this counter. Serial allocation, stream locks,
  mutation transaction boundaries, latest selection and wire payloads are unchanged.
- SQL initialization serializes installers, blocks entry writes during backfill, and
  publishes a completion marker only after success. PostgreSQL uses transactional
  DDL; MySQL uses a session lock and closes its initialization connection on drop
  so cancellation cannot leave pooled session/table locks. Lock order is existing
  mutation stream lock → entry → counter; migration does not acquire stream locks.
- Deployment/repair/backup guidance is in
  `docs/content/docs/server/mutable-messages.mdx`. New database permissions and a
  one-time blocking backfill are operational costs; this is not a free migration.

C2's accumulated-snapshot representation, C7's durable batch reads, lock sharding,
and other findings are not implemented here.

## Measurement

See [exact commands and profiles](COMMANDS.md), [plan](PLAN.md),
[per-run p50/p95/p99, throughput and allocation results](results/summary.csv),
[environment](results/environment.txt), [image digests](results/image-digests.txt)
and the raw `.csv`, `.log`, `.time` files in [results](results/).

Host: Apple M5 Pro, 18 CPUs, 48 GiB RAM, macOS 26.6.2, Rust 1.98.0.
Real PostgreSQL 17.6 and MySQL 8.4.11 ran locally in the dedicated Docker Compose
project on loopback ports 25461/25462. These are actual database-engine tests,
not mocked storage or external production-service validation.

Each pair uses identical feature flags, profile, allocator, 256-byte data, starting
revision counts (16/128/1024), 1/8 channels and 8 Tokio workers. Three processes
per phase; 31 freshly seeded memory samples and 11 freshly seeded SQL samples per
case per process. A SQL capped append starts immediately after the timed update,
so it has V+1 retained revisions. Every operation must return Applied; duplicate,
rejected and conflict cases are tested separately, not counted as faster writes.

Times are **batch completion latency including task scheduling**, not socket p99 or
production capacity. Throughput excludes fixture setup; process CPU/RSS includes
setup. These short runs do not establish sustained-load or production tail latency.
Memory and SQL use different documented release settings and allocators; only
within-backend before/after comparisons are meaningful. Small SQL cases have
visible noise and counter-write overhead, especially capped MySQL appends.

Representative 1,024-revision results (range of the three process p50s):

| Workload | Before | After |
|---|---:|---:|
| Memory update, 1 channel | 87.9–98.4 µs | 11.7–12.6 µs |
| Memory update, 8 channels | 944.2–1,279.5 µs | 40.8–45.9 µs |

Memory requested allocation bytes per operation in the eight-channel case fell
from 129,162 to 22,634; allocator calls fell from 61.375 to 42.375 (batch averages).
Vector capacity expansion at the measured power-of-two sizes accounts for much
of the remaining requested bytes: this is amortized indexing, not a claim that
all individual allocations are independent of chain length. Peak process RSS was
25.08–25.20 MB before versus 25.72–25.82 MB after. The memory indexes have a storage
cost. Whole-fixture user CPU was 8.45–9.81 s before versus 0.21–0.23 s after, largely
reflecting removal of repeated validation during untimed fixture imports.

SQL removes one count query from updates/uncapped mutations. Capped appends retain
one scalar read and add transactional counter maintenance on insert. Query plans
in `*-baseline-explain.txt` versus `*-after-explain.txt` show 1,026 scanned entry
rows becoming one counter row; PostgreSQL buffer hits fell from 122 to 3 for that
lookup. Network bytes and database process CPU were not independently instrumented;
client CPU/RSS and database rows/buffers are retained, and no network/CPU savings
percentage is claimed. Final-source SQL results are below; preliminary/failed runs
are explicitly excluded in COMMANDS.md.

| SQL workload, 1,024 starting revisions | Before p50 range | Final after p50 range |
|---|---:|---:|
| postgres, update, 1 channel(s) | 3.004–3.056 ms | 2.099–2.322 ms |
| postgres, update, 8 channel(s) | 5.141–5.366 ms | 4.063–4.546 ms |
| postgres, capped append, 1 channel(s) | 2.628–2.723 ms | 2.320–2.553 ms |
| postgres, capped append, 8 channel(s) | 4.510–4.736 ms | 3.959–4.638 ms |
| mysql, update, 1 channel(s) | 4.525–4.884 ms | 2.854–2.951 ms |
| mysql, update, 8 channel(s) | 10.903–11.788 ms | 5.688–6.487 ms |
| mysql, capped append, 1 channel(s) | 4.378–4.483 ms | 2.868–3.022 ms |
| mysql, capped append, 8 channel(s) | 11.262–11.537 ms | 5.847–6.920 ms |

## Correctness and verification

- Baseline: 19 existing memory version-store tests passed before production editing.
- Candidate: 23 version-store tests, 400 core tests and 434 adapter tests passed.
  Coverage includes imported order, greatest-serial winner, duplicate version/delivery
  rejection, identity mismatch, receipt replay/conflict, concurrent same-message and
  independent-channel mutations, append/open/terminal caps, deletion, partial purge,
  receipt promotion, stream continuity and contiguous replay.
- Actual PostgreSQL/MySQL: import order, duplicate insert, capped appends, migration
  backfill, repeated initialization, delete rollback, same-message CAS race, exact
  post-race append count, idempotency conflict/replay, original actor preservation,
  contiguous replay and purge/reuse checks passed. See `sql-final-correctness.log`.
- Selected HTTP mutation tests: all 15 passed, including authenticated owner/any
  authorization, non-owner denial, duplicate acknowledgement and terminal append.
- Live native HTTP + V1/V2 JSON WebSockets: eight concurrent appends delivered every
  original exactly once with contiguous serials; duplicate receipts, conflict error,
  actor-spoof denial, deletion/history and V1 field stripping passed. See
  [fixture](live_native.mjs) and `live-native-final.log`.
- Live Ably mutable-message and recovery fixtures passed (`live-ably-mutable.log`,
  `live-ably-recovery.log`), including original append fragments and version history.
- `cargo fmt --all`, `cargo test --workspace` (1,606 passed, one explicitly ignored
  nightly disaster burn-in), and `cargo clippy --workspace --all-targets -- -D warnings`
  passed. Strict Clippy also passed with the C1 SQL feature set (`local,versioned-messages,postgres,mysql`). The first workspace run lacked Redis; the successful repeat used only the
  isolated Redis on 25463. Minimal, selected and full server feature checks passed.
- Docs types/build and offline AI fixture validation passed. Exact commands/exits are
  in `verification.json`, `verification-extra.json` and the dedicated rerun logs.

## Remaining gaps and separate follow-ups

1. The existing native AI live golden harness subscribes to private channels without
   auth and expects `sockudo:subscription_succeeded`; the current V2 contract emits
   `sockudo_internal:subscription_succeeded`. Its unmodified run fails before C1
   mutations. A diagnostic auth adapter confirmed the event-name mismatch. The
   focused C1 native fixture uses the current authenticated protocol and passes.
   Propose **`fix/native-ai-conformance-fixtures`** separately; no protocol behavior
   was weakened to satisfy stale golden fixtures.
2. Extra selected-feature strict Clippy fails on existing `unused_mut` in
   `crates/sockudo-server/src/bootstrap/push/queue.rs:257`. The full feature check also
   warns about an existing unused APNs import. Default workspace strict Clippy passes.
   Propose **`fix/push-feature-lints`** separately; neither push file is changed here.
3. No production multi-node capacity/soak claim, external database deployment test,
   mixed-version fleet stress test, independent database RSS/CPU sampling, or new
   exhaustive live MessagePack/Protobuf matrix. Existing workspace codec/recovery
   tests passed. Counter migrations were exercised on small local retained datasets;
   large production backfill duration and contention still require deployment planning.

Task-created server, containers, network and anonymous database volumes were cleaned
up after verification; see `results/cleanup.log`. Downloaded images and local build
artifacts remain available for reproduction.

## PR base verification

On the isolated PR branch based on `master`, formatting and all 23 focused
version-store tests pass. See `results/pr-base-core.log`. Prior full-workspace,
live-service and benchmark evidence above remains explicitly tied to the original
checkout; full runtime validation on the new PR base is still pending.

The PR-base SQL feature check also passes:
`cargo check -p sockudo --no-default-features --features local,versioned-messages,postgres,mysql`.
Both PR-base commands used `CARGO_TARGET_DIR=/Users/radudiaconu/Desktop/Code/Rust/sockudo/target`;
formatting used `cargo fmt --all -- --check`. Raw logs retain tool-produced whitespace.
