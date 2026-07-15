use std::collections::HashMap;

use tempfile::TempDir;
use warp_multi_agent_api as api;

use super::resources::{ResourceSnapshotBuilder, ResourceSnapshotError};
use super::transcript::RuntimeContentBlock;

#[test]
fn builds_snapshots_from_retained_selected_content_after_sources_change() {
    let source_dir = TempDir::new().unwrap();
    let rule_path = source_dir.path().join("AGENTS.md");
    std::fs::write(&rule_path, "current instructions").unwrap();
    let mut attachments = HashMap::new();
    attachments.insert(
        "notes".to_string(),
        api::Attachment {
            value: Some(api::attachment::Value::PlainText(
                "retained attachment".to_string(),
            )),
        },
    );
    let user_message = api::Message {
        id: "user-1".to_string(),
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            query: "Inspect the repository".to_string(),
            context: Some(api::InputContext {
                project_rules: vec![api::input_context::ProjectRules {
                    root_path: source_dir.path().display().to_string(),
                    active_rule_files: vec![api::FileContent {
                        file_path: rule_path.display().to_string(),
                        content: "retained instructions".to_string(),
                        line_range: None,
                    }],
                    additional_rule_file_paths: vec![],
                }],
                ..Default::default()
            }),
            referenced_attachments: attachments,
            mode: None,
            intended_agent: Default::default(),
        })),
        ..Default::default()
    };
    let skill_message = api::Message {
        id: "skill-1".to_string(),
        message: Some(api::message::Message::InvokeSkill(
            api::message::InvokeSkill {
                skill: Some(api::Skill {
                    descriptor: Some(api::SkillDescriptor {
                        name: "review".to_string(),
                        ..Default::default()
                    }),
                    content: Some(api::FileContent {
                        file_path: "/skills/review/SKILL.md".to_string(),
                        content: "retained skill instructions".to_string(),
                        line_range: None,
                    }),
                }),
                user_query: None,
            },
        )),
        ..Default::default()
    };

    let snapshots = ResourceSnapshotBuilder::default()
        .build([&user_message, &skill_message])
        .unwrap();

    assert_eq!(
        snapshots
            .iter()
            .map(|snapshot| (snapshot.name.as_str(), snapshot.content.as_slice()))
            .collect::<Vec<_>>(),
        vec![
            (
                rule_path.to_str().unwrap(),
                [RuntimeContentBlock::Text {
                    text: "retained instructions".to_string(),
                }]
                .as_slice(),
            ),
            (
                "notes",
                [RuntimeContentBlock::Text {
                    text: "retained attachment".to_string(),
                }]
                .as_slice(),
            ),
            (
                "review",
                [RuntimeContentBlock::Text {
                    text: "retained skill instructions".to_string(),
                }]
                .as_slice(),
            ),
        ]
    );
    assert_eq!(
        std::fs::read_to_string(rule_path).unwrap(),
        "current instructions"
    );
}

#[test]
fn rejects_local_path_attachments_instead_of_resolving_them() {
    let message = api::Message {
        id: "user-1".to_string(),
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            referenced_attachments: HashMap::from([(
                "secret".to_string(),
                api::Attachment {
                    value: Some(api::attachment::Value::FilePathReference(
                        api::FilePathReference {
                            file_path: "/private/secret.txt".to_string(),
                        },
                    )),
                },
            )]),
            ..Default::default()
        })),
        ..Default::default()
    };

    assert_eq!(
        ResourceSnapshotBuilder::default().build([&message]),
        Err(ResourceSnapshotError::UnsupportedPathReference)
    );
}
