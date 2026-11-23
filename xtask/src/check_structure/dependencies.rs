use super::{add, Rule, Violation};

pub(super) fn check(metadata_json: &str, out: &mut Vec<Violation>) -> bool {
    let metadata: serde_json::Value = match serde_json::from_str(metadata_json) {
        Ok(v) => v,
        Err(e) => {
            add(
                out,
                Rule::ForbiddenCrateDependency,
                "Cargo.toml",
                e.to_string(),
            );
            return false;
        }
    };
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let members = metadata["workspace_members"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let workspace: Vec<_> = packages
        .iter()
        .filter(|p| members.contains(&p["id"]))
        .collect();
    for package in &workspace {
        let name = package["name"].as_str().unwrap_or("");
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dependency = dep["name"].as_str().unwrap_or("");
            let is_workspace = workspace.iter().any(|p| p["name"] == dependency);
            let forbidden = match name {
                "vaio-core" => {
                    is_workspace
                        || dependency.starts_with("gpui")
                        || matches!(dependency, "rusqlite" | "reqwest")
                }
                "vaio-media" | "vaio-storage" | "vaio-platform" => {
                    is_workspace && dependency != "vaio-core"
                }
                "xtask" => is_workspace && !matches!(dependency, "vaio-core" | "vaio-platform"),
                _ => false,
            };
            if forbidden {
                add(
                    out,
                    Rule::ForbiddenCrateDependency,
                    name,
                    format!("depends on {dependency}"),
                );
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, has, map, metadata};
    use super::super::Rule;

    #[test]
    fn core_depending_on_gpui_is_a_violation() {
        assert!(has(
            &fixture(),
            &metadata("vaio-core", &["gpui"]),
            &map(),
            Rule::ForbiddenCrateDependency
        ));
    }

    #[test]
    fn core_depending_on_rusqlite_is_a_violation() {
        assert!(has(
            &fixture(),
            &metadata("vaio-core", &["rusqlite"]),
            &map(),
            Rule::ForbiddenCrateDependency
        ));
    }

    #[test]
    fn media_depending_on_storage_is_a_violation() {
        let meta = serde_json::json!({"workspace_members":["media", "storage"], "packages":[
            {"id":"media", "name":"vaio-media", "dependencies":[{"name":"vaio-storage"}]},
            {"id":"storage", "name":"vaio-storage", "dependencies":[]}
        ]})
        .to_string();
        assert!(has(
            &fixture(),
            &meta,
            &map(),
            Rule::ForbiddenCrateDependency
        ));
    }

    #[test]
    fn desktop_depending_on_all_crates_is_allowed() {
        let names = [
            "vaio-core",
            "vaio-media",
            "vaio-storage",
            "vaio-platform",
            "xtask",
        ];
        let mut packages: Vec<_> = names
            .iter()
            .map(|name| serde_json::json!({"id":name,"name":name,"dependencies":[]}))
            .collect();
        packages.push(serde_json::json!({"id":"desktop","name":"vaio-desktop","dependencies":names.iter().map(|name| serde_json::json!({"name":name})).collect::<Vec<_>>()}));
        let mut members = names.to_vec();
        members.push("desktop");
        let meta = serde_json::json!({"workspace_members":members,"packages":packages}).to_string();
        assert!(!has(
            &fixture(),
            &meta,
            &map(),
            Rule::ForbiddenCrateDependency
        ));
    }
}
