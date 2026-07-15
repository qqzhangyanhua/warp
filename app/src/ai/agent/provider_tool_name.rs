use uuid::Uuid;

const MAX_PROVIDER_TOOL_NAME_LEN: usize = 64;

pub(crate) fn mcp_provider_name(server_id: Uuid, tool_name: &str) -> String {
    let sanitized = tool_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let hash = fnv1a_32(format!("{server_id}\0{tool_name}").as_bytes());
    let prefix = format!("mcp_{}", server_id.simple());
    let suffix = format!("_{hash:08x}");
    let available_name_len =
        MAX_PROVIDER_TOOL_NAME_LEN.saturating_sub(prefix.len() + suffix.len() + 1);
    let sanitized = sanitized
        .chars()
        .take(available_name_len)
        .collect::<String>();
    format!("{prefix}_{sanitized}{suffix}")
}

fn fnv1a_32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
    })
}
