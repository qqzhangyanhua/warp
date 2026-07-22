use thiserror::Error;
use toml_edit::{DocumentMut, Item, Table};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingDisposition {
    Copy,
    #[allow(dead_code)]
    Rename(&'static str),
    OmitCloud,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SettingRule {
    source: &'static str,
    disposition: SettingDisposition,
}

impl SettingRule {
    pub(crate) const fn new(source: &'static str, disposition: SettingDisposition) -> Self {
        Self {
            source,
            disposition,
        }
    }

    pub(crate) const fn copy(source: &'static str) -> Self {
        Self::new(source, SettingDisposition::Copy)
    }

    pub(crate) const fn omit_cloud(source: &'static str) -> Self {
        Self::new(source, SettingDisposition::OmitCloud)
    }
}

#[derive(Debug)]
pub(crate) struct SettingsTranslation {
    pub(crate) settings: DocumentMut,
    pub(crate) omitted_keys: Vec<String>,
    pub(crate) unknown_keys: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum SettingsTranslationError {
    #[error("legacy settings contain malformed TOML")]
    Malformed,
}

pub(crate) fn translate_legacy_settings(
    source: &str,
    rules: &[SettingRule],
) -> Result<SettingsTranslation, SettingsTranslationError> {
    let source = source
        .parse::<DocumentMut>()
        .map_err(|_| SettingsTranslationError::Malformed)?;
    let mut leaves = Vec::new();
    collect_leaves(source.as_table(), "", &mut leaves);

    let mut settings = DocumentMut::new();
    let mut omitted_keys = Vec::new();
    let mut unknown_keys = Vec::new();

    for (path, item) in leaves {
        let Some(rule) = rules.iter().find(|rule| rule.source == path) else {
            unknown_keys.push(path);
            continue;
        };

        match rule.disposition {
            SettingDisposition::Copy => insert_item(settings.as_table_mut(), &path, item),
            SettingDisposition::Rename(destination) => {
                insert_item(settings.as_table_mut(), destination, item)
            }
            SettingDisposition::OmitCloud => omitted_keys.push(path),
        }
    }

    Ok(SettingsTranslation {
        settings,
        omitted_keys,
        unknown_keys,
    })
}

fn collect_leaves(table: &Table, prefix: &str, leaves: &mut Vec<(String, Item)>) {
    for (key, item) in table {
        let path = if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        };

        if let Some(table) = item.as_table() {
            collect_leaves(table, &path, leaves);
        } else {
            leaves.push((path, item.clone()));
        }
    }
}

fn insert_item(table: &mut Table, path: &str, item: Item) {
    let mut segments = path.split('.').peekable();
    let mut current = table;

    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            current.insert(segment, item);
            return;
        }

        if !current.contains_key(segment) {
            current.insert(segment, Item::Table(Table::new()));
        }
        current = current[segment]
            .as_table_mut()
            .expect("settings rules must not map a value beneath another value");
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
