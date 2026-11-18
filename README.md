# FlipClock

一台机械翻页钟：Windows 桌面时钟，也是一份真正的 `.scr` 屏幕保护程序。

## 功能

- 上叶片下折、下叶片翻入的机械翻页动画；翻页声由 Web Audio 实时合成，不打包音频文件
- 三套钟面配色：经典碳灰 / OLED 纯黑 / 复古暖白
- 两种排布：一位一卡（四张竖卡），或两位一卡（两张方卡各装 `12` `41`）
- 24 / 12 小时制、日期行、秒数、时间冒号均可开关
- 时钟缩放 50%–450%、亮度 20%–100%、三款数字字体（Bebas Neue / Bahnschrift / 系统默认）
- 主显示器或全部显示器；无参数启动可进设置窗口，也可直接全屏

## 下载

- **安装版**：Releases 里的 `FlipClock_<版本>_x64-setup.exe`。按当前用户安装，不需要管理员权限；
  装完之后可以在应用内「系统 → 关于」检查并安装更新。
- **便携版**：`FlipClock_<版本>_x64-portable.zip`，解压即用。
  它能检查更新，但没有安装器可跑，新版本要手动下载。
- 国内镜像：<https://gitee.com/ShiXiongZhiDao/FlipClock/releases>（同步 Release 附件，含安装包）

更新包带 minisign 签名，更新源为 Gitee 优先、GitHub 兜底，应用内「系统 → 关于」可检查并安装。

## 构建

需要 Windows 10/11（含 WebView2）、Node.js 18+、Rust 稳定版与 pnpm。

```bash
pnpm install
pnpm build          # 前端类型检查 + 构建
pnpm tauri dev      # 开发
pnpm build:app      # release 构建，前端内嵌进二进制
pnpm scr            # 复制为 dist-scr/FlipClock.exe 与 FlipClock.scr
```

不要用裸 `cargo build --release`：缺 `custom-protocol` 特性时前端不会被内嵌，双击只会看到 localhost 拒绝连接。

要出安装包得额外配 `TAURI_SIGNING_PRIVATE_KEY`（更新包必须签名，`pnpm tauri build` 会检查）；
只想本地验屏保用 `pnpm build:app` 就够了，它不打包。

## 使用

- `FlipClock.exe`：按设置里的无参数策略启动
- `FlipClock.exe /c`：打开设置窗口
- `FlipClock.exe /s`：以屏保启动，任意输入退出
- `FlipClock.exe /p <HWND>`：暂不支持，不创建窗口直接退出

屏保组件由设置窗口里的「安装并设为当前屏保」写入当前用户，不碰 `System32`，不需要管理员权限。
卸载安装版时，安装器会先把屏保选择恢复成安装前的样子，再删文件。

## 许可

GNU GPL v3.0，见 `LICENSE`。字体与运行时依赖的归属见 `THIRD_PARTY_NOTICES.md`。
