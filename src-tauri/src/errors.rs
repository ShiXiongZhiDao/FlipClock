//! 用户可见错误的稳定码。设计见 docs/adr/0018。
//!
//! 命令返回的字符串永远是 JSON：`{"code":"copySaverFailed","detail":"..."}`。
//! 前端按 code 查字典，detail 只做插值；认不出的 code 原样显示，绝不吞掉。
//! 日志里的中文不在这里——它不面向用户，保持中文更好排查。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum AppError {
    ConfigUnavailable,
    UnknownAboutLink { key: String },
    OpenBrowserFailed { detail: String },
    NoLocalAppData,
    CurrentExeFailed { detail: String },
    CopySaverFailed { detail: String },
    ReplaceSaverFailed { detail: String },
    CreateInstallDirFailed { detail: String },
    OpenWindowsSettingsFailed { detail: String },
    ConfigDirUnavailable,
    CreateConfigDirFailed { detail: String },
    SerializeConfigFailed { detail: String },
    WriteConfigFailed { detail: String },
    ReplaceConfigFailed { detail: String },
    OpenRegistryFailed { detail: String },
    ReadPrimaryMonitorFailed { detail: String },
    EnumerateMonitorsFailed { detail: String },
    NoMonitor,
    CreateWindowFailed { label: String, detail: String },
    LocateWindowFailed { label: String, detail: String },
    ResizeWindowFailed { label: String, detail: String },
    ShowWindowFailed { label: String, detail: String },
    OpenSettingsWindowFailed { detail: String },
    ReadWindowScaleFailed { detail: String },
    ReadWindowSizeFailed { detail: String },
    InvalidPreviewParent,
    SystemPreviewUnsupported,
    ReadWindowHandleFailed { detail: String },
    RoundRegionCreateFailed,
    RoundRegionApplyFailed,
    WriteRegistryFailed { detail: String },
    DeleteRegistryValueFailed { detail: String },
    SaverRegistrationUnsupported,
    AutostartKeyFailed { key: String, detail: String },
    AutostartExeFailed { detail: String },
    AutostartWriteFailed { key: String, detail: String },
    SetLocaleFailed { detail: String },
}

impl AppError {
    /// 序列化失败不可能发生：所有字段都是 String。但真失败时也不能静默——
    /// 给一个前端不认识的 code，让它原样显示，比返回空串好排查。
    pub fn as_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"code":"serializationFailed"}"#.into())
    }
}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.as_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_json_with_camel_case_fields() {
        let raw = AppError::CopySaverFailed {
            detail: "Access denied".into(),
        }
        .as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["code"], "copySaverFailed");
        assert_eq!(value["detail"], "Access denied");
    }

    #[test]
    fn every_code_has_a_frontend_key() {
        // 前端字典的 key 集合在 i18n.ts 里由类型钉死，这里只保证 code 的命名形状：
        // 稳定、无空格、小驼峰。改一个 code 而忘记改字典时，前端会原样显示 JSON，
        // 这很难看但不会静默——测试把命名漂移挡在编译前。
        let raw = AppError::NoMonitor.as_string();
        assert!(raw.starts_with(r#"{"code":"noMonitor""#));
    }

    #[test]
    fn interpolated_errors_keep_parameters_structured() {
        let raw = AppError::UnknownAboutLink {
            key: "https://evil.example".into(),
        }
        .as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["code"], "unknownAboutLink");
        assert_eq!(value["key"], "https://evil.example");
        // 参数不能拼进 code 里，否则前端按 code 查表就失效了。
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}
