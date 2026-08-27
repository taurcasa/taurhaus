use chrono::{DateTime, Utc};

use crate::models::{ActivityState, ActivityThresholds, Project};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivityWatchTarget {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub activity_state: ActivityState,
    pub should_watch: bool,
}

fn should_watch_for_activity(state: ActivityState) -> bool {
    matches!(state, ActivityState::Active | ActivityState::Recent)
}

pub(crate) fn plan_activity_watch_targets(
    projects: &[Project],
    thresholds: &ActivityThresholds,
) -> Vec<ActivityWatchTarget> {
    plan_activity_watch_targets_at(projects, thresholds, Utc::now())
}

pub(crate) fn plan_activity_watch_targets_at(
    projects: &[Project],
    thresholds: &ActivityThresholds,
    now: DateTime<Utc>,
) -> Vec<ActivityWatchTarget> {
    projects
        .iter()
        .map(|project| {
            let activity_state =
                ActivityState::compute(project.last_activity_at.as_deref(), thresholds, now);
            ActivityWatchTarget {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                project_path: project.path.clone(),
                activity_state,
                should_watch: should_watch_for_activity(activity_state),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn test_project(id: &str, last_activity_at: Option<String>) -> Project {
        Project {
            id: id.to_string(),
            name: id.to_string(),
            path: format!("/tmp/{id}"),
            description: None,
            last_activity_at,
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
            account_memory: Default::default(),
        }
    }

    #[test]
    fn planner_marks_only_active_and_recent_as_watchable() {
        let now = Utc::now();
        let thresholds = ActivityThresholds::default();
        let projects = vec![
            test_project("active", Some((now - Duration::days(1)).to_rfc3339())),
            test_project("recent", Some((now - Duration::days(10)).to_rfc3339())),
            test_project("stale", Some((now - Duration::days(45)).to_rfc3339())),
            test_project("dormant", Some((now - Duration::days(150)).to_rfc3339())),
            test_project("missing", None),
        ];

        let planned = plan_activity_watch_targets_at(&projects, &thresholds, now);
        let watchable_ids: Vec<&str> = planned
            .iter()
            .filter(|target| target.should_watch)
            .map(|target| target.project_id.as_str())
            .collect();

        assert_eq!(watchable_ids, vec!["active", "recent"]);
    }
}
