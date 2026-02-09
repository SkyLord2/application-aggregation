use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent"))
        .unwrap_or_else(|e| panic!("create parent for {} failed: {e}", path.display()));
    std::fs::write(path, content).unwrap_or_else(|e| panic!("write {} failed: {e}", path.display()));
}

#[test]
fn e2e_install_then_uninstall_filecopy_in_sandbox() {
    let root = unique_temp_dir("xiaohai-bootstrapper-e2e");
    let _cleanup = CleanupDir(root.clone());

    let program_data = root.join("ProgramData");
    let install_root = root.join("InstallRoot");
    let payload_root = root.join("payload");

    write_file(
        &payload_root.join("myapp").join("nested").join("hello.txt"),
        "hello",
    );

    let manifest_json = format!(
        r#"
{{
  "product_name": "TestProduct",
  "product_code": "test-product",
  "version": "0.0.0",
  "install_root": "{}",
  "prerequisites": {{}},
  "modules": [
    {{
      "id": "module_a",
      "display_name": "ModuleA",
      "enabled": true,
      "kind": "file_copy",
      "detect": "none",
      "payload": {{ "path": "payload/myapp", "install_subdir": "appdir" }},
      "installer": null,
      "uninstaller": null,
      "remove_desktop_shortcuts": [],
      "plugin": {{
        "id": "plugin_a",
        "name": "PluginA",
        "exe": "appdir/nested/hello.txt",
        "args": [],
        "icon": null,
        "healthcheck": "process"
      }},
      "config": {{
        "server_url": null,
        "data_subdir": "module_a",
        "file_replacements": []
      }}
    }}
  ],
  "shortcuts": {{
    "assistant_exe": "xiaohai-assistant.exe",
    "assistant_name": "XiaoHai",
    "start_menu": false,
    "desktop": false
  }},
  "post_config": {{
    "server_url": null,
    "data_root": null,
    "plugin_dir": null
  }},
  "firewall": {{ "enabled": false, "rules": [] }},
  "service": {{ "enabled": false, "name": "", "display_name": "", "description": "", "exe": "", "args": [] }},
  "autorun": {{ "enabled": false, "name": "", "command": "" }}
}}
"#,
        escape_json_string(&install_root.to_string_lossy())
    );

    let manifest_path = root.join("bundle-manifest.json");
    write_file(&manifest_path, &manifest_json);

    let exe = env!("CARGO_BIN_EXE_xiaohai-bootstrapper");

    let install_out = Command::new(exe)
        .env("XIAOHAI_TEST_ALLOW_NON_ADMIN", "1")
        .env("ProgramData", &program_data)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--silent")
        .arg("install")
        .output()
        .expect("run install");
    assert!(
        install_out.status.success(),
        "install failed: status={:?}, stdout={}, stderr={}",
        install_out.status.code(),
        String::from_utf8_lossy(&install_out.stdout),
        String::from_utf8_lossy(&install_out.stderr)
    );

    let installed_file = install_root.join("appdir").join("nested").join("hello.txt");
    assert!(installed_file.exists(), "expected installed file: {}", installed_file.display());

    let vendor_dir = program_data.join("XiaoHaiAssistant");
    let plugins_dir = vendor_dir.join("plugins");
    let state_file = vendor_dir.join("install-state.json");
    assert!(plugins_dir.join("plugin_a.json").exists(), "expected plugin json");
    assert!(state_file.exists(), "expected state file");

    let uninstall_out = Command::new(exe)
        .env("XIAOHAI_TEST_ALLOW_NON_ADMIN", "1")
        .env("ProgramData", &program_data)
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--silent")
        .arg("uninstall")
        .output()
        .expect("run uninstall");
    assert!(
        uninstall_out.status.success(),
        "uninstall failed: status={:?}, stdout={}, stderr={}",
        uninstall_out.status.code(),
        String::from_utf8_lossy(&uninstall_out.stdout),
        String::from_utf8_lossy(&uninstall_out.stderr)
    );

    assert!(!install_root.exists(), "install_root should be removed");
    assert!(!vendor_dir.exists(), "ProgramData vendor dir should be removed");
}

fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

struct CleanupDir(PathBuf);

impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

