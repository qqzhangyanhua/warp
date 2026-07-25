use super::{Notebook, SerializedNotebook};

#[test]
fn notebook_round_trips_json() {
    let notebook = Notebook::new("Notes", "# Hello")
        .with_ai_document_id("11111111-1111-1111-1111-111111111111")
        .with_conversation_id("conv-1");

    let json = serde_json::to_value(&notebook).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({
            "title": "Notes",
            "data": "# Hello",
            "ai_document_id": "11111111-1111-1111-1111-111111111111",
            "conversation_id": "conv-1",
        })
    );

    let parsed: Notebook = serde_json::from_value(json).expect("deserialize");
    assert_eq!(parsed, notebook);
    assert!(parsed.is_plan());
}

#[test]
fn notebook_defaults_optional_ids() {
    let parsed: Notebook = serde_json::from_str(r#"{"title":"t","data":"body"}"#)
        .expect("deserialize minimal notebook");
    assert_eq!(parsed.title, "t");
    assert_eq!(parsed.data, "body");
    assert!(parsed.ai_document_id.is_none());
    assert!(parsed.conversation_id.is_none());
    assert!(!parsed.is_plan());
}

#[test]
fn serialized_notebook_round_trips_and_rebuilds() {
    let notebook = Notebook::new("Plan", "steps")
        .with_ai_document_id("doc-1")
        .with_conversation_id("c-1");
    let serialized = notebook.to_serialized();
    assert_eq!(
        serialized,
        SerializedNotebook {
            data: "steps".to_string(),
            ai_document_id: Some("doc-1".to_string()),
            conversation_id: Some("c-1".to_string()),
        }
    );

    let wire = serde_json::to_string(&serialized).expect("serialize wire");
    let parsed: SerializedNotebook = serde_json::from_str(&wire).expect("deserialize wire");
    let rebuilt = Notebook::from_serialized("Plan", parsed);
    assert_eq!(rebuilt, notebook);
}
