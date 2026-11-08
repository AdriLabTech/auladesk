fn main() {
    // Permisos explícitos por comando: la ventana de Moodle solo recibe `ingest_page`.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_info", "get_setting", "set_setting", "backup_database",
        "moodle_open", "moodle_close", "moodle_navigate", "moodle_probe", "moodle_status", "moodle_account",
        "ingest_page", "moodle_sync", "moodle_forget_session", "list_courses", "course_tree", "download_activity", "download_course", "course_cover", "list_files", "file_open", "file_reveal", "file_rename", "file_move", "folder_create", "file_delete", "list_duplicates", "list_folders", "import_file", "task_list", "dashboard_data", "task_set_flags", "note_set", "submission_prepare", "submission_history", "submission_cancel", "submission_start", "submission_verify", "calendar_list", "calendar_create", "calendar_update", "calendar_delete", "calendar_set_notes", "notif_list", "notif_mark_read", "notif_unread_count", "notif_settings_get", "notif_settings_set", "notif_test", "service_status", "service_install", "service_uninstall",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build");
}
