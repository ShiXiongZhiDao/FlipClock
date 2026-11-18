//! 开机自启：只写当前用户的 Run 键，不需要管理员权限。
//!
//! 「是否启用」以注册表为准，不额外存一份布尔值——否则用户在任务管理器里
//! 禁用之后，设置页会继续显示「已开启」。任务管理器的开关写在
//! `StartupApproved\Run`，首字节 0x03 表示被禁用，这里一并读。

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE_NAME: &str = "FlipClock";
const TRAY_FLAG: &str = "--tray";

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(Some(0)).collect()
}

fn open(path: &str, write: bool) -> Result<HKEY, String> {
    let sub = wide(path);
    let mut key = HKEY::default();
    let access = if write { KEY_SET_VALUE } else { KEY_QUERY_VALUE };
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            None,
            access,
            &mut key,
        )
    };
    if code == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(format!("打开注册表键失败（{}）：{code:?}", code.0))
    }
}

fn read(path: &str, name: &str) -> Option<Vec<u8>> {
    let key = open(path, false).ok()?;
    let name_w = wide(name);
    let mut size = 0u32;
    let mut kind = REG_BINARY;
    let code = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if code != ERROR_SUCCESS || size == 0 {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return None;
    }
    let mut data = vec![0u8; size as usize];
    let code = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut kind),
            Some(data.as_mut_ptr()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    (code == ERROR_SUCCESS).then_some(data)
}

/// 注册表里存的命令行：带引号的绝对路径 + `--tray`（自启只驻留托盘）。
pub fn command_line() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("取程序路径失败：{e}"))?;
    Ok(format!("\"{}\" {TRAY_FLAG}", exe.display()))
}

pub fn is_enabled() -> bool {
    let Some(bytes) = read(RUN_KEY, VALUE_NAME) else {
        return false;
    };
    if bytes.is_empty() {
        return false;
    }
    if let Some(approved) = read(APPROVED_KEY, VALUE_NAME) {
        if approved.first() == Some(&0x03) {
            return false;
        }
    }
    true
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let key = open(RUN_KEY, true)?;
    let name = wide(VALUE_NAME);
    let code = if enabled {
        let command = command_line()?;
        let mut data = Vec::with_capacity((command.len() + 1) * 2);
        for unit in wide(&command) {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&data)) }
    } else {
        unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if code != ERROR_SUCCESS && code != ERROR_FILE_NOT_FOUND {
        return Err(format!("写入启动项失败（{}）：{code:?}", code.0));
    }

    if enabled {
        // 清掉任务管理器留下的禁用标记，否则刚打开仍被压在启动项之外。
        if let Ok(approved) = open(APPROVED_KEY, true) {
            unsafe {
                let _ = RegDeleteValueW(approved, PCWSTR(name.as_ptr()));
                let _ = RegCloseKey(approved);
            }
        }
    }
    Ok(())
}

/// 程序被移动过之后，注册表里的旧路径会让自启悄悄失效；启动时对齐一次。
pub fn sync_command_line() {
    if !is_enabled() {
        return;
    }
    let Ok(expected) = command_line() else {
        return;
    };
    let Some(bytes) = read(RUN_KEY, VALUE_NAME) else {
        return;
    };
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let stored = String::from_utf16_lossy(&units);
    if stored.trim_end_matches('\0') != expected {
        let _ = set_enabled(true);
    }
}
