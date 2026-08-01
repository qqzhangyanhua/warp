#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryCapability {
    Create { initiating_user_text: String },
    Query { initiating_user_text: String },
}

impl MemoryCapability {
    pub(crate) fn derive(user_text: &str) -> Option<Self> {
        let normalized = user_text.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        if is_explicit_query(&normalized) {
            return Some(Self::Query {
                initiating_user_text: user_text.to_string(),
            });
        }
        if is_explicit_create(&normalized) {
            return Some(Self::Create {
                initiating_user_text: user_text.to_string(),
            });
        }
        None
    }
}

fn is_explicit_query(text: &str) -> bool {
    [
        "do you remember",
        "did you remember",
        "what is my",
        "what's my",
        "recall my",
        "remember my ",
        "还记得",
        "记得吗",
        "记得么",
        "记得不",
        "我的",
    ]
    .iter()
    .any(|pattern| text.contains(pattern))
        && (text.contains('?')
            || text.contains('？')
            || text.contains("记得")
            || text.contains("是什么")
            || text.contains("是多少")
            || text.contains("what")
            || text.contains("recall"))
}

fn is_explicit_create(text: &str) -> bool {
    [
        "remember ",
        "remember:",
        "remember that",
        "记住",
        "记一下",
        "记下",
        "帮我保存",
        "请保存这条",
    ]
    .iter()
    .any(|pattern| text.contains(pattern))
}
