#[path = "../src/db.rs"]
mod db;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open("db/gmail_worker.db")?;

    db::initialize_db(&conn)?;

    let task_id = db::insert_task(
        &conn,
        "gmail",
        "gmail/v1/users/me/messages",
        Some(r#"{"maxResults":5}"#),
    )?;

    println!("Tarea insertada con id: {}", task_id);

    let tareas = db::list_tasks(&conn)?;
    println!("Tareas ahora: {:?}", tareas);

    let claimed = db::claim_next_task(&conn, "gmail")?;
    println!("Tarea reclamada: {:?}", claimed);

    db::mark_task_completed(&conn, task_id)?;
    println!("Tarea completada: {}", task_id);

    let final_task = db::get_task_by_id(&conn, task_id)?;
    println!("Estado final: {:?}", final_task);

    Ok(())
}