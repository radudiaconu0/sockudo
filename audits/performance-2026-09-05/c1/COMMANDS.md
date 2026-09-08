# Reproduction commands

Run at the repository root. Original commit: `743ab4c18565a9ca774fc447ce032cf39aa33eea`.
Build the baseline before applying the production diff, using the same final fixtures.
No production commits, remote branches, PRs or deployments were created.

## Environment

```
rustc -Vv
sw_vers
sysctl -n machdep.cpu.brand_string hw.memsize hw.ncpu
docker version
docker compose -p sockudo-c1-indexes -f audits/performance-2026-09-05/c1/compose.yaml up -d postgres mysql
docker image inspect postgres:17.6 --format '{{json .RepoDigests}}'
docker image inspect mysql:8.4 --format '{{json .RepoDigests}}'
```

`results/environment.txt`, `image-digests.txt`, and `compose-up.log` retain details.
The Compose services were created for this work. Redis was added **after** the
performance runs for correctness tests; it did not run during the measured pairs.

## Memory, before and after

```
cargo build -p sockudo-core --example c1_version_indexes --release --no-default-features --features local
```

Run the built executable three times, saving stdout/stderr separately:

```
/usr/bin/time -l target/release/examples/c1_version_indexes
```

Chosen before files: `results/memory-baseline-clean-{1,2,3}.{csv,time}`.
After files: `results/memory-after-{1,2,3}.{csv,time}`.
The earlier `memory-baseline-{1,2,3}` runs are preliminary and excluded: a compiler
briefly overlapped one run. The clean runs had no compilation started by this task.
Both chosen phases use root release settings: LTO=true, codegen-units=1, local
feature, System allocator, 8 Tokio workers, identical 256-byte payloads and fixed
16/128/1024 starting revisions, 1/8 channels, 31 fresh samples per group/process.
Build and setup time are excluded from latency; process CPU/RSS includes setup.

## SQL, before and after

```
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 cargo test -p sockudo --release --no-default-features --features local,versioned-messages,postgres,mysql c1_sql_fixed_revisions --no-run
```

Copy the resulting unit-test executable (path printed by Cargo) before overwriting
it with the candidate build. This task saved binaries locally under `target/`.
Then run:

```
python3 audits/performance-2026-09-05/c1/run_sql.py target/c1-sql-baseline-final baseline-v2
python3 audits/performance-2026-09-05/c1/run_sql.py target/c1-sql-after-final after-final
python3 audits/performance-2026-09-05/c1/summarize.py
```

The runner resets only the two isolated C1 databases before each process and saves
all outputs, exit failures, process CPU and peak RSS. Final inputs use 11 fresh
samples per 16/128/1024 starting revisions and 1/8 channels, 8 Tokio workers,
256-byte updates, and a one-byte capped append immediately after each update.
Thus the capped append starts with V+1 revisions. All seed revisions are creates;
the capped append must scan V+1 non-append rows in the original implementation.
First/last fixture imports use the real API; intermediate payload rows are loaded
in bulk outside timing. Both phases use the same fixture and default service
configuration. SQL measurements use optimized release without LTO, codegen-units=16,
and the server's jemalloc allocator. Do not compare absolute SQL and memory costs
as though their profiles, runtimes and allocators were identical.

`sql-baseline.log` is an incomplete slow-setup diagnostic, and `sql-baseline-final-1`
is a failed fixture run (MySQL's identifier-length limit). Neither is included.
The shortened random table prefix and final bulk setup are identical in all chosen
`sql-baseline-v2-*` and `sql-after-final-*` runs. SQL source was unchanged until all three
chosen baseline runs completed; the already-measured memory implementation does
not participate in these SQL mutation paths.

## Correctness and required checks

```
python3 audits/performance-2026-09-05/c1/verify.py
docker compose -p sockudo-c1-indexes -f audits/performance-2026-09-05/c1/compose.yaml up -d redis
REDIS_URL=redis://127.0.0.1:25463/ cargo test --workspace
python3 audits/performance-2026-09-05/c1/verify_extra.py
AIT_CONFORMANCE_OFFLINE=1 node tests/ai-conformance/src/run.mjs
```

Exact commands and exits for formatting, focused/core/adapter tests, workspace
checks, minimal/selected/full features and docs are saved by the verification
scripts. The first workspace attempt failed only because its Redis fixture was
absent; the rerun uses the isolated Redis service above.

The earlier successful `sql-after-*` runs are retained as intermediate evidence;
`sql-after-final-*` repeats the final production source and is used in summary.csv.
The final SQL source differs from that intermediate build only in equivalent
single-statement migration initialization syntax; no mutation behavior changed.

## Live local checks

The server was launched from the C1 directory with a clean allowlist environment
(PATH/HOME/TMPDIR/locale only), using `target/debug/sockudo --config` and the absolute
path to `live.toml`. It listens only on loopback port 25464, with memory drivers.

```
node audits/performance-2026-09-05/c1/live_native.mjs
ABLY_PORT=25464 node tests/ably-compat/ait-mutable.mjs
ABLY_PORT=25464 node tests/ably-compat/ait-recovery.mjs
cargo clippy -p sockudo --all-targets --no-default-features --features local,versioned-messages,postgres,mysql -- -D warnings
```

The final native fixture asserts the existing HTTP 400 idempotency conflict,
HTTP 401 unsigned-in actor rejection and V2 `serial` wire field; preliminary
fixture assertions used the wrong status/field names. Production code was not
changed to accommodate them. Failed broader AI conformance commands:

```
SOCKUDO_BASE_URL=http://127.0.0.1:25464 SOCKUDO_WS_URL='ws://127.0.0.1:25464/app/app-key?protocol=2&client=ait-conformance&version=0' node tests/ai-conformance/src/run.mjs
SOCKUDO_BASE_URL=http://127.0.0.1:25464 SOCKUDO_WS_URL='ws://127.0.0.1:25464/app/app-key?protocol=2&client=ait-conformance&version=0' node audits/performance-2026-09-05/c1/run_ai_conformance.mjs
```

Only task-created services are cleaned up:

```
docker compose -p sockudo-c1-indexes -f audits/performance-2026-09-05/c1/compose.yaml down -v
```
