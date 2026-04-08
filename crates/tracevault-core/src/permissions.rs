#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
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
}
