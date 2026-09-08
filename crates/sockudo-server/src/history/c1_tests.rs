//! Isolated C1 fixture. Requires the audit's dedicated Compose project.
use super::*;
use sockudo_core::options::DatabaseConnection;
use sockudo_core::version_store::*;
use sockudo_core::versioned_messages::*;
use sockudo_protocol::messages::MessageData;
use std::time::Instant;

fn record(n: u64, channel: &str) -> StoredVersionRecord {
    StoredVersionRecord {
        app_id: "c1".into(),
        channel: channel.into(),
        original_client_id: Some("actor".into()),
        envelope: None,
        message: VersionedMessage::new_create(
            MessageSerial::new("msg:1").unwrap(),
            VersionMetadata {
                serial: VersionSerial::new(format!("ver:{n:020}")).unwrap(),
                client_id: Some("actor".into()),
                timestamp_ms: 1,
                description: None,
                metadata: None,
            },
            1,
            n,
            Some("event".into()),
            Some(MessageData::String("x".repeat(256))),
            None,
        ),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "requires isolated C1 Compose databases on ports 25461/25462"]
async fn c1_sql_fixed_revisions() {
    for backend in ["postgres", "mysql"] {
        let config = DatabaseConnection {
            host: "127.0.0.1".into(),
            port: if backend == "postgres" { 25461 } else { 25462 },
            username: if backend == "postgres" { "c1" } else { "root" }.into(),
            password: "c1-local-only".into(),
            database: "c1".into(),
            ..Default::default()
        };
        let prefix = format!("c1_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
        let store: Arc<dyn VersionStore> = if backend == "postgres" {
            Arc::new(
                postgres::PostgresVersionStore::new(&config, &DatabasePooling::default(), &prefix)
                    .await
                    .unwrap(),
            )
        } else {
            mysql::create_mysql_version_store(&config, &DatabasePooling::default(), &prefix)
                .await
                .unwrap()
        };
        let pg = if backend == "postgres" {
            Some(
                sqlx::PgPool::connect("postgres://c1:c1-local-only@127.0.0.1:25461/c1")
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let my = if backend == "mysql" {
            Some(
                sqlx::MySqlPool::connect("mysql://root:c1-local-only@127.0.0.1:25462/c1")
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        for count in [16, 128, 1024] {
            for channels in [1, 8] {
                for sample in 0..11 {
                    let mut requests = Vec::new();
                    for c in 0..channels {
                        let channel = format!("r{count}_c{channels}_s{sample}_{c}");
                        store
                            .reserve_delivery_position("c1", &channel)
                            .await
                            .unwrap();
                        store.append_version(record(1, &channel)).await.unwrap();
                        // Bulk-load equivalent immutable fixture rows outside the timer.
                        // First/last import calls populate identical message/stream metadata.
                        let rows = (2..=count)
                            .map(|n| {
                                let r = record(n, &channel);
                                (n, sonic_rs::to_vec(&r).unwrap())
                            })
                            .collect::<Vec<_>>();
                        macro_rules! insert_rows {
                            ($db:ty, $pool:expr, $table:expr) => {{
                                let mut query=sqlx::QueryBuilder::<$db>::new(format!("INSERT INTO {} (app_id,channel,message_serial,version_serial,delivery_serial,history_serial,action,payload_bytes,payload_size_bytes,version_timestamp_ms,created_at_ms) ", $table));
                                query.push_values(&rows, |mut row,(n,payload)| {row.push_bind("c1").push_bind(&channel).push_bind("msg:1").push_bind(format!("ver:{n:020}")).push_bind(*n as i64).push_bind(1_i64).push_bind("message.create").push_bind(payload).push_bind(payload.len() as i64).push_bind(1_i64).push_bind(sockudo_core::history::now_ms());});
                                query.build().execute($pool).await.unwrap();
                            }};
                        }
                        if let Some(pool) = pg.as_ref() {
                            insert_rows!(sqlx::Postgres, pool, format!("{prefix}_version_entries"));
                        }
                        if let Some(pool) = my.as_ref() {
                            insert_rows!(sqlx::MySql, pool, format!("`{prefix}_version_entries`"));
                        }
                        store.append_version(record(count, &channel)).await.unwrap();
                        let current = record(count, &channel);
                        requests.push(VersionMutationRequest {
                            app_id: "c1".into(),
                            channel,
                            message_serial: current.message_serial().clone(),
                            expected: VersionPrecondition::from_record(&current),
                            version: record(count + 1, "").message.version,
                            mutation: VersionMutation::Update(MessageFieldDelta {
                                data: FieldPatch::Replace(MessageData::String("y".repeat(256))),
                                ..Default::default()
                            }),
                            idempotency: None,
                            limits: VersionMutationLimits::default(),
                        });
                    }
                    let append_requests = requests.clone();
                    let start = Instant::now();
                    let mut tasks = tokio::task::JoinSet::new();
                    let barrier = Arc::new(tokio::sync::Barrier::new(channels));
                    for request in requests {
                        let store = store.clone();
                        let barrier = barrier.clone();
                        tasks.spawn(async move {
                            barrier.wait().await;
                            assert!(matches!(
                                store.compare_and_apply(request).await.unwrap(),
                                VersionMutationResult::Applied { .. }
                            ));
                        });
                    }
                    while let Some(result) = tasks.join_next().await {
                        result.unwrap();
                    }
                    println!(
                        "C1,{backend},{count},{channels},{sample},{}",
                        start.elapsed().as_nanos()
                    );
                    let mut requests = Vec::new();
                    for mut request in append_requests {
                        let current = store
                            .get_latest("c1", &request.channel, &request.message_serial)
                            .await
                            .unwrap()
                            .unwrap();
                        request.expected = VersionPrecondition::from_record(&current);
                        request.version = record(count + 2, "").message.version;
                        request.mutation = VersionMutation::Append(MessageAppend {
                            data_fragment: "!".into(),
                            extras: None,
                        });
                        request.limits.max_appends_per_message = Some(1);
                        requests.push(request);
                    }
                    let start = Instant::now();
                    let mut tasks = tokio::task::JoinSet::new();
                    let barrier = Arc::new(tokio::sync::Barrier::new(channels));
                    for request in requests {
                        let store = store.clone();
                        let barrier = barrier.clone();
                        tasks.spawn(async move {
                            barrier.wait().await;
                            assert!(matches!(
                                store.compare_and_apply(request).await.unwrap(),
                                VersionMutationResult::Applied { .. }
                            ));
                        });
                    }
                    while let Some(result) = tasks.join_next().await {
                        result.unwrap();
                    }
                    println!(
                        "C1A,{backend},{count},{channels},{sample},{}",
                        start.elapsed().as_nanos()
                    );
                }
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated C1 Compose databases on ports 25461/25462"]
async fn c1_sql_append_caps_import_duplicate_and_purge() {
    for backend in ["postgres", "mysql"] {
        let config = DatabaseConnection {
            host: "127.0.0.1".into(),
            port: if backend == "postgres" { 25461 } else { 25462 },
            username: if backend == "postgres" { "c1" } else { "root" }.into(),
            password: "c1-local-only".into(),
            database: "c1".into(),
            ..Default::default()
        };
        let prefix = format!("c1_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
        let store: Arc<dyn VersionStore> = if backend == "postgres" {
            Arc::new(
                postgres::PostgresVersionStore::new(&config, &DatabasePooling::default(), &prefix)
                    .await
                    .unwrap(),
            )
        } else {
            mysql::create_mysql_version_store(&config, &DatabasePooling::default(), &prefix)
                .await
                .unwrap()
        };
        let mut newest = record(3, "caps");
        newest.message.action = MessageAction::Append;
        newest.message.append_fragment = Some("x".into());
        store.reserve_delivery_position("c1", "caps").await.unwrap();
        // Arbitrary import order and a duplicate import must not inflate counts.
        store.append_version(newest.clone()).await.unwrap();
        store.append_version(record(1, "caps")).await.unwrap();
        store.append_version(newest.clone()).await.unwrap();
        // Simulate pre-index retained data and interrupted installation, then
        // exercise backfill and a second initialization with the marker present.
        let entries = format!("{prefix}_version_entries");
        if backend == "postgres" {
            let pool = sqlx::PgPool::connect("postgres://c1:c1-local-only@127.0.0.1:25461/c1")
                .await
                .unwrap();
            for sql in [
                format!("DROP TRIGGER {entries}_act ON {entries}"),
                format!("DELETE FROM {entries}_c1"),
                format!("DELETE FROM {entries}_ac"),
            ] {
                sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            for _ in 0..2 {
                postgres::PostgresVersionStore::new(&config, &DatabasePooling::default(), &prefix)
                    .await
                    .unwrap();
            }
            let mut tx = pool.begin().await.unwrap();
            let sql = format!("DELETE FROM {entries} WHERE delivery_serial = 3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.rollback().await.unwrap();
        } else {
            let pool = sqlx::MySqlPool::connect("mysql://root:c1-local-only@127.0.0.1:25462/c1")
                .await
                .unwrap();
            for sql in [
                format!("DROP TRIGGER {entries}_aci"),
                format!("DROP TRIGGER {entries}_acd"),
                format!("DELETE FROM {entries}_c1"),
                format!("DELETE FROM {entries}_ac"),
            ] {
                sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            for _ in 0..2 {
                mysql::create_mysql_version_store(&config, &DatabasePooling::default(), &prefix)
                    .await
                    .unwrap();
            }
            let mut tx = pool.begin().await.unwrap();
            let sql = format!("DELETE FROM {entries} WHERE delivery_serial = 3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.rollback().await.unwrap();
        }
        let request = VersionMutationRequest {
            app_id: "c1".into(),
            channel: "caps".into(),
            message_serial: newest.message_serial().clone(),
            expected: VersionPrecondition::from_record(&newest),
            version: record(4, "").message.version,
            mutation: VersionMutation::Append(MessageAppend {
                data_fragment: "!".into(),
                extras: None,
            }),
            idempotency: None,
            limits: VersionMutationLimits {
                max_appends_per_message: Some(1),
                ..Default::default()
            },
        };
        assert!(matches!(
            store.compare_and_apply(request.clone()).await.unwrap(),
            VersionMutationResult::Rejected(VersionMutationRejection::AppendCount { limit: 1 })
        ));
        let request = VersionMutationRequest {
            limits: VersionMutationLimits {
                max_appends_per_message: Some(2),
                ..Default::default()
            },
            ..request
        };
        let (left, right) = tokio::join!(
            store.compare_and_apply(request.clone()),
            store.compare_and_apply(request.clone())
        );
        let results = [left.unwrap(), right.unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, VersionMutationResult::Applied { .. }))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, VersionMutationResult::Conflict { .. }))
                .count(),
            1
        );
        let latest = store
            .get_latest("c1", "caps", newest.message_serial())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.delivery_serial(), 4);
        let capped = VersionMutationRequest {
            expected: VersionPrecondition::from_record(&latest),
            version: record(5, "").message.version,
            ..request
        };
        assert!(matches!(
            store.compare_and_apply(capped).await.unwrap(),
            VersionMutationResult::Rejected(VersionMutationRejection::AppendCount { limit: 2 })
        ));
        let receipt = sockudo_core::message_envelope::PublishIdempotencyMetadata {
            cache_key: "c1-receipt".into(),
            payload_fingerprint: "original".into(),
        };
        let third = VersionMutationRequest {
            app_id: "c1".into(),
            channel: "caps".into(),
            message_serial: latest.message_serial().clone(),
            expected: VersionPrecondition::from_record(&latest),
            version: record(5, "").message.version,
            mutation: VersionMutation::Append(MessageAppend {
                data_fragment: "!".into(),
                extras: None,
            }),
            idempotency: Some(receipt),
            limits: VersionMutationLimits {
                max_appends_per_message: Some(3),
                ..Default::default()
            },
        };
        let VersionMutationResult::Applied {
            record: applied, ..
        } = store.compare_and_apply(third.clone()).await.unwrap()
        else {
            panic!("exactly two appends must precede the third")
        };
        let VersionMutationResult::Duplicate {
            record: duplicate, ..
        } = store.compare_and_apply(third.clone()).await.unwrap()
        else {
            panic!("expected original receipt")
        };
        assert_eq!(applied.delivery_serial(), duplicate.delivery_serial());
        assert_eq!(applied.original_client_id.as_deref(), Some("actor"));
        let mut changed = third;
        changed.idempotency.as_mut().unwrap().payload_fingerprint = "different".into();
        assert!(matches!(
            store.compare_and_apply(changed).await,
            Err(sockudo_core::error::Error::IdempotencyConflict)
        ));
        let replay = store
            .replay_after(VersionReplayRequest {
                app_id: "c1".into(),
                channel: "caps".into(),
                after_delivery_serial: 3,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(
            replay
                .iter()
                .map(StoredVersionRecord::delivery_serial)
                .collect::<Vec<_>>(),
            vec![4, 5]
        );
        while store
            .purge_before(sockudo_core::history::now_ms() + 1000, 10)
            .await
            .unwrap()
            .1
        {}
        // Reusing the same logical message after retention starts with zero appends.
        let fresh = record(5, "caps");
        store.append_version(fresh.clone()).await.unwrap();
        let request = VersionMutationRequest {
            app_id: "c1".into(),
            channel: "caps".into(),
            message_serial: fresh.message_serial().clone(),
            expected: VersionPrecondition::from_record(&fresh),
            version: record(6, "").message.version,
            mutation: VersionMutation::Append(MessageAppend {
                data_fragment: "!".into(),
                extras: None,
            }),
            idempotency: None,
            limits: VersionMutationLimits {
                max_appends_per_message: Some(1),
                ..Default::default()
            },
        };
        assert!(matches!(
            store.compare_and_apply(request).await.unwrap(),
            VersionMutationResult::Applied { .. }
        ));
    }
}
