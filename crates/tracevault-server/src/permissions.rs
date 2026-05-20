use std::collections::HashSet;

// Re-export Permission enum from core
pub use tracevault_core::permissions::Permission;

/// Valid roles in the system.
pub const VALID_ROLES: &[&str] = &["owner", "admin", "policy_admin", "developer", "auditor"];

pub fn is_valid_role(role: &str) -> bool {
    VALID_ROLES.contains(&role)
}

pub fn role_permissions(role: &str) -> HashSet<Permission> {
    use Permission::*;
    match role {
        "owner" => HashSet::from([
            TracePush,
            TraceViewAll,
            TraceViewOwn,
            PolicyManage,
            AuditLogView,
            UserManage,
            OrgSettingsManage,
            ComplianceManage,
            ComplianceView,
            CodeBrowse,
            StoryGenerate,
            StoryView,
            ChatUse,
        ]),
        "admin" => HashSet::from([
            TracePush,
            TraceViewAll,
            TraceViewOwn,
            PolicyManage,
            AuditLogView,
            UserManage,
            OrgSettingsManage,
            ComplianceManage,
            ComplianceView,
            CodeBrowse,
            StoryGenerate,
            StoryView,
            ChatUse,
        ]),
        "policy_admin" => HashSet::from([
            TracePush,
            TraceViewAll,
            TraceViewOwn,
            PolicyManage,
            AuditLogView,
            ComplianceView,
            CodeBrowse,
            StoryGenerate,
            StoryView,
        ]),
        "developer" => HashSet::from([
            TracePush,
            TraceViewOwn,
            CodeBrowse,
            StoryGenerate,
            StoryView,
            ChatUse,
        ]),
        "auditor" => HashSet::from([
            TraceViewAll,
            TraceViewOwn,
            AuditLogView,
            ComplianceView,
            CodeBrowse,
            StoryView,
        ]),
        _ => HashSet::new(),
    }
}

pub fn has_permission(role: &str, perm: Permission) -> bool {
    role_permissions(role).contains(&perm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_role_all_roles() {
        for role in VALID_ROLES {
            assert!(is_valid_role(role), "Expected {} to be valid", role);
        }
    }

    #[test]
    fn is_valid_role_invalid() {
        assert!(!is_valid_role("superadmin"));
        assert!(!is_valid_role(""));
    }

    #[test]
    fn owner_has_12_permissions() {
        assert_eq!(role_permissions("owner").len(), 13);
    }

    #[test]
    fn admin_same_as_owner() {
        assert_eq!(role_permissions("admin"), role_permissions("owner"));
    }

    #[test]
    fn developer_has_5_permissions() {
        let perms = role_permissions("developer");
        assert_eq!(perms.len(), 6);
        assert!(perms.contains(&Permission::TracePush));
        assert!(perms.contains(&Permission::TraceViewOwn));
        assert!(perms.contains(&Permission::CodeBrowse));
    }

    #[test]
    fn auditor_has_6_permissions() {
        let perms = role_permissions("auditor");
        assert_eq!(perms.len(), 6);
        assert!(perms.contains(&Permission::AuditLogView));
        assert!(perms.contains(&Permission::ComplianceView));
    }

    #[test]
    fn invalid_role_empty_permissions() {
        assert!(role_permissions("invalid").is_empty());
    }

    #[test]
    fn has_permission_developer_trace_push() {
        assert!(has_permission("developer", Permission::TracePush));
    }

    #[test]
    fn has_permission_developer_no_user_manage() {
        assert!(!has_permission("developer", Permission::UserManage));
    }

    #[test]
    fn has_permission_auditor_audit_log_view() {
        assert!(has_permission("auditor", Permission::AuditLogView));
    }
}
