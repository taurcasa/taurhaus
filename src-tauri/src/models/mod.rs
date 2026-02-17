use serde::{Deserialize, Serialize};

/// A registered project tracked by taurhaus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub last_activity_at: Option<String>,
    pub hero_preference: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
