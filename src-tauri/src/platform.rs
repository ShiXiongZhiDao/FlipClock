//! 平台适配层。当前只实现 Windows，其余平台给出空实现以免编译期噪音。

#[cfg(windows)]
mod imp {
    use tauri::WebviewWindow;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, IsWindow};

    use crate::errors::AppError;

    const DESKTOP_KEY: &str = r"Control Panel\Desktop";

    /// 用窗口区域把方角裁成圆角。
    ///
    /// Windows 10 没有 `DWMWA_WINDOW_CORNER_PREFERENCE`（Win11 才有），而无边框
    /// 透明窗口又依赖 DWM 的透明效果：系统把「透明效果」关掉时，WebView2 的透明
    /// 像素会合成成黑色，圆角外面就是一圈黑边。区域裁剪不经过 DWM，任何设置下都
    /// 能得到真正的圆角，代价是边缘只有 1 bit，没有抗锯齿。
    ///
    /// `radius` 是逻辑像素，必须与 `--radius-shell` 保持一致。
    pub fn set_round_window_region(
        window: &WebviewWindow,
        width: u32,
        height: u32,
        scale: f64,
        radius: f64,
    ) -> Result<(), String> {
        let hwnd = window
            .hwnd()
            .map_err(|e| AppError::ReadWindowHandleFailed {
                detail: e.to_string(),
            })?;

        // SetWindowRgn 的单位是物理像素；CreateRoundRectRgn 的后两个参数是
        // 椭圆直径，所以传 2r。+1 是为了不让最右/最下一列像素被排除在外。
        let r = (radius * scale).round().max(1.0) as i32;
        unsafe {
            let region = CreateRoundRectRgn(
                0,
                0,
                width as i32 + 1,
                height as i32 + 1,
                r * 2,
                r * 2,
            );
            if region.is_invalid() {
                return Err(String::from(AppError::RoundRegionCreateFailed));
            }
            // 成功时区域归系统所有，不能再自己释放；失败时才需要释放。
            if SetWindowRgn(hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
                return Err(String::from(AppError::RoundRegionApplyFailed));
            }
        }
        Ok(())
    }

    /// 当前光标位置（物理像素，虚拟桌面坐标系）。
    pub fn cursor_pos() -> Option<(i32, i32)> {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }.ok()?;
        Some((point.x, point.y))
    }

    /// 任意鼠标键是否处于按下状态。
    pub fn mouse_button_down() -> bool {
        const VK_LBUTTON: i32 = 0x01;
        const VK_RBUTTON: i32 = 0x02;
        const VK_MBUTTON: i32 = 0x04;
        const VK_XBUTTON1: i32 = 0x05;
        const VK_XBUTTON2: i32 = 0x06;

        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
            .into_iter()
            .any(|vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0)
    }

    /// 系统范围内最后一次真实输入的 tick。键盘、鼠标移动、鼠标键都会刷新它，
    /// 所以一次调用就能覆盖「用户动了」，不必去猜具体是哪个键。
    pub fn last_input_tick() -> Option<u32> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        unsafe { GetLastInputInfo(&mut info) }
            .as_bool()
            .then_some(info.dwTime)
    }

    /// Windows 传进来的 HWND 属于不可信输入，必须验证。
    pub fn is_valid_window(hwnd: isize) -> bool {
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        unsafe { IsWindow(Some(hwnd)) }.as_bool()
    }

    pub fn read_desktop_string(name: &str) -> Option<String> {
        read_hkcu_string(DESKTOP_KEY, name)
    }

    /// 读 `HKCU` 下一个字符串值。安装器写在卸载键里的 `InstallLocation` 也走这里。
    pub fn read_hkcu_string(subkey: &str, name: &str) -> Option<String> {
        unsafe {
            let mut key = HKEY::default();
            let opened = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(subkey),
                Some(0),
                KEY_QUERY_VALUE,
                &mut key,
            );
            if opened.is_err() {
                return None;
            }

            let name = HSTRING::from(name);
            let mut kind = REG_VALUE_TYPE::default();
            let mut len: u32 = 0;
            let probe = RegQueryValueExW(key, &name, None, Some(&mut kind), None, Some(&mut len));
            if probe.is_err() || len == 0 {
                let _ = RegCloseKey(key);
                return None;
            }

            let mut buffer = vec![0u8; len as usize];
            let read = RegQueryValueExW(
                key,
                &name,
                None,
                Some(&mut kind),
                Some(buffer.as_mut_ptr()),
                Some(&mut len),
            );
            let _ = RegCloseKey(key);
            if read.is_err() {
                return None;
            }

            let units: Vec<u16> = buffer
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .take_while(|unit| *unit != 0)
                .collect();
            Some(String::from_utf16_lossy(&units))
        }
    }

    pub fn write_desktop_string(name: &str, value: &str) -> Result<(), String> {
        unsafe {
            let mut key = HKEY::default();
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(DESKTOP_KEY),
                Some(0),
                KEY_SET_VALUE,
                &mut key,
            )
            .ok()
            .map_err(|e| AppError::OpenRegistryFailed {
                detail: e.to_string(),
            })?;

            let name = HSTRING::from(name);
            let mut data: Vec<u16> = value.encode_utf16().collect();
            data.push(0);
            let bytes: Vec<u8> = data.iter().flat_map(|u| u.to_le_bytes()).collect();

            let status = RegSetValueExW(key, &name, None, REG_SZ, Some(&bytes));
            let _ = RegCloseKey(key);
            status.ok().map_err(|e| {
                String::from(AppError::WriteRegistryFailed {
                    detail: e.to_string(),
                })
            })
        }
    }

    /// 删除当前用户桌面设置里的某个值。用于没有回滚点时清掉本程序的屏保指向。
    pub fn delete_desktop_value(name: &str) -> Result<(), String> {
        unsafe {
            let mut key = HKEY::default();
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(DESKTOP_KEY),
                Some(0),
                KEY_SET_VALUE,
                &mut key,
            )
            .ok()
            .map_err(|e| AppError::OpenRegistryFailed {
                detail: e.to_string(),
            })?;

            let status = RegDeleteValueW(key, &HSTRING::from(name));
            let _ = RegCloseKey(key);
            status.ok().map_err(|e| {
                String::from(AppError::DeleteRegistryValueFailed {
                    detail: e.to_string(),
                })
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::WebviewWindow;

    use crate::errors::AppError;

    pub fn set_round_window_region(
        _window: &WebviewWindow,
        _width: u32,
        _height: u32,
        _scale: f64,
        _radius: f64,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn cursor_pos() -> Option<(i32, i32)> {
        None
    }
    pub fn mouse_button_down() -> bool {
        false
    }
    pub fn last_input_tick() -> Option<u32> {
        None
    }
    pub fn is_valid_window(_hwnd: isize) -> bool {
        false
    }
    pub fn read_desktop_string(_name: &str) -> Option<String> {
        None
    }
    pub fn read_hkcu_string(_subkey: &str, _name: &str) -> Option<String> {
        None
    }
    pub fn write_desktop_string(_name: &str, _value: &str) -> Result<(), String> {
        Err(String::from(AppError::SaverRegistrationUnsupported))
    }
    pub fn delete_desktop_value(_name: &str) -> Result<(), String> {
        Err(String::from(AppError::SaverRegistrationUnsupported))
    }
}

pub use imp::*;
