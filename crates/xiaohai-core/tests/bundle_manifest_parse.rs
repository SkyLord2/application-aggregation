use std::path::PathBuf;

use xiaohai_core::manifest::{BundleManifest, DetectRule, Healthcheck, ModuleKind};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn parse_real_bundle_manifest_json() {
    let manifest_path = repo_root().join("bundle-manifest.json");
    let bytes = std::fs::read(&manifest_path)
        .unwrap_or_else(|e| panic!("read {} failed: {e}", manifest_path.display()));
    let manifest: BundleManifest = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse {} failed: {e}", manifest_path.display()));

    assert!(!manifest.product_name.trim().is_empty());
    assert!(!manifest.product_code.trim().is_empty());
    assert!(!manifest.version.trim().is_empty());
    assert!(!manifest.install_root.trim().is_empty());
    assert!(!manifest.modules.is_empty());

    let demo = manifest
        .modules
        .iter()
        .find(|m| m.id == "demo-filecopy-app")
        .expect("demo-filecopy-app module must exist in bundle-manifest.json");
    assert!(demo.enabled);
    assert!(matches!(demo.kind, ModuleKind::FileCopy));
    assert!(
        matches!(demo.detect, DetectRule::FileExists(_)),
        "demo-filecopy-app should use file_exists detect rule"
    );
    let plugin = demo
        .plugin
        .as_ref()
        .expect("demo-filecopy-app should have plugin registration");
    assert!(
        matches!(plugin.healthcheck, Some(Healthcheck::Process)),
        "demo-filecopy-app healthcheck should be process"
    );

    let disabled = manifest
        .modules
        .iter()
        .find(|m| !m.enabled)
        .expect("bundle-manifest.json should contain at least one disabled module");
    assert!(
        matches!(disabled.detect, DetectRule::None),
        "disabled module detect should deserialize"
    );
}
