use rusqlite::{params, Connection, OptionalExtension, Result};
use std::fs;

const DEFAULT_DB_PATH: &str = "db/gmail_worker.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerEventType {
    TaskAssigned,
    AssetCaptured,
    SchemaVersionCreated,
    TaskCompleted,
    TaskFailed,
}

impl WorkerEventType {
    fn as_str(self) -> &'static str {
        match self {
            Self::TaskAssigned => "TaskAssigned",
            Self::AssetCaptured => "AssetCaptured",
            Self::SchemaVersionCreated => "SchemaVersionCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::TaskFailed => "TaskFailed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerEvent {
    pub id: i64,
    pub task_id: Option<i64>,
    pub event_type: String,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: i64,
    pub worker_type: String,
    pub endpoint: String,
    pub parameters_json: Option<String>,
    pub status: i64,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
}

impl Task {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            worker_type: row.get(1)?,
            endpoint: row.get(2)?,
            parameters_json: row.get(3)?,
            status: row.get(4)?,
            locked_by: row.get(5)?,
            locked_at: row.get(6)?,
            created_at: row.get(7)?,
            completed_at: row.get(8)?,
        })
    }
}

pub fn connect() -> Result<Connection> {
    Connection::open(DEFAULT_DB_PATH)
}

pub fn initialize_db(conn: &Connection) -> Result<()> {
    let sql = fs::read_to_string("db/schema.sql").map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?;
    conn.execute_batch(&sql)?;
    Ok(())
}

pub fn insert_task(conn: &Connection, worker_type: &str, endpoint: &str, parameters_json: Option<&str>) -> Result<i64> {
    conn.execute(
        "INSERT INTO task_queue (worker_type, endpoint, parameters_json, status) VALUES (?, ?, ?, 1)",
        params![worker_type, endpoint, parameters_json],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn get_task_by_id(conn: &Connection, task_id: i64) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_type, endpoint, parameters_json, status, locked_by, locked_at, created_at, completed_at FROM task_queue WHERE id = ?",
    )?;

    let task = stmt
        .query_row(params![task_id], |row| Task::from_row(row))
        .optional()?;

    Ok(task)
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_type, endpoint, parameters_json, status, locked_by, locked_at, created_at, completed_at FROM task_queue ORDER BY id DESC",
    )?;

    let tasks = stmt
        .query_map([], |row| Task::from_row(row))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(tasks)
}

pub fn count_pending_tasks(conn: &Connection, worker_type: &str) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM task_queue WHERE status = 1 AND worker_type = ?",
        params![worker_type],
        |row| row.get(0),
    )?;

    Ok(count)
}

pub fn claim_next_task(conn: &Connection, worker_type: &str) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_type, endpoint, parameters_json, status, locked_by, locked_at, created_at, completed_at FROM task_queue WHERE status = 1 AND worker_type = ? ORDER BY id ASC LIMIT 1",
    )?;

    let task = match stmt.query_row(params![worker_type], |row| Task::from_row(row)).optional()? {
        Some(task) => task,
        None => return Ok(None),
    };

    let updated = conn.execute(
        "UPDATE task_queue SET status = 2, locked_by = ?, locked_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 1",
        params![worker_type, task.id],
    )?;

    if updated == 0 {
        return Ok(None);
    }

    Ok(Some(Task {
        status: 2,
        locked_by: Some(worker_type.to_string()),
        locked_at: Some("CURRENT_TIMESTAMP".to_string()),
        ..task
    }))
}

pub fn mark_task_completed(conn: &Connection, task_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE task_queue SET status = 3, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![task_id],
    )?;

    Ok(())
}

pub fn mark_task_failed(conn: &Connection, task_id: i64, message: &str) -> Result<()> {
    conn.execute(
        "UPDATE task_queue SET status = 4, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![task_id],
    )?;

    record_worker_event(conn, Some(task_id), WorkerEventType::TaskFailed, Some(message))?;

    Ok(())
}

pub fn record_worker_event(
    conn: &Connection,
    task_id: Option<i64>,
    event_type: WorkerEventType,
    details: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO worker_events (task_id, event_type, details, created_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
        params![task_id, event_type.as_str(), details],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn list_worker_events(conn: &Connection, task_id: Option<i64>) -> Result<Vec<WorkerEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, event_type, details, created_at FROM worker_events WHERE (? IS NULL OR task_id = ?) ORDER BY id ASC",
    )?;

    let events = stmt.query_map(params![task_id, task_id], |row| {
        Ok(WorkerEvent {
            id: row.get(0)?,
            task_id: row.get(1)?,
            event_type: row.get(2)?,
            details: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    events.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_list_tasks_works() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        initialize_db(&conn)?;

        let first_id = insert_task(&conn, "gmail", "gmail/v1/users/me/messages", Some(r#"{"maxResults":5}"#))?;
        let second_id = insert_task(&conn, "gmail", "gmail/v1/users/me/profile", None)?;

        assert!(first_id > 0);
        assert!(second_id > 0);

        let tasks = list_tasks(&conn)?;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, second_id);
        assert_eq!(tasks[0].status, 1);
        Ok(())
    }

    #[test]
    fn claim_next_task_marks_task_as_processing() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        initialize_db(&conn)?;

        insert_task(&conn, "gmail", "gmail/v1/users/me/messages", Some(r#"{"maxResults":1}"#))?;

        let claimed = claim_next_task(&conn, "gmail")?;
        assert!(claimed.is_some());

        let task = claimed.unwrap();
        assert_eq!(task.status, 2);
        assert_eq!(task.locked_by.as_deref(), Some("gmail"));

        let pending = count_pending_tasks(&conn, "gmail")?;
        assert_eq!(pending, 0);

        let loaded = get_task_by_id(&conn, task.id)?;
        assert_eq!(loaded.unwrap().status, 2);

        Ok(())
    }

    #[test]
    fn records_and_filters_worker_events() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        initialize_db(&conn)?;
        let first_task_id = insert_task(&conn, "gmail", "gmail/v1/users/me/messages", None)?;
        let second_task_id = insert_task(&conn, "gmail", "gmail/v1/users/me/profile", None)?;

        let event_id = record_worker_event(
            &conn,
            Some(first_task_id),
            WorkerEventType::TaskAssigned,
            Some("Task assigned to gmail worker"),
        )?;
        record_worker_event(&conn, Some(second_task_id), WorkerEventType::TaskCompleted, None)?;

        let events = list_worker_events(&conn, Some(first_task_id))?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event_id);
        assert_eq!(events[0].task_id, Some(first_task_id));
        assert_eq!(events[0].event_type, "TaskAssigned");
        assert_eq!(events[0].details.as_deref(), Some("Task assigned to gmail worker"));
        assert!(!events[0].created_at.is_empty());

        Ok(())
    }

    #[test]
    fn failing_a_task_records_a_task_failed_event() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        initialize_db(&conn)?;
        let task_id = insert_task(&conn, "gmail", "gmail/v1/users/me/messages", None)?;

        mark_task_failed(&conn, task_id, "OAuth credentials are missing")?;

        let task = get_task_by_id(&conn, task_id)?.expect("task should exist");
        assert_eq!(task.status, 4);

        let events = list_worker_events(&conn, Some(task_id))?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "TaskFailed");
        assert_eq!(events[0].details.as_deref(), Some("OAuth credentials are missing"));

        Ok(())
    }
}
