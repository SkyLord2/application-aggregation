#![cfg(windows)]

use uuid::Uuid;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use xiaohai_core::manifest::{
    RegistryExpectedValue, RegistryHive, RegistryValueKind, RegistryValueRule,
};

#[test]
fn detect_registry_rule_dword_at_least_hkcu() {
    let (key_path, _guard) = create_test_key();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _disp) = hkcu.create_subkey(&key_path).expect("create subkey");
    key.set_value("Release", &528040u32).expect("set dword");

    let rule = RegistryValueRule {
        hive: RegistryHive::Hkcu,
        key: key_path.clone(),
        value_name: "Release".to_string(),
        kind: RegistryValueKind::Dword,
        expected: RegistryExpectedValue::DwordAtLeast(528040),
    };
    let ok = xiaohai_windows::registry::detect_registry_rule(&rule).expect("detect rule");
    assert!(ok);

    let rule2 = RegistryValueRule {
        expected: RegistryExpectedValue::DwordAtLeast(528041),
        ..rule
    };
    let ok2 = xiaohai_windows::registry::detect_registry_rule(&rule2).expect("detect rule");
    assert!(!ok2);
}

#[test]
fn detect_registry_rule_sz_equals_hkcu() {
    let (key_path, _guard) = create_test_key();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _disp) = hkcu.create_subkey(&key_path).expect("create subkey");
    key.set_value("ServerUrl", &"https://example.invalid")
        .expect("set sz");

    let rule = RegistryValueRule {
        hive: RegistryHive::Hkcu,
        key: key_path.clone(),
        value_name: "ServerUrl".to_string(),
        kind: RegistryValueKind::Sz,
        expected: RegistryExpectedValue::SzEquals("https://example.invalid".to_string()),
    };
    let ok = xiaohai_windows::registry::detect_registry_rule(&rule).expect("detect rule");
    assert!(ok);

    let rule2 = RegistryValueRule {
        expected: RegistryExpectedValue::SzEquals("https://nope.invalid".to_string()),
        ..rule
    };
    let ok2 = xiaohai_windows::registry::detect_registry_rule(&rule2).expect("detect rule");
    assert!(!ok2);
}

fn create_test_key() -> (String, CleanupKey) {
    let path = format!("Software\\XiaoHaiAssistantTest\\{}", Uuid::new_v4());
    (path.clone(), CleanupKey(path))
}

struct CleanupKey(String);

impl Drop for CleanupKey {
    fn drop(&mut self) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(&self.0);
    }
}
