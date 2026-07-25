use super::{Argument, ArgumentType, ObjectRef, Workflow};

fn assert_workflow_roundtrips(workflow: &Workflow) {
    let serialized = serde_json::to_string(workflow).expect("Serialized workflow.");
    let deserialized =
        serde_json::from_str::<Workflow>(&serialized).expect("Deserialized workflow.");
    assert_eq!(&deserialized, workflow);
}

#[test]
fn test_workflow_serialization_with_enum_params() {
    let workflow = Workflow::Command {
        name: "name".to_string(),
        command: "command".to_string(),
        arguments: vec![
            Argument {
                name: "text".to_string(),
                arg_type: ArgumentType::Text,
                description: None,
                default_value: Some("default".to_string()),
            },
            Argument {
                name: "server id enum".to_string(),
                arg_type: ArgumentType::Enum {
                    enum_id: ObjectRef::new("test_uid00000000000123"),
                },
                description: Some("description".to_string()),
                default_value: None,
            },
            Argument {
                name: "client id enum".to_string(),
                arg_type: ArgumentType::Enum {
                    enum_id: ObjectRef::new("Client-06d26381-ac61-4a4a-8a23-a3431f1d340c"),
                },
                description: Some("description".to_string()),
                default_value: None,
            },
        ],
        description: None,
        source_url: None,
        author: None,
        author_url: None,
        shells: vec![],
        tags: vec![],
        environment_variables: None,
    };

    let serialized = serde_json::to_string(&workflow).expect("failed to serialize");
    let correct_serialized = r#"{"name":"name","command":"command","tags":[],"description":null,"arguments":[{"name":"text","arg_type":"Text","description":null,"default_value":"default"},{"name":"server id enum","arg_type":"Enum","enum_id":"test_uid00000000000123","description":"description","default_value":null},{"name":"client id enum","arg_type":"Enum","enum_id":"Client-06d26381-ac61-4a4a-8a23-a3431f1d340c","description":"description","default_value":null}],"source_url":null,"author":null,"author_url":null,"shells":[],"environment_variables":null}"#;

    assert_eq!(
        serialized, correct_serialized,
        "Workflow should serialize correctly"
    );

    let deserialized: Workflow =
        serde_json::from_str(serialized.as_str()).expect("failed to deserialized");

    assert_eq!(deserialized, workflow);
}

#[test]
fn test_agent_mode_workflow_serialization() {
    let workflow = Workflow::AgentMode {
        name: "name".to_string(),
        query: "query {{text}}".to_string(),
        arguments: vec![Argument {
            name: "text".to_string(),
            arg_type: ArgumentType::Text,
            description: None,
            default_value: Some("default".to_string()),
        }],
        description: None,
    };

    let serialized = serde_json::to_string(&workflow).expect("failed to serialize");
    let correct_serialized = r#"{"type":"agent_mode","name":"name","query":"query {{text}}","arguments":[{"name":"text","arg_type":"Text","description":null,"default_value":"default"}]}"#;

    assert_eq!(
        serialized, correct_serialized,
        "Workflow should serialize correctly"
    );

    let deserialized: Workflow =
        serde_json::from_str(serialized.as_str()).expect("failed to deserialized");

    assert_eq!(deserialized, workflow);
}

#[test]
fn test_serialize_local_workflow() {
    let sample_workflow = Workflow::new("Test name", "Command name");
    assert_workflow_roundtrips(&sample_workflow);

    let arguments = vec![Argument {
        name: "Argument".to_string(),
        description: Some("no".to_string()),
        default_value: None,
        arg_type: Default::default(),
    }];
    let arguments_workflow = sample_workflow.clone().with_arguments(arguments);
    assert_workflow_roundtrips(&arguments_workflow);

    let description_workflow = sample_workflow.with_description("cool description".to_string());
    assert_workflow_roundtrips(&description_workflow);

    let workflow_with_additional_fields = Workflow::Command {
        name: "Test".to_string(),
        command: "Command".to_string(),
        tags: vec![],
        description: None,
        arguments: vec![],
        source_url: Some("url".to_string()),
        author: Some("author_name".to_string()),
        author_url: None,
        shells: vec![],
        environment_variables: Some(ObjectRef::new("test_uid00000000000123")),
    };
    assert_workflow_roundtrips(&workflow_with_additional_fields);
}

#[test]
fn local_models_has_no_cloud_crate_in_name_contract() {
    // Package-level contract: this crate is the non-cloud home for Workflow.
    assert_eq!(env!("CARGO_PKG_NAME"), "local_models");
}
