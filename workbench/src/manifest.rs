//! Loads the SAME declarative module manifests the React SPA interprets
//! (`modules/<id>/module.js`) - never forked, never TUI-specific. Each file
//! is `osRegisterModule({ ... })`; the object literal is JSON5, so we strip
//! the call wrapper and parse it as data.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct ModuleManifest {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "entityTypes")]
    pub entity_types: HashMap<String, EntityType>,
    #[serde(default)]
    pub views: Vec<View>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityType {
    pub label: String,
    #[serde(default)]
    pub attrs: HashMap<String, AttrSpec>,
    #[serde(default)]
    pub display: Display,
    #[serde(default)]
    pub lifecycle: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttrSpec {
    #[serde(rename = "type")]
    pub attr_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, rename = "enum")]
    pub variants: Vec<String>,
}

/// Which entity fields the renderers surface per row/card.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Display {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub badge: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct View {
    pub id: String,
    pub label: String,
    pub kind: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(default, rename = "groupBy")]
    pub group_by: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
}

/// Extract the object literal from `osRegisterModule({...});`.
fn object_literal(source: &str) -> Option<&str> {
    // Anchor on the call itself - header comments legitimately contain '('.
    let call = source.find("osRegisterModule")?;
    let start = call + source[call..].find('(')? + 1;
    let end = source.rfind(')')?;
    Some(source[start..end].trim())
}

pub fn parse_manifest(source: &str) -> Result<ModuleManifest, String> {
    let literal =
        object_literal(source).ok_or_else(|| "no osRegisterModule(...) call found".to_string())?;
    json5::from_str(literal).map_err(|e| format!("manifest parse error: {e}"))
}

/// Load every `modules/<id>/module.js` under a root, skipping the template
/// and anything unparsable (reported, not fatal - one bad module must not
/// take down the whole workbench).
pub fn load_all(modules_root: &Path) -> (Vec<ModuleManifest>, Vec<String>) {
    let mut manifests = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(modules_root) else {
        return (
            manifests,
            vec![format!("cannot read {}", modules_root.display())],
        );
    };
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let file = dir.join("module.js");
        if !file.is_file() || dir.file_name().is_some_and(|n| n == "_template") {
            continue;
        }
        match std::fs::read_to_string(&file) {
            Ok(src) => match parse_manifest(&src) {
                Ok(m) => manifests.push(m),
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            },
            Err(e) => errors.push(format!("{}: {e}", file.display())),
        }
    }
    (manifests, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASKS: &str = r#"
/** Tasks module */
osRegisterModule({
  id: "tasks",
  name: "Tasks & Productivity",
  entityTypes: {
    task: {
      label: "Task",
      attrs: {
        priority: { type: "enum", enum: ["high", "medium", "low"], required: true },
      },
      display: { title: "title", subtitle: "due", badge: "priority" },
      lifecycle: ["todo", "in_progress", "completed", "blocked"]
    }
  },
  views: [
    { id: "kanban", label: "Task Board", kind: "board", type: "task", groupBy: "status" },
    { id: "today", label: "My Today", kind: "list", type: "task", filter: "status = 'in_progress'" }
  ],
});
"#;

    #[test]
    fn parses_the_untouched_upstream_manifest_shape() {
        let m = parse_manifest(TASKS).expect("parse");
        assert_eq!(m.id, "tasks");
        let task = &m.entity_types["task"];
        assert_eq!(
            task.lifecycle,
            vec!["todo", "in_progress", "completed", "blocked"]
        );
        assert_eq!(task.display.badge.as_deref(), Some("priority"));
        assert_eq!(m.views[0].kind, "board");
        assert_eq!(m.views[0].group_by.as_deref(), Some("status"));
        assert_eq!(m.views[1].filter.as_deref(), Some("status = 'in_progress'"));
    }

    #[test]
    fn loads_the_real_repo_manifests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../modules");
        let (manifests, errors) = load_all(&root);
        assert!(errors.is_empty(), "manifest load errors: {errors:?}");
        assert!(
            manifests.iter().any(|m| m.id == "tasks"),
            "tasks module missing from {}",
            root.display()
        );
    }

    #[test]
    fn unknown_extra_keys_are_ignored_not_fatal() {
        let src = r#"osRegisterModule({ id: "x", name: "X", botCommands: [{cmd: "y"}] });"#;
        assert!(parse_manifest(src).is_ok());
    }
}
