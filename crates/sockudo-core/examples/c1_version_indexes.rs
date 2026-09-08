use sockudo_core::version_store::*;
use sockudo_core::versioned_messages::*;
use sockudo_protocol::messages::MessageData;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

struct CountAlloc;
static TRACK: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
// SAFETY: all operations forward their original valid arguments to System.
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() && TRACK.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let p = unsafe { System.realloc(p, l, n) };
        if !p.is_null() && TRACK.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(n as u64, Ordering::Relaxed);
        }
        p
    }
}
#[global_allocator]
static ALLOC: CountAlloc = CountAlloc;
fn version(n: u64) -> VersionMetadata {
    VersionMetadata {
        serial: VersionSerial::new(format!("ver:{n:020}")).unwrap(),
        client_id: Some("actor".into()),
        timestamp_ms: n as i64,
        description: None,
        metadata: None,
    }
}
fn record(n: u64, size: usize) -> StoredVersionRecord {
    StoredVersionRecord {
        app_id: "audit".into(),
        channel: "room".into(),
        original_client_id: Some("actor".into()),
        envelope: None,
        message: VersionedMessage::new_create(
            MessageSerial::new("msg:1").unwrap(),
            version(n),
            1,
            n,
            Some("evt".into()),
            Some(MessageData::String("x".repeat(size))),
            None,
        ),
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();
    println!("revisions,channels,sample,elapsed_ns,alloc_calls,alloc_bytes");
    for count in [16, 128, 1024] {
        for channels in [1, 8] {
            for sample in 0..31 {
                let store = MemoryVersionStore::new();
                rt.block_on(async {
                    for c in 0..channels {
                        for n in 1..=count {
                            let mut r = record(n, 256);
                            r.channel = format!("room{c}");
                            store.append_version(r).await.unwrap();
                        }
                    }
                });
                let mut requests = Vec::new();
                for c in 0..channels {
                    requests.push(VersionMutationRequest {
                        app_id: "audit".into(),
                        channel: format!("room{c}"),
                        message_serial: MessageSerial::new("msg:1").unwrap(),
                        expected: VersionPrecondition::from_record(&record(count, 256)),
                        version: version(count + 1),
                        mutation: VersionMutation::Update(MessageFieldDelta {
                            data: FieldPatch::Replace(MessageData::String("y".repeat(256))),
                            ..Default::default()
                        }),
                        idempotency: None,
                        limits: VersionMutationLimits::default(),
                    });
                }
                CALLS.store(0, Ordering::SeqCst);
                BYTES.store(0, Ordering::SeqCst);
                TRACK.store(true, Ordering::SeqCst);
                let start = Instant::now();
                rt.block_on(async {
                    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(channels));
                    let mut tasks = tokio::task::JoinSet::new();
                    for request in requests {
                        let store = store.clone();
                        let barrier = barrier.clone();
                        tasks.spawn(async move {
                            barrier.wait().await;
                            let result = store.compare_and_apply(request).await.unwrap();
                            assert!(matches!(result, VersionMutationResult::Applied { .. }));
                            black_box(result);
                        });
                    }
                    while let Some(result) = tasks.join_next().await {
                        result.unwrap();
                    }
                });
                let elapsed = start.elapsed().as_nanos();
                TRACK.store(false, Ordering::SeqCst);
                println!(
                    "{count},{channels},{sample},{elapsed},{},{}",
                    CALLS.load(Ordering::SeqCst),
                    BYTES.load(Ordering::SeqCst)
                );
            }
        }
    }
}
