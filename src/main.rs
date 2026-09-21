use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};

const DEFAULT_DB_PATH: &str = "db/gmail_worker.db";
const DEFAULT_WORKER_TYPE: &str = "gmail";
const GMAIL_API_BASE: &str = "https://gmail.googleapis.com";

mod db;

#[derive(Debug, PartialEq, Eq)]
struct GmailCredentials {
    client_id: String,
    client_secret: String,
    refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Task {
    id: i64,
    worker_type: String,
    endpoint: String,
    parameters_json: Option<String>,
    status: i64,
    locked_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CliArgs {
    action: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = env::args().skip(1).collect();
    let action = args.first().cloned().unwrap_or_else(|| "--process-one".to_string());

    let conn = db::connect()?;
    db::initialize_db(&conn)?;

    match action.as_str() {
        "--init-db" => {
            println!("Database initialized at {}", database_path()?.display());
        }
        "--queue-example" => {
            let task_id = queue_example_task(&conn)?;
            println!("Example task {task_id} inserted into task_queue.");
        }
        "--list-tasks" => {
            list_tasks(&conn)?;
        }
        "--process-one" => {
            process_one_task(&conn)?;
        }
        "--help" | "-h" | "help" => {
            print_usage();
        }
        _ => {
            print_usage();
            bail!("Unknown command: {action}");
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Rust Gmail worker CLI");
    println!("Usage:");
    println!("  cargo run -- --init-db");
    println!("  cargo run -- --queue-example");
    println!("  cargo run -- --process-one");
    println!("  cargo run -- --list-tasks");
    println!("");
    println!("Environment variables:");
    println!("  GMAIL_CLIENT_ID, GMAIL_CLIENT_SECRET, GMAIL_REFRESH_TOKEN (.env supported)");
    println!("  DATABASE_URL (optional, default: db/gmail_worker.db)");
}

fn database_path() -> Result<PathBuf> {
    let raw = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB_PATH.to_string());
    let path = PathBuf::from(raw);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn connect_db() -> Result<Connection> {
    let db_path = database_path()?;
    Connection::open(db_path).context("No se pudo abrir la base de datos SQLite")
}

fn initialize_db(conn: &Connection) -> Result<()> {
    let sql = fs::read_to_string("db/schema.sql")
        .context("No se encontró db/schema.sql. Asegúrate de que el archivo exista.")?;
    conn.execute_batch(&sql)
        .context("No se pudo inicializar el esquema de SQLite")?;
    Ok(())
}

fn queue_example_task(conn: &Connection) -> Result<i64> {
    let params = serde_json::json!({
        "maxResults": 5,
        "q": "has:attachment"
    });

    conn.execute(
        "INSERT INTO task_queue (worker_type, endpoint, parameters_json, status) VALUES (?, ?, ?, 1)",
        params![
            DEFAULT_WORKER_TYPE,
            "gmail/v1/users/me/messages",
            params.to_string()
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

fn list_tasks(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_type, endpoint, parameters_json, status, locked_by, created_at, completed_at FROM task_queue ORDER BY id DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    })?;

    println!("ID | Worker | Endpoint | Status | Locked by | Created at | Completed at");
    for row in rows {
        let (id, worker, endpoint, params, status, locked_by, created_at, completed_at) = row?;
        println!(
            "{} | {} | {} | {} | {:?} | {} | {:?}",
            id, worker, endpoint, status, locked_by, created_at, completed_at
        );
        if let Some(params) = params {
            println!("  params: {params}");
        }
    }

    Ok(())
}

fn process_one_task(conn: &Connection) -> Result<()> {
    let worker_type = env::var("GMAIL_WORKER_TYPE").unwrap_or_else(|_| DEFAULT_WORKER_TYPE.to_string());

    let task = match claim_next_task(conn, &worker_type)? {
        Some(task) => task,
        None => {
            println!("No hay tareas pendientes para '{}'.", worker_type);
            return Ok(());
        }
    };

    let task_id = task.id;

    if let Err(err) = execute_task_cycle(conn, &task) {
        let message = err.to_string();
        mark_task_failed(conn, task_id, &message)?;
        println!("Task {task_id} failed: {message}");
        return Err(err);
    }

    println!("Task {task_id} completed successfully.");
    Ok(())
}

fn claim_next_task(conn: &Connection, worker_type: &str) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_type, endpoint, parameters_json, status, locked_by FROM task_queue WHERE status = 1 AND worker_type = ? ORDER BY id ASC LIMIT 1",
    )?;

    let task = match stmt.query_row(params![worker_type], |row| {
        Ok(Task {
            id: row.get(0)?,
            worker_type: row.get(1)?,
            endpoint: row.get(2)?,
            parameters_json: row.get(3)?,
            status: row.get(4)?,
            locked_by: row.get(5)?,
        })
    }) {
        Ok(task) => task,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(err) => return Err(err.into()),
    };

    let updated = conn.execute(
        "UPDATE task_queue SET status = 2, locked_by = ?, locked_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 1",
        params![worker_type, task.id],
    )?;

    if updated == 0 {
        return Ok(None);
    }

    record_event(conn, Some(task.id), "TaskAssigned", &format!("Task {} assigned to worker {}", task.id, worker_type))?;

    Ok(Some(task))
}

fn execute_task_cycle(conn: &Connection, task: &Task) -> Result<()> {
    let access_token = refresh_access_token()?;

    let payload = fetch_gmail_messages(&access_token, task)?;

    save_asset(conn, task.id, &payload)?;
    record_event(conn, Some(task.id), "AssetCaptured", &format!("Stored payload for task {}", task.id))?;

    let schema = infer_schema(&payload);
    let schema_hash = compute_schema_hash(&schema);
    let version_created = ensure_schema_version(conn, &task.endpoint, &schema, &schema_hash, task.id)?;

    if version_created {
        record_event(
            conn,
            Some(task.id),
            "SchemaVersionCreated",
            &format!("New schema version created for endpoint {}: {}", task.endpoint, schema_hash),
        )?;
    }

    mark_task_completed(conn, task.id)?;
    record_event(conn, Some(task.id), "TaskCompleted", &format!("Task {} finalizada correctamente", task.id))?;

    println!("Payload stored and schema hash {} checked for {}.", schema_hash, task.endpoint);
    Ok(())
}

fn refresh_access_token() -> Result<String> {
    let credentials = gmail_credentials()?;

    let response = Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", credentials.client_id),
            ("client_secret", credentials.client_secret),
            ("refresh_token", credentials.refresh_token),
            ("grant_type", "refresh_token".to_string()),
        ])
        .send()
        .context("No se pudo contactar a Google OAuth")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        bail!("OAuth token refresh failed ({}): {}", status, body);
    }

