use super::*;

impl PostgresVersionStore {
    /// Install once under the schema lock. DML lock order is stream (when
    /// mutating), entry, then append counter. Migration never locks streams.
    pub(super) async fn ensure_append_counts(&self) -> Result<()> {
        let entries = &self.tables.version_entries;
        let counts = format!("{entries}_ac");
        let marker = format!("{entries}_c1");
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                Error::Internal(format!("failed to begin append count migration: {e}"))
            })?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('sockudo_version_append_counts'))")
            .execute(&mut *tx)
            .await
            .map_err(|e| Error::Internal(format!("failed to lock append count migration: {e}")))?;
        let sql =
            format!("CREATE TABLE IF NOT EXISTS {marker} (id INTEGER PRIMARY KEY CHECK (id = 1))");
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                Error::Internal(format!("failed to initialize append counter tables: {e}"))
            })?;
        let probe = format!("SELECT id FROM {marker} WHERE id = 1");
        if sqlx::query(sqlx::AssertSqlSafe(probe.as_str()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                Error::Internal(format!("failed to read append count migration state: {e}"))
            })?
            .is_none()
        {
            let statements = [
                format!(
                    "CREATE TABLE IF NOT EXISTS {counts} (app_id TEXT NOT NULL, channel TEXT NOT NULL, message_serial TEXT NOT NULL, append_count BIGINT NOT NULL, PRIMARY KEY (app_id, channel, message_serial))"
                ),
                format!("LOCK TABLE {entries} IN SHARE ROW EXCLUSIVE MODE"),
                format!(
                    r#"CREATE OR REPLACE FUNCTION {entries}_acf() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP = 'INSERT' THEN
  IF NEW.action = 'message.append' THEN
   INSERT INTO {counts} VALUES (NEW.app_id, NEW.channel, NEW.message_serial, 1)
   ON CONFLICT (app_id, channel, message_serial) DO UPDATE SET append_count = {counts}.append_count + 1;
  END IF;
  RETURN NEW;
 ELSE
  IF OLD.action = 'message.append' THEN
   UPDATE {counts} SET append_count = append_count - 1 WHERE app_id = OLD.app_id AND channel = OLD.channel AND message_serial = OLD.message_serial;
   DELETE FROM {counts} WHERE app_id = OLD.app_id AND channel = OLD.channel AND message_serial = OLD.message_serial AND append_count = 0;
  END IF;
  RETURN OLD;
 END IF;
END $$"#
                ),
                format!("DROP TRIGGER IF EXISTS {entries}_act ON {entries}"),
                format!(
                    "CREATE TRIGGER {entries}_act AFTER INSERT OR DELETE ON {entries} FOR EACH ROW EXECUTE FUNCTION {entries}_acf()"
                ),
                format!("DELETE FROM {counts}"),
                format!(
                    "INSERT INTO {counts} SELECT app_id, channel, message_serial, COUNT(*) FROM {entries} WHERE action = 'message.append' GROUP BY app_id, channel, message_serial"
                ),
                format!("INSERT INTO {marker} VALUES (1)"),
            ];
            for sql in statements {
                sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        Error::Internal(format!("failed to migrate version append counts: {e}"))
                    })?;
            }
        }
        tx.commit()
            .await
            .map_err(|e| Error::Internal(format!("failed to commit append count migration: {e}")))
    }
}
