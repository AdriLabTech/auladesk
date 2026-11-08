//! Modelos internos independientes de cómo se obtuvieron los datos.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteCourse { pub moodle_id: i64, pub fullname: String, pub shortname: Option<String>, pub url: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteSection { pub moodle_id: Option<i64>, pub position: i64, pub name: String, pub activities: Vec<RemoteActivity> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteActivity {
    pub cmid: i64, pub modname: String, pub name: String, pub url: Option<String>,
    pub due_at: Option<i64>, pub files: Vec<RemoteFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteFile { pub name: String, pub url: String, pub mimetype: Option<String>, pub size: Option<i64> }