    let payload: Value = response
        .json()
        .context("La respuesta de OAuth no fue un JSON válido")?;

    payload
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("La respuesta de OAuth no incluye access_token")
}

fn gmail_credentials() -> Result<GmailCredentials> {
    gmail_credentials_from(|key| env::var(key))
}

fn gmail_credentials_from<F>(get_var: F) -> Result<GmailCredentials>
where
    F: Fn(&str) -> std::result::Result<String, env::VarError>,
{
    Ok(GmailCredentials {
        client_id: get_var("GMAIL_CLIENT_ID").context("Falta GMAIL_CLIENT_ID")?,
        client_secret: get_var("GMAIL_CLIENT_SECRET").context("Falta GMAIL_CLIENT_SECRET")?,
        refresh_token: get_var("GMAIL_REFRESH_TOKEN").context("Falta GMAIL_REFRESH_TOKEN")?,
    })
}

fn fetch_gmail_messages(access_token: &str, task: &Task) -> Result<Value> {
    let url = format!("{}/{}", GMAIL_API_BASE, task.endpoint.trim_start_matches('/'));
    let mut request = Client::new().get(url).bearer_auth(access_token);

    if let Some(raw_params) = task.parameters_json.as_deref() {
        let params: Value = serde_json::from_str(raw_params)
            .context("Los parámetros de la tarea no son un JSON válido")?;

        if let Value::Object(map) = params {
            for (key, value) in map {
                match value {
                    Value::Array(items) => {
                        for item in items {
                            if let Some(str_value) = item.as_str() {
                                request = request.query(&[(key.clone(), str_value)]);
                            } else {
                                request = request.query(&[(key.clone(), item.to_string())]);
                            }
                        }
                    }
                    Value::Bool(value) => {
                        request = request.query(&[(key, value.to_string())]);
                    }
                    Value::Null => {}
                    Value::Number(value) => {
                        request = request.query(&[(key, value.to_string())]);
                    }
                    Value::String(value) => {
                        request = request.query(&[(key, value)]);
                    }
                    Value::Object(_) => {
                        request = request.query(&[(key, value.to_string())]);
                    }
                }
            }
        }
    }

    let response = request.send().context("Error al consultar la Gmail API")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        bail!("La API de Gmail devolvió {}: {}", status, body);
    }

    response
        .json::<Value>()
        .context("La respuesta de Gmail no fue JSON válido")
}

fn save_asset(conn: &Connection, task_id: i64, payload: &Value) -> Result<()> {
    let payload_json = serde_json::to_string(payload).context("No se pudo serializar el payload JSON")?;
    conn.execute(
        "INSERT INTO assets (task_id, payload_json) VALUES (?, ?)",
        params![task_id, payload_json],
    )?;
    Ok(())
}

fn infer_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut normalized = Map::new();
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            for key in keys {
                normalized.insert(key.to_string(), infer_schema(map.get(key).unwrap()));
            }
            Value::Object(normalized)
        }
        Value::Array(items) => {
            let item_schema = if let Some(first) = items.first() {
                infer_schema(first)
            } else {
                Value::Null
            };
            let mut normalized = Map::new();
            normalized.insert("type".to_string(), Value::String("array".to_string()));
            normalized.insert("items".to_string(), item_schema);
            Value::Object(normalized)
        }
        Value::String(_) => json_object("string"),
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                json_object("integer")
            } else {
                json_object("number")
            }
        }
        Value::Bool(_) => json_object("boolean"),
        Value::Null => json_object("null"),
    }
}

