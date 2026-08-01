use super::MemoryCapability;

#[test]
fn explicit_english_and_chinese_create_requests_enable_only_create() {
    assert!(matches!(
        MemoryCapability::derive("Remember my GitHub account is zyh-work"),
        Some(MemoryCapability::Create { .. })
    ));
    assert!(matches!(
        MemoryCapability::derive("帮我记住我的 GitHub 帐号是 zyh-work"),
        Some(MemoryCapability::Create { .. })
    ));
}

#[test]
fn explicit_queries_are_classified_before_create_phrases() {
    assert!(matches!(
        MemoryCapability::derive("Do you remember my GitHub account?"),
        Some(MemoryCapability::Query { .. })
    ));
    assert!(matches!(
        MemoryCapability::derive("我的 GitHub 帐号记得么"),
        Some(MemoryCapability::Query { .. })
    ));
}

#[test]
fn ordinary_fact_mentions_expose_no_memory_capability() {
    assert_eq!(
        MemoryCapability::derive("My GitHub account is zyh-work"),
        None
    );
    assert_eq!(
        MemoryCapability::derive("我的 GitHub 帐号是 zyh-work"),
        None
    );
}
