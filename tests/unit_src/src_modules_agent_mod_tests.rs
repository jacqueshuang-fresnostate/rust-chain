use super::domain::{AgentHierarchyNode, derive_agent_placement};
use super::*;

fn user(user_id: &str, agent_path: Option<&str>) -> AgentTeamUser {
    AgentTeamUser {
        user_id: user_id.to_owned(),
        agent_path: agent_path.map(str::to_owned),
    }
}

#[test]
fn agent_scope_filters_users_by_agent_subtree_path() {
    let scope = AgentScope {
        agent_id: "agent-admin-1".to_owned(),
        agent_path: "/agent:1/agent:2".to_owned(),
    };
    let users = [
        user("user-1", Some("/agent:1/agent:2")),
        user("user-2", Some("/agent:1/agent:20")),
        user("user-3", None),
        user("user-4", Some("/agent:1/agent:2/agent:3")),
    ];

    let visible: Vec<_> = users
        .iter()
        .filter(|user| scope.can_access_user(user))
        .collect();

    assert_eq!(
        visible
            .iter()
            .map(|user| user.user_id.as_str())
            .collect::<Vec<_>>(),
        vec!["user-1", "user-4"]
    );
    assert!(scope.can_access_user(&user("team-user", Some("/agent:1/agent:2"))));
    assert!(!scope.can_access_user(&user("parent-user", Some("/agent:1"))));
    assert!(!scope.can_access_user(&user("sibling-user", Some("/agent:1/agent:4"))));
    assert!(!scope.can_access_user(&user("organic-user", None)));
}

#[test]
fn agent_hierarchy_derives_three_levels_and_rejects_a_fourth() {
    let root = derive_agent_placement(None, Some(1)).unwrap();
    assert_eq!(root.level, 1);

    let level_one = AgentHierarchyNode {
        id: 10,
        parent_agent_id: None,
        root_agent_id: 10,
        level: 1,
        path: "/agent:10".to_owned(),
        status: "active".to_owned(),
    };
    let level_two = derive_agent_placement(Some(&level_one), Some(2)).unwrap();
    assert_eq!(level_two.parent_agent_id, Some(10));
    assert_eq!(level_two.root_agent_id, Some(10));
    assert_eq!(level_two.level, 2);

    let level_three_parent = AgentHierarchyNode {
        id: 30,
        parent_agent_id: Some(20),
        root_agent_id: 10,
        level: 3,
        path: "/agent:10/agent:20/agent:30".to_owned(),
        status: "active".to_owned(),
    };
    let error = derive_agent_placement(Some(&level_three_parent), None).unwrap_err();
    assert!(error.to_string().contains("at most three levels"));
}

#[test]
fn agent_commission_businesses_allow_all_configurable_trading_products() {
    assert_eq!(
        super::service::normalize_agent_commission_product_type("convert").unwrap(),
        super::service::AGENT_COMMISSION_PRODUCT_CONVERT
    );
    assert_eq!(
        super::service::normalize_agent_commission_product_type(" prediction ").unwrap(),
        super::service::AGENT_COMMISSION_PRODUCT_PREDICTION
    );
    assert_eq!(
        super::service::normalize_agent_commission_product_type("spot").unwrap(),
        super::service::AGENT_COMMISSION_PRODUCT_SPOT
    );
    assert_eq!(
        super::service::normalize_agent_commission_product_type("margin").unwrap(),
        super::service::AGENT_COMMISSION_PRODUCT_MARGIN
    );
    assert_eq!(
        super::service::normalize_agent_commission_product_type("seconds_contract").unwrap(),
        super::service::AGENT_COMMISSION_PRODUCT_SECONDS_CONTRACT
    );
    assert!(super::service::normalize_agent_commission_product_type("earn").is_err());
}
