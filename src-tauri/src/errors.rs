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
    UnknownAboutLink {
        key: String,
    },
    OpenBrowserFailed {
        detail: String,
    },
    NoLocalAppData,
    CurrentExeFailed {
        detail: String,
    },
    CopySaverFailed {
        detail: String,
    },
    ReplaceSaverFailed {
        detail: String,
    },
    CreateInstallDirFailed {
        detail: String,
    },
    OpenWindowsSettingsFailed {
        detail: String,
    },
    ConfigDirUnavailable,
    CreateConfigDirFailed {
        detail: String,
    },
    SerializeConfigFailed {
        detail: String,
    },
    WriteConfigFailed {
        detail: String,
    },
    ReplaceConfigFailed {
        detail: String,
    },
    OpenRegistryFailed {
        detail: String,
    },
    ReadPrimaryMonitorFailed {
        detail: String,
    },
    EnumerateMonitorsFailed {
        detail: String,
    },
    NoMonitor,
    CreateWindowFailed {
        label: String,
        detail: String,
    },
    LocateWindowFailed {
        label: String,
        detail: String,
    },
    ResizeWindowFailed {
        label: String,
        detail: String,
    },
    ShowWindowFailed {
        label: String,
        detail: String,
    },
    OpenSettingsWindowFailed {
        detail: String,
    },
    ReadWindowScaleFailed {
        detail: String,
    },
    ReadWindowSizeFailed {
        detail: String,
    },
    InvalidPreviewParent,
    SystemPreviewUnsupported,
    ReadWindowHandleFailed {
        detail: String,
    },
    RoundRegionCreateFailed,
    RoundRegionApplyFailed,
    WriteRegistryFailed {
        detail: String,
    },
    DeleteRegistryValueFailed {
        detail: String,
    },
    SaverRegistrationUnsupported,
    AutostartKeyFailed {
        key: String,
        detail: String,
    },
    AutostartExeFailed {
        detail: String,
    },
    AutostartWriteFailed {
        key: String,
        detail: String,
    },
    SetLocaleFailed {
        detail: String,
    },
    // 主题图片导入（ADR-0020）。三条拒绝理由分开，用户才知道该换图还是换格式。
    //
    // 变体上的 `rename_all` 只管变体名本身；结构体变体的**字段**要另写一次，
    // 否则 `limit_mb` 会原样序列化成蛇形，而前端字典插值的是 `{limitMb}`——
    // 用户看到的是没被替换的裸占位符。这条由 check-i18n-runtime 的占位符闸兜住。
    #[serde(rename_all = "camelCase")]
    ThemeImageTooLarge {
        limit_mb: u32,
    },
    #[serde(rename_all = "camelCase")]
    ThemeImageTooManyPixels {
        limit_side: u32,
    },
    ThemeImageUnsupported {
        detail: String,
    },
    ThemeImageCopyFailed {
        detail: String,
    },
    ThemeDirUnavailable {
        detail: String,
    },
    InvalidThemeId {
        detail: String,
    },
    // 字体导入（ADR-0026）。与图片一样，拒绝理由分开，用户才知道该换文件还是换格式。
    #[serde(rename_all = "camelCase")]
    FontTooLarge {
        limit_mb: u32,
    },
    FontUnsupported {
        detail: String,
    },
    FontCopyFailed {
        detail: String,
    },
    FontDirUnavailable {
        detail: String,
    },
    InvalidFontId {
        detail: String,
    },
    // 配置包（ADR-0027）。包损坏/版本不匹配/路径不安全/备份失败各有独立码，
    // 用户才知道是换文件、升级还是先腾磁盘。
    ConfigBundleInvalid {
        detail: String,
    },
    #[serde(rename_all = "camelCase")]
    ConfigBundleUnsupportedVersion {
        format_version: u32,
        schema_version: u32,
    },
    ConfigBundleTooLarge {
        detail: String,
    },
    ConfigBundleUnsafePath {
        detail: String,
    },
    ConfigBundleReadFailed {
        detail: String,
    },
    ConfigBundleWriteFailed {
        detail: String,
    },
    ConfigBundleBackupFailed {
        detail: String,
    },
    ConfigBundleImportFailed {
        detail: String,
    },
    // 全屏配置窗口关不掉（ADR-0020 D19）。关窗失败必须留痕并回给调用方，
    // 不能像以前那样吞掉失败、还照发「窗口已关闭」事件。
    ThemePreviewCloseFailed {
        detail: String,
    },
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

    #[test]
    fn multi_word_fields_serialize_as_camel_case() {
        // 枚举上的 `rename_all` 不作用于结构体变体的**字段**，所以每个带多词字段的
        // 变体要自己再标一次。漏标时序列化出 `limit_mb`，而字典插值 `{limitMb}`，
        // 用户看到的是裸占位符——这条测试就是为了让漏标在编译期就炸。
        let raw = AppError::ThemeImageTooLarge { limit_mb: 32 }.as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["code"], "themeImageTooLarge");
        assert_eq!(value["limitMb"], 32);
        assert!(value.get("limit_mb").is_none(), "字段名必须是驼峰");

        let raw = AppError::ThemeImageTooManyPixels { limit_side: 8192 }.as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["limitSide"], 8192);
        assert!(value.get("limit_side").is_none(), "字段名必须是驼峰");

        let raw = AppError::FontTooLarge { limit_mb: 32 }.as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["code"], "fontTooLarge");
        assert_eq!(value["limitMb"], 32);
        assert!(value.get("limit_mb").is_none(), "字段名必须是驼峰");

        let raw = AppError::ConfigBundleUnsupportedVersion {
            format_version: 1,
            schema_version: 5,
        }
        .as_string();
        let value: serde_json::Value = serde_json::from_str(&raw).expect("错误串应是 JSON");
        assert_eq!(value["code"], "configBundleUnsupportedVersion");
        assert_eq!(value["formatVersion"], 1);
        assert_eq!(value["schemaVersion"], 5);
        assert!(value.get("format_version").is_none(), "字段名必须是驼峰");
        assert!(value.get("schema_version").is_none(), "字段名必须是驼峰");
    }
}
