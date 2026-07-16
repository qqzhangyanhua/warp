use super::super::transcript::{RuntimeContentBlock, ToolErrorCode, ToolResultProjection};

pub(super) fn unknown_outcome_projection() -> ToolResultProjection {
    error_projection(
        ToolErrorCode::ToolOutcomeUnknown,
        true,
        "Warp cannot determine whether the previous tool effect completed.",
    )
}

pub(super) fn error_projection(
    error_code: ToolErrorCode,
    may_have_executed: bool,
    text: &str,
) -> ToolResultProjection {
    ToolResultProjection::Error {
        error_code,
        may_have_executed,
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
        truncated: false,
    }
}

pub(super) fn projection_ends_run(projection: &ToolResultProjection) -> bool {
    matches!(
        projection,
        ToolResultProjection::Error {
            error_code: ToolErrorCode::ToolRequestLimitExceeded | ToolErrorCode::ToolOutcomeUnknown,
            ..
        }
    )
}
