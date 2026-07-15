use crate::ai::agent::runtime::transcript::{
    RuntimeContentBlock, ToolErrorCode, ToolResultProjection,
};

pub(super) fn success_projection(text: &str) -> ToolResultProjection {
    ToolResultProjection::Success {
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
        truncated: false,
    }
}

pub(super) fn assert_error(
    projection: &ToolResultProjection,
    expected_code: ToolErrorCode,
    expected_may_have_executed: bool,
) {
    assert!(matches!(
        projection,
        ToolResultProjection::Error {
            error_code,
            may_have_executed,
            ..
        } if *error_code == expected_code && *may_have_executed == expected_may_have_executed
    ));
}
