fn main() {
    // Permisos explícitos por comando: la ventana de Moodle solo recibe `ingest_page`.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_info", "get_setting", "set_setting", "backup_database",
        "moodle_open", "moodle_close", "moodle_navigate", "moodle_probe", "moodle_status", "autosync_status", "moodle_account",
        "ingest_page", "audio_viz_start", "audio_viz_stop", "startup_sync_status", "moodle_sync", "moodle_sync_full", "moodle_forget_session", "list_courses", "course_tree", "download_activity", "download_course", "course_cover", "list_files", "file_open", "file_reveal", "file_read", "file_rename", "file_move", "folder_create", "file_delete", "list_duplicates", "list_folders", "import_file", "task_list", "dashboard_data", "task_set_flags", "note_set", "submission_prepare", "submission_history", "submission_cancel", "submission_start", "submission_submit", "submission_verify", "calendar_list", "calendar_create", "calendar_update", "calendar_delete", "calendar_set_notes", "notif_list", "notif_mark_read", "notif_unread_count", "notif_settings_get", "notif_settings_set", "notif_test", "service_status", "service_install", "service_uninstall",
        "schedule_create", "schedule_list", "schedule_cancel", "pending_prompt_data", "schedule_mark_seen", "folder_backup_status", "folder_backup_set", "folder_backup_submissions_set", "folder_backup_run",
        "integrations_list", "integration_sync", "sync_all", "ingest_familias", "familias_open", "familias_close", "familias_status", "familias_set_active_student", "familias_sync", "familias_recon", "familias_pause", "familias_disconnect", "familias_grades", "familias_attendance", "familias_messages", "familias_message_read", "familias_overview", "familias_action_prepare", "familias_action_run", "familias_action_cancel", "familias_actions",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build");
}
