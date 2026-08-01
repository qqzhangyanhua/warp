use warpui::App;

use super::ForTelemetry;
use crate::ai::agent::AIAgentCitation;

#[test]
fn personal_memory_citations_are_excluded_from_telemetry() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let citation = AIAgentCitation::PersonalMemory {
                record_id: "record-1".to_string(),
                content: "private fact".to_string(),
            };

            assert!(citation.for_telemetry(ctx).is_none());
        });
    });
}
