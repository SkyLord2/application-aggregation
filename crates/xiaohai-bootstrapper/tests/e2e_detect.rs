use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    std::fs::write(path, content)
        .unwrap_or_else(|e| panic!("write {} failed: {e}", path.display()));
}

#[test]
fn e2e_detect_reports_file_exists_correctly() {
    let dir = unique_temp_dir("xiaohai-bootstrapper-detect");
    let _cleanup = CleanupDir(dir.clone());

    write_file(&dir.join("present.txt"), "ok");

    let manifest_json = r#"
{
  "product_name": "TestProduct",
  "product_code": "test-product",
  "version": "0.0.0",
  "install_root": "C:\\\\Test\\\\InstallRoot",
  "prerequisites": {},
  "modules": [
    {
      "id": "present",
      "display_name": "PresentModule",
      "enabled": true,
      "kind": "file_copy",
      "detect": { "file_exists": { "path": "present.txt" } },
      "payload": null,
      "installer": null,
      "uninstaller": null,
      "remove_desktop_shortcuts": [],
      "plugin": null,
      "config": {}
    },
    {
      "id": "missing",
      "display_name": "MissingModule",
      "enabled": true,
      "kind": "file_copy",
      "detect": { "file_exists": { "path": "missing.txt" } },
      "payload": null,
      "installer": null,
      "uninstaller": null,
      "remove_desktop_shortcuts": [],
      "plugin": null,
      "config": {}
    }
  ],
  "shortcuts": {
    "assistant_exe": "xiaohai-assistant.exe",
    "assistant_name": "XiaoHai"
  },
  "post_config": {},
  "firewall": {},
  "service": {},
  "autorun": { "enabled": false, "name": "", "command": "" }
}
"#;

    let manifest_path = dir.join("bundle-manifest.json");
    write_file(&manifest_path, manifest_json);

    let exe = env!("CARGO_BIN_EXE_xiaohai-bootstrapper");
    let out = Command::new(exe)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("detect")
        .output()
        .expect("run xiaohai-bootstrapper detect");

    assert!(
        out.status.success(),
        "detect failed: status={:?}, stdout={}, stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("PresentModule (present) = true"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("MissingModule (missing) = false"),
        "stdout: {stdout}"
    );
}

struct CleanupDir(PathBuf);

impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
