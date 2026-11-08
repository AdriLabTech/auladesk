//! Tareas y actividades: estado derivado, prioridad personal y notas.
use crate::error::AppResult;
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

/// Estado que ve el usuario. Se calcula al consultar (depende de «ahora»); en BD solo se guarda lo que dice Moodle.
pub fn derive_state(submission: Option<&str>, grading: Option<&str>, due_at: Option<i64>, now: i64, has_prepared: bool, synced: bool) -> &'static str {
    if !synced { return "sin_sincronizar"; }
    let sub = submission.unwrap_or("").to_lowercase();
    let gr = grading.unwrap_or("").to_lowercase();
    if gr.contains("calificado") && !gr.starts_with("sin") && !gr.starts_with("no ") { return "evaluada"; }
    if sub.contains("borrador") { return "iniciada"; }
    if sub.contains("enviado") { return "entregada"; }
    if has_prepared { return "preparada"; }
    match due_at { None => "sin_fecha", Some(d) if now > d => "fuera_de_plazo", _ => "pendiente" }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TaskRaw {
    pub id: i64, pub moodle_cmid: i64, pub course_id: i64, pub course_name: String, pub section_position: Option<i64>, pub section_name: Option<String>, pub name: String, pub url: Option<String>,
    pub description: Option<String>, pub opens_at: Option<i64>, pub due_at: Option<i64>, pub priority: i64, pub favorite: i64, pub done_local: i64, pub local_status: String,
    pub submission_status: Option<String>, pub grading_status: Option<String>, pub grade: Option<String>, pub feedback: Option<String>, pub time_remaining_text: Option<String>,
    pub last_modified_at: Option<i64>, pub submitted_files_json: Option<String>, pub can_submit: Option<i64>, pub assign_synced_at: Option<i64>, pub last_synced_at: Option<i64>,
    pub local_files: i64, pub open_versions: i64, pub latest_version_state: Option<String>, pub note: Option<String>,
    pub intro_text: Option<String>, pub intro_links_json: Option<String>, pub raw_rows_json: Option<String>, pub grader: Option<String>, pub graded_at: Option<i64>,
}

