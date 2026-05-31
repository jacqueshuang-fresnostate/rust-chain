pub mod routes;

#[derive(Debug, Clone)]
pub struct AgentScope {
    pub agent_id: String,
    pub root_agent_id: String,
}

impl AgentScope {
    pub fn can_access_user(&self, user: &AgentTeamUser) -> bool {
        user.root_agent_id.as_deref() == Some(self.root_agent_id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTeamUser {
    pub user_id: String,
    pub root_agent_id: Option<String>,
}

pub fn filter_team_users<'a>(
    scope: &AgentScope,
    users: impl IntoIterator<Item = &'a AgentTeamUser>,
) -> Vec<&'a AgentTeamUser> {
    users
        .into_iter()
        .filter(|user| scope.can_access_user(user))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(user_id: &str, root_agent_id: Option<&str>) -> AgentTeamUser {
        AgentTeamUser {
            user_id: user_id.to_owned(),
            root_agent_id: root_agent_id.map(str::to_owned),
        }
    }

    #[test]
    fn agent_scope_filters_team_users_by_root_agent_id() {
        let scope = AgentScope {
            agent_id: "agent-admin-1".to_owned(),
            root_agent_id: "agent-root-1".to_owned(),
        };
        let users = [
            user("user-1", Some("agent-root-1")),
            user("user-2", Some("agent-root-2")),
            user("user-3", None),
            user("user-4", Some("agent-root-1")),
        ];

        let visible = filter_team_users(&scope, users.iter());

        assert_eq!(
            visible
                .iter()
                .map(|user| user.user_id.as_str())
                .collect::<Vec<_>>(),
            vec!["user-1", "user-4"]
        );
        assert!(scope.can_access_user(&user("team-user", Some("agent-root-1"))));
        assert!(!scope.can_access_user(&user("other-user", Some("agent-root-2"))));
        assert!(!scope.can_access_user(&user("organic-user", None)));
    }
}