fn json_object(value: &str) -> Value {
    let mut map = Map::new();
    map.insert("type".to_string(), Value::String(value.to_string()));
    Value::Object(map)
}

fn compute_schema_hash(schema: &Value) -> String {
    let canonical = sort_json(schema);
    let serialized = serde_json::to_string(&canonical).expect("schema should serialize");
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sort_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            for key in keys {
                sorted.insert(key.to_string(), sort_json(map.get(key).unwrap()));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(sort_json).collect()),
        _ => value.clone(),
    }
}

fn ensure_schema_version(
    conn: &Connection,
    endpoint: &str,
    schema: &Value,
    schema_hash: &str,
    task_id: i64,
) -> Result<bool> {
    let latest_hash: Option<String> = conn
        .query_row(
            "SELECT schema_hash FROM schema_versions WHERE endpoint = ? ORDER BY version_number DESC LIMIT 1",
            params![endpoint],
            |row| row.get(0),
        )
        .optional()?;

    if latest_hash.as_deref() == Some(schema_hash) {
        return Ok(false);
    }

    let next_number: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM schema_versions WHERE endpoint = ?",
            params![endpoint],
            |row| row.get(0),
        )?;

    conn.execute(
        "INSERT INTO schema_versions (endpoint, version_number, schema_hash, created_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
        params![endpoint, next_number, schema_hash],
    )?;

    let version_id = conn.last_insert_rowid();
    let canonical = serde_json::to_string(&sort_json(schema))?;
    conn.execute(
        "INSERT INTO schema_snapshots (schema_version_id, schema_json, created_at) VALUES (?, ?, CURRENT_TIMESTAMP)",
        params![version_id, canonical],
    )?;

    record_event(
        conn,
        Some(task_id),
        "SchemaVersionCreated",
        &format!("Schema changed for {}. New version {} stored.", endpoint, next_number),
    )?;

    Ok(true)
}

fn mark_task_completed(conn: &Connection, task_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE task_queue SET status = 3, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![task_id],
    )?;
    Ok(())
}

fn mark_task_failed(conn: &Connection, task_id: i64, message: &str) -> Result<()> {
    conn.execute(
        "UPDATE task_queue SET status = 4, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![task_id],
    )?;
    record_event(conn, Some(task_id), "TaskFailed", message)?;
    Ok(())
}

fn record_event(conn: &Connection, task_id: Option<i64>, event_type: &str, details: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO worker_events (task_id, event_type, details, created_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
        params![task_id, event_type, details],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn schema_hash_is_stable_for_same_structure() {
        let payload = json!({
            "messages": [{ "id": "abc", "threadId": "xyz" }],
            "resultSizeEstimate": 2450,
            "nextPageToken": "token"
        });

        let schema_a = infer_schema(&payload);
        let schema_b = infer_schema(&payload);

        assert_eq!(compute_schema_hash(&schema_a), compute_schema_hash(&schema_b));
    }

    #[test]
    fn schema_hash_changes_when_structure_changes() {
        let before = json!({
            "messages": [{ "id": "abc", "threadId": "xyz" }],
            "resultSizeEstimate": 2450
        });

        let after = json!({
            "messages": [{ "id": "abc", "threadId": "xyz", "labelIds": ["inbox"] }],
            "resultSizeEstimate": 2450
        });

        assert_ne!(compute_schema_hash(&infer_schema(&before)), compute_schema_hash(&infer_schema(&after)));
    }

    #[test]
    fn gmail_credentials_reads_all_required_values() -> Result<()> {
        let values = HashMap::from([
            ("GMAIL_CLIENT_ID", "client-id"),
            ("GMAIL_CLIENT_SECRET", "client-secret"),
            ("GMAIL_REFRESH_TOKEN", "refresh-token"),
        ]);

        let credentials = gmail_credentials_from(|key| {
            values
                .get(key)
                .map(|value| (*value).to_string())
                .ok_or(env::VarError::NotPresent)
        })?;

        assert_eq!(
            credentials,
            GmailCredentials {
                client_id: "client-id".to_string(),
                client_secret: "client-secret".to_string(),
                refresh_token: "refresh-token".to_string(),
            }
        );
        Ok(())
    }

    #[test]
    fn gmail_credentials_reports_missing_value() {
        let err = gmail_credentials_from(|_| Err(env::VarError::NotPresent))
            .expect_err("missing credentials should fail");

        assert_eq!(err.to_string(), "Falta GMAIL_CLIENT_ID");
    }

    #[test]
    fn queue_example_task_always_inserts_a_new_pending_task() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        initialize_db(&conn)?;

        let first_id = queue_example_task(&conn)?;
        let second_id = queue_example_task(&conn)?;
        assert_ne!(first_id, second_id);

        let pending_tasks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM task_queue WHERE worker_type = ? AND status = 1",
            params![DEFAULT_WORKER_TYPE],
            |row| row.get(0),
        )?;
        assert_eq!(pending_tasks, 2);
        Ok(())
    }
}
