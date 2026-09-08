# C1: version-store indexes

Branch: `perf/version-store-indexes`. Starting commit: `743ab4c18565a9ca774fc447ce032cf39aa33eea`.
The checkout is `/Users/radudiaconu/Desktop/Code/Rust/sockudo`, not the Linux path in the request.
Existing IDE edits and the untracked audit directory are unrelated and preserved.
Historical implementation claims in the parent tracker are not evidence for this checkout.

## Behavior-preserving implementation plan

1. Capture release/local unchanged-source baselines with the C1 example fixture, fixed
   revision counts (16, 128, 1024), 256-byte update state, 1/8 independent channels,
   31 independently prepared samples per process and three process repetitions.
   Retain exact commands, allocation calls/bytes, latency samples, CPU/RSS and environment.
2. Index each memory chain by version serial, operation identity and append count.
   Keep maximum-serial selection independent of import order, preserve first matching
   operation receipt, validate incoming history/message identity, and update/reclaim
   indexes atomically with replay and chain state. Preserve full-chain validation.
   Keep the existing lock initially; change lock scope only if measured and verified.
3. Reduce SQL append-count work without changing serial allocation, mutation transaction
   boundaries, duplicate outcomes, import semantics or retention. Capture live SQL baseline
   before changing its production source. Define migration/backfill and concurrent purge
   behavior before adding authoritative counters.
4. Add regressions covering import order, duplicate identities/receipts, conflicts,
   concurrent mutations, caps, terminal/deleted state, purge and contiguous replay;
   run actor/protocol conformance and relevant backend runtime tests.
5. Repeat identical workloads, report variability and costs, update docs and C1 tracker;
   run formatting, workspace tests/clippy, minimal/selected/full feature checks.

No performance or completion claim is valid without before/after and required runtime evidence.
C2 snapshot representation, C7 batch reads and unrelated audit findings remain out of scope.
