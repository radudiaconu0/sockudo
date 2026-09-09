use super::*;

impl MysqlVersionStore {
    /// DML locks entries before counters; mutation additionally holds its
    /// existing stream lock first. Installation never acquires stream locks.
    pub(super) async fn ensure_append_counts(&self) -> Result<()> {
        let entries = &self.tables.version_entries;
        let counts = format!("{entries}_ac");
        let marker = format!("{entries}_c1");
        let mut conn = self.pool.acquire().await.map_err(|e| {
            Error::Internal(format!(
                "failed to acquire append count migration connection: {e}"
            ))
        })?;
        // Cancellation must not return a connection holding session/table locks to the pool.
        conn.close_on_drop();
        // MySQL DDL commits implicitly, so serialize installers with a session
        // lock and publish a durable marker only after the locked backfill.
        let acquired: Option<i64> =
            sqlx::query_scalar("SELECT GET_LOCK('sockudo_version_append_counts', 60)")
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| {
                    Error::Internal(format!("failed to lock append count migration: {e}"))
                })?;
        if acquired != Some(1) {
            return Err(Error::Internal(
                "append count migration lock unavailable".into(),
            ));
        }
        let result: Result<()> = async {
            let sql = format!("CREATE TABLE IF NOT EXISTS `{marker}` (id INTEGER PRIMARY KEY) ENGINE=InnoDB");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to initialize append counter tables: {e}")))?;
            let probe = format!("SELECT id FROM `{marker}` WHERE id = 1");
            if sqlx::query(sqlx::AssertSqlSafe(probe.as_str())).fetch_optional(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to read append count migration state: {e}")))?.is_some() { return Ok(()); }
            let sql = format!("CREATE TABLE IF NOT EXISTS `{counts}` (app_id VARCHAR(255) {MYSQL_ASCII_IDENTIFIER_CHARSET} NOT NULL, channel VARCHAR(255) {MYSQL_ASCII_IDENTIFIER_CHARSET} NOT NULL, message_serial VARCHAR({MAX_VERSIONED_SERIAL_LENGTH}) NOT NULL, append_count BIGINT NOT NULL, PRIMARY KEY (app_id, channel, message_serial)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to create append count table: {e}")))?;
            for (suffix, event, body) in [
                ("aci", "INSERT", format!("IF NEW.action = 'message.append' THEN INSERT INTO `{counts}` VALUES (NEW.app_id, NEW.channel, NEW.message_serial, 1) ON DUPLICATE KEY UPDATE append_count = append_count + 1; END IF;")),
                ("acd", "DELETE", format!("IF OLD.action = 'message.append' THEN UPDATE `{counts}` SET append_count = append_count - 1 WHERE app_id = OLD.app_id AND channel = OLD.channel AND message_serial = OLD.message_serial; DELETE FROM `{counts}` WHERE app_id = OLD.app_id AND channel = OLD.channel AND message_serial = OLD.message_serial AND append_count = 0; END IF;")),
            ] {
                let trigger = format!("{entries}_{suffix}");
                let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM INFORMATION_SCHEMA.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE() AND TRIGGER_NAME = ?").bind(&trigger).fetch_one(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to inspect append count trigger: {e}")))?;
                if exists == 0 {
                    let sql = format!("CREATE TRIGGER `{trigger}` AFTER {event} ON `{entries}` FOR EACH ROW BEGIN {body} END");
                    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str())).execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to install append count trigger: {e}")))?;
                }
            }
            let lock = format!("LOCK TABLES `{entries}` WRITE, `{counts}` WRITE, `{marker}` WRITE");
            sqlx::raw_sql(sqlx::AssertSqlSafe(lock.as_str())).execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to lock append count backfill: {e}")))?;
            let backfill: Result<()> = async {
                for sql in [format!("DELETE FROM `{counts}`"), format!("INSERT INTO `{counts}` SELECT app_id, channel, message_serial, COUNT(*) FROM `{entries}` WHERE action = 'message.append' GROUP BY app_id, channel, message_serial"), format!("INSERT INTO `{marker}` VALUES (1)")] {
                    sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to backfill append counts: {e}")))?;
                }
                Ok(())
            }.await;
            sqlx::raw_sql("UNLOCK TABLES").execute(&mut *conn).await.map_err(|e| Error::Internal(format!("failed to unlock append count backfill: {e}")))?;
            backfill
        }.await;
        sqlx::query("SELECT RELEASE_LOCK('sockudo_version_append_counts')")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                Error::Internal(format!(
                    "failed to release append count migration lock: {e}"
                ))
            })?;
        result
    }
}