#[derive(Serialize)]
pub struct TaskRow { #[serde(flatten)] pub raw: TaskRaw, pub state: &'static str, pub seconds_to_due: Option<i64> }

#[tauri::command]
pub async fn task_list(st: State<'_, AppState>) -> AppResult<Vec<TaskRow>> { list_tasks(&st.pool, chrono::Utc::now().timestamp()).await }

pub async fn list_tasks(pool: &sqlx::SqlitePool, now: i64) -> AppResult<Vec<TaskRow>> {
    let raws: Vec<TaskRaw> = sqlx::query_as(
        "SELECT a.id, a.moodle_cmid, a.course_id, c.fullname AS course_name, s.position AS section_position, s.name AS section_name, a.name, a.url, a.description,
           a.opens_at, a.due_at, a.priority, a.favorite, a.done_local, a.local_status,
           x.submission_status, x.grading_status, x.grade, x.feedback, x.time_remaining_text, x.last_modified_at, x.submitted_files_json, x.can_submit,
           x.last_synced_at AS assign_synced_at, a.last_synced_at, x.intro_text, x.intro_links_json, x.raw_rows_json, x.grader, x.graded_at,
           (SELECT count(*) FROM file_association f WHERE f.activity_id=a.id AND f.role IN ('statement','resource')) AS local_files,
           (SELECT count(*) FROM submission_version v JOIN submission su ON su.id=v.submission_id WHERE su.activity_id=a.id AND v.state IN ('prepared','in_progress','pending_confirmation')) AS open_versions,
           (SELECT v.state FROM submission_version v JOIN submission su ON su.id=v.submission_id WHERE su.activity_id=a.id ORDER BY v.id DESC LIMIT 1) AS latest_version_state,
           (SELECT n.body FROM personal_note n WHERE n.activity_id=a.id ORDER BY n.id DESC LIMIT 1) AS note
         FROM activity a JOIN course c ON c.id=a.course_id LEFT JOIN course_section s ON s.id=a.section_id LEFT JOIN assignment x ON x.activity_id=a.id
         WHERE a.modname='assign' AND a.removed_at IS NULL AND c.removed_at IS NULL ORDER BY a.due_at IS NULL, a.due_at, a.id").fetch_all(pool).await?;
    Ok(raws.into_iter().map(|r| {
        let state = derive_state(r.submission_status.as_deref(), r.grading_status.as_deref(), r.due_at, now, r.open_versions > 0, r.assign_synced_at.is_some());
        let seconds_to_due = r.due_at.map(|d| d - now);
        TaskRow { raw: r, state, seconds_to_due }
    }).collect())
}

#[tauri::command]
pub async fn task_set_flags(st: State<'_, AppState>, activity_id: i64, priority: Option<i64>, favorite: Option<bool>, done_local: Option<bool>, local_status: Option<String>) -> AppResult<()> {
    if let Some(p) = priority { sqlx::query("UPDATE activity SET priority=? WHERE id=?").bind(p.clamp(0, 3)).bind(activity_id).execute(&st.pool).await?; }
    if let Some(f) = favorite { sqlx::query("UPDATE activity SET favorite=? WHERE id=?").bind(f as i64).bind(activity_id).execute(&st.pool).await?; }
    if let Some(d) = done_local { sqlx::query("UPDATE activity SET done_local=? WHERE id=?").bind(d as i64).bind(activity_id).execute(&st.pool).await?; }
    // «none» (o cadena vacía) = quitar la columna personal y volver a la que marca el estado real.
    if let Some(l) = local_status {
        let v = match l.as_str() { "" | "none" => "none".to_string(), "todo" | "doing" => l, _ => return Err(crate::error::AppError::Other(format!("columna local no válida: {l}"))) };
        sqlx::query("UPDATE activity SET local_status=? WHERE id=?").bind(v).bind(activity_id).execute(&st.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn note_set(st: State<'_, AppState>, activity_id: i64, body: String) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp();
    let n = sqlx::query("UPDATE personal_note SET body=?, updated_at=? WHERE activity_id=?").bind(&body).bind(now).bind(activity_id).execute(&st.pool).await?.rows_affected();
    if n == 0 && !body.trim().is_empty() {
        sqlx::query("INSERT INTO personal_note(activity_id,course_id,body,created_at,updated_at) SELECT id, course_id, ?, ?, ? FROM activity WHERE id=?")
            .bind(&body).bind(now).bind(now).bind(activity_id).execute(&st.pool).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_000_000;
    #[test]
    fn all_seven_states() {
        let s = |sub: &str, gr: &str, due: Option<i64>, prep: bool| derive_state(Some(sub), Some(gr), due, NOW, prep, true);
        assert_eq!(s("Todavía no se han realizado envíos", "Sin calificar", Some(NOW + 5), false), "pendiente");
        assert_eq!(s("Borrador (no enviado)", "Sin calificar", Some(NOW + 5), false), "iniciada");
        assert_eq!(s("Todavía no se han realizado envíos", "Sin calificar", Some(NOW + 5), true), "preparada");
        assert_eq!(s("Enviado para calificar", "Sin calificar", Some(NOW + 5), false), "entregada");
        assert_eq!(s("Enviado para calificar", "Calificado", Some(NOW - 5), false), "evaluada");
        assert_eq!(s("Todavía no se han realizado envíos", "Sin calificar", Some(NOW - 5), false), "fuera_de_plazo");
        assert_eq!(s("Todavía no se han realizado envíos", "Sin calificar", None, false), "sin_fecha");
    }
    #[test]
    fn edge_cases() {
        // calificada por el profesor sin que haya entrega (caso real en Moodle)
        assert_eq!(derive_state(Some("Todavía no se han realizado envíos"), Some("Calificado"), Some(NOW - 9), NOW, false, true), "evaluada");
        // entregada tarde sigue siendo entregada, no «fuera de plazo»
        assert_eq!(derive_state(Some("Enviado para calificar"), Some("Sin calificar"), Some(NOW - 9), NOW, false, true), "entregada");
        // «No calificado» no es evaluada
        assert_eq!(derive_state(Some("x"), Some("No calificado"), Some(NOW + 9), NOW, false, true), "pendiente");
        assert_eq!(derive_state(None, None, Some(NOW - 9), NOW, false, false), "sin_sincronizar", "sin datos de Moodle no se afirma nada");
    }
}
