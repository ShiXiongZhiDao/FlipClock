//! 界面语言的唯一真相源。设计见 docs/adr/0018。
//!
//! 字典用 `as const` + `satisfies` 把 key 集合钉死：en 少一个 key 或多一个 key
//! 都会在编译期报错。Rust 侧错误码也在这里查中文/英文文案——它不认识语言偏好，
//! 前端把生效语言推给它只用于托盘菜单。

import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

import {
  errorKey,
  parseErrorPayload,
  resolveLocale,
  translate,
  type Locale,
  type LanguagePreference,
} from "./i18n-core.mjs";
import { isTauri } from "./env";

export type { Locale, LanguagePreference } from "./i18n-core.mjs";

export const zh = {
  // 导航与标题栏
  "nav.clock": "时钟预览",
  "nav.appearance": "外观主题",
  "nav.time": "时间与日期",
  "nav.sound": "声音",
  "nav.screensaver": "屏幕保护程序",
  "nav.system": "系统",
  "titlebar.restoreDefaults": "恢复默认设置",
  "titlebar.previewSaver": "预览屏保",
  "titlebar.minimize": "最小化",
  "titlebar.close": "关闭",

  // 设置外壳
  "settings.saveFailed": "保存失败：{error}",
  "settings.restoreConfirm": "恢复默认设置？当前的外观、时间与声音设置会被清除。",
  "settings.restoreSuccess": "已恢复默认设置",

  // 预览卡
  "preview.live": "实时效果",
  "preview.quickSettings": "快速设置",

  // 时间与日期
  "time.format": "时间制式",
  "time.currentStyle": "当前样式",
  "time.format24": "24 小时",
  "time.format12": "12 小时",
  "time.showDate": "显示日期与星期",
  "time.showSeconds": "显示秒数",
  "time.showColon": "显示时间冒号",
  "time.digitLayout": "翻页卡排布",
  "time.layoutPerDigit": "一位一卡",
  "time.layoutPerPair": "两位一卡",

  // 时钟外观
  "clock.scale": "时钟缩放",
  "clock.scaleAria": "时钟缩放比例",
  "appearance.inUse": "✓ 使用中",
  "appearance.digitLook": "数字外观",
  "appearance.brightness": "亮度",
  "appearance.brightnessAria": "时钟亮度",
  "appearance.uiGlass": "界面玻璃",
  "appearance.transparencySuffix": "透（{percent}% 透）",
  "appearance.glassAria": "界面玻璃透明度",
  "appearance.showTheme": "显示主题",
  "appearance.theme.classicDark": "经典碳灰",
  "appearance.theme.oledBlack": "OLED 纯黑",
  "appearance.theme.retroLight": "复古暖白",
  "appearance.font.bebas": "Bebas Neue",
  "appearance.font.bahnschrift": "Bahnschrift",
  "appearance.font.systemDefault": "系统默认",

  // 声音
  "sound.title": "翻页音效",
  "sound.enable": "启用音效",
  "sound.volume": "音量",
  "sound.volumeAria": "翻页音效音量",
  "sound.audition": "试听",
  "sound.stageOnly": "仅在全屏时钟与屏保中发声",
  "sound.unsupported": "此环境无法播放音效",
  "sound.muted": "音量为 0，先把音量调高",

  // 屏保
  "saver.status.notInstalled": "未安装",
  "saver.status.installed": "已安装",
  "saver.status.missing": "文件缺失",
  "saver.status.outdated": "版本不匹配",
  "saver.installToast": "屏保组件已就位，并已设为当前屏保",
  "saver.restoreToast": "已恢复安装前的屏保设置",
  "saver.windowsTitle": "Windows 屏幕保护程序",
  "saver.install": "安装并设为当前屏保",
  "saver.redeploy": "重新部署屏保组件",
  "saver.openSettings": "打开 Windows 屏保设置",
  "saver.restoreOriginal": "恢复原屏保",
  "saver.currentTarget": "系统当前指向",
  "saver.notSet": "（未设置，即屏保为「无」）",
  "saver.componentVersion": "组件版本",
  "saver.versionMatch": "与当前程序一致",
  "saver.versionMismatch": "与当前程序不一致，建议重新部署",
  "saver.rollbackPoint": "回滚点",
  "saver.rollbackRecorded": "已记录安装前的设置",
  "saver.rollbackNone": "无（安装前系统本就没有设置）",
  "saver.monitorTitle": "多显示器",
  "saver.monitorPrimary": "主显示器",
  "saver.monitorAll": "所有显示器",
  "saver.launchSettings": "打开设置窗口",
  "saver.launchFullscreen": "直接全屏",

  // 系统页
  "common.startup": "启动",
  "system.autostartOn": "已加入开机启动",
  "system.autostartOff": "已从开机启动移除",
  "system.autostart": "开机自启",
  "system.autostartHint": "登录 Windows 后驻留托盘，不弹窗口",
  "system.closeToTray": "关闭窗口时最小化到托盘",
  "system.closeToTrayHint": "关掉后点关闭按钮就退出整个程序",
  "system.saverUpdatedToast": "屏保组件已更新为当前版本",
  "system.about": "关于",
  "system.version": "版本",
  "system.saverComponent": "屏保组件",
  "system.reinstall": "重新安装",
  "system.aboutAuthor": "关于作者",
  "system.aboutText":
    "FlipClock 由「师兄知道」制作。个人主页、源码与反馈入口都在下面，公众号与收款码从公开图片仓库实时加载，不随安装包分发。",
  "system.officialWebsite": "官网",
  "system.openSource": "开源项目",
  "system.feedback": "问题反馈",
  "system.followWechat": "关注公众号",
  "system.wechatQrAlt": "微信公众号「师兄知道」二维码",
  "system.wechatQrFail": "图片源暂时连不上，可在微信里直接搜「师兄知道」。",
  "system.sponsor": "赞助作者",
  "system.sponsorText": "喜欢的话可以请作者喝杯咖啡，不影响功能。",
  "system.alipayQrAlt": "支付宝收款码",
  "system.alipayQrFail": "支付宝收款码暂时加载不出来。",
  "system.wechatPayQrAlt": "微信支付收款码",
  "system.wechatPayQrFail": "微信收款码暂时加载不出来。",

  // 更新
  "system.update.title": "更新",
  "system.update.checking": "正在检查更新…",
  "system.update.upToDate": "已是最新版本",
  "system.update.available": "有新版本 v{version}",
  "system.update.downloading": "正在下载 {percent}%",
  "system.update.installing": "正在安装，应用会退出并重新启动",
  "system.update.failed": "检查更新失败",
  "system.update.portableHint": "当前是便携版：自动安装只能替换安装版，请到 Release 页下载新包。",
  "system.update.downloadHint": "下载完成后应用会短暂退出，由安装器替换文件。",
  "system.update.installerHint": "安装器接管后会自动把 FlipClock 拉起来。",
  "system.update.portableIdle": "便携版可以检查，但不能就地安装。",
  "system.update.retry": "重试",
  "system.update.check": "检查更新",
  "system.update.downloadInstall": "下载并安装",

  // 界面语言
  "language.title": "界面语言",
  "language.system": "跟随系统",
  "language.zh": "简体中文",
  "language.en": "English",
  "language.hint": "跟随系统在启动时按系统语言选择；运行中改了系统语言需重启或手动切换。",

  // Rust 错误码。参数用 {detail} / {key} / {label} 插值。
  "error.configUnavailable": "配置状态不可用",
  "error.unknownAboutLink": "未知的外链标识：{key}",
  "error.openBrowserFailed": "打开浏览器失败：{detail}",
  "error.noLocalAppData": "无法定位 LOCALAPPDATA",
  "error.currentExeFailed": "无法定位当前程序：{detail}",
  "error.copySaverFailed": "写入屏保组件失败：{detail}",
  "error.replaceSaverFailed": "替换屏保组件失败：{detail}",
  "error.createInstallDirFailed": "创建安装目录失败：{detail}",
  "error.openWindowsSettingsFailed": "打开系统屏保设置失败：{detail}",
  "error.configDirUnavailable": "无法定位配置目录",
  "error.createConfigDirFailed": "创建配置目录失败：{detail}",
  "error.serializeConfigFailed": "序列化配置失败：{detail}",
  "error.writeConfigFailed": "写入临时配置失败：{detail}",
  "error.replaceConfigFailed": "替换配置失败：{detail}",
  "error.openRegistryFailed": "打开注册表键失败：{detail}",
  "error.readPrimaryMonitorFailed": "读取主显示器失败：{detail}",
  "error.enumerateMonitorsFailed": "枚举显示器失败：{detail}",
  "error.noMonitor": "没有可用的显示器",
  "error.createWindowFailed": "创建窗口 {label} 失败：{detail}",
  "error.locateWindowFailed": "定位窗口 {label} 失败：{detail}",
  "error.resizeWindowFailed": "调整窗口 {label} 尺寸失败：{detail}",
  "error.showWindowFailed": "显示窗口 {label} 失败：{detail}",
  "error.openSettingsWindowFailed": "打开设置窗口失败：{detail}",
  "error.readWindowScaleFailed": "读取窗口缩放失败：{detail}",
  "error.readWindowSizeFailed": "读取窗口尺寸失败：{detail}",
  "error.invalidPreviewParent": "Windows 传入的预览父窗口无效",
  "error.systemPreviewUnsupported": "系统预览暂未支持：内置 WebView 无法稳定嵌入系统预览框",
  "error.readWindowHandleFailed": "读取窗口句柄失败：{detail}",
  "error.roundRegionCreateFailed": "创建窗口圆角区域失败",
  "error.roundRegionApplyFailed": "应用窗口圆角区域失败",
  "error.writeRegistryFailed": "写入注册表失败：{detail}",
  "error.deleteRegistryValueFailed": "删除注册表值失败：{detail}",
  "error.saverRegistrationUnsupported": "当前平台不支持屏保注册",
  "error.autostartKeyFailed": "打开注册表键失败（{key}）：{detail}",
  "error.autostartExeFailed": "取程序路径失败：{detail}",
  "error.autostartWriteFailed": "写入启动项失败（{key}）：{detail}",
  "error.setLocaleFailed": "切换托盘语言失败：{detail}",
} as const;

export type MessageKey = keyof typeof zh;

/** key 集合由 zh 决定；en 少一个、多一个或值不是 string 都编译不过。 */
export const en: Record<MessageKey, string> = {
  "nav.clock": "Clock Preview",
  "nav.appearance": "Appearance",
  "nav.time": "Time & Date",
  "nav.sound": "Sound",
  "nav.screensaver": "Screen Saver",
  "nav.system": "System",
  "titlebar.restoreDefaults": "Restore Default Settings",
  "titlebar.previewSaver": "Preview Screen Saver",
  "titlebar.minimize": "Minimize",
  "titlebar.close": "Close",
  "settings.saveFailed": "Failed to save: {error}",
  "settings.restoreConfirm":
    "Restore default settings? Your current appearance, time, and sound settings will be cleared.",
  "settings.restoreSuccess": "Defaults restored",
  "preview.live": "Live Preview",
  "preview.quickSettings": "Quick Settings",
  "time.format": "Time Format",
  "time.currentStyle": "Current Style",
  "time.format24": "24-Hour",
  "time.format12": "12-Hour",
  "time.showDate": "Show Date & Weekday",
  "time.showSeconds": "Show Seconds",
  "time.showColon": "Show Separator Dots",
  "time.digitLayout": "Flip Card Layout",
  "time.layoutPerDigit": "One Digit per Card",
  "time.layoutPerPair": "Two Digits per Card",
  "clock.scale": "Clock Scale",
  "clock.scaleAria": "Clock Scale Ratio",
  "appearance.inUse": "✓ In Use",
  "appearance.digitLook": "Digit Appearance",
  "appearance.brightness": "Brightness",
  "appearance.brightnessAria": "Clock Brightness",
  "appearance.uiGlass": "UI Glass",
  "appearance.transparencySuffix": "transparent ({percent}%)",
  "appearance.glassAria": "UI Glass Transparency",
  "appearance.showTheme": "Show Theme",
  "appearance.theme.classicDark": "Classic Carbon Gray",
  "appearance.theme.oledBlack": "OLED True Black",
  "appearance.theme.retroLight": "Retro Warm White",
  "appearance.font.bebas": "Bebas Neue",
  "appearance.font.bahnschrift": "Bahnschrift",
  "appearance.font.systemDefault": "System Default",
  "sound.title": "Flip Sound",
  "sound.enable": "Enable Sound",
  "sound.volume": "Volume",
  "sound.volumeAria": "Flip Sound Volume",
  "sound.audition": "Test",
  "sound.stageOnly": "Sound plays only in the full-screen clock and screen saver.",
  "sound.unsupported": "Audio is unavailable in this environment.",
  "sound.muted": "Volume is 0; turn it up first.",
  "saver.status.notInstalled": "Not Installed",
  "saver.status.installed": "Installed",
  "saver.status.missing": "File Missing",
  "saver.status.outdated": "Version Mismatch",
  "saver.installToast": "Screen saver component is in place and set as the current screen saver",
  "saver.restoreToast": "Pre-install screen saver settings restored",
  "saver.windowsTitle": "Windows Screen Saver",
  "saver.install": "Install and Set as Current Screen Saver",
  "saver.redeploy": "Redeploy Screen Saver Component",
  "saver.openSettings": "Open Windows Screen Saver Settings",
  "saver.restoreOriginal": "Restore Original Screen Saver",
  "saver.currentTarget": "Current System Selection",
  "saver.notSet": '(Not set — screen saver is "None")',
  "saver.componentVersion": "Component Version",
  "saver.versionMatch": "Matches the current app",
  "saver.versionMismatch": "Doesn't match the current app; redeployment recommended",
  "saver.rollbackPoint": "Rollback Point",
  "saver.rollbackRecorded": "Pre-install settings recorded",
  "saver.rollbackNone": "None (no screen saver was set before installation)",
  "saver.monitorTitle": "Multiple Monitors",
  "saver.monitorPrimary": "Primary Monitor",
  "saver.monitorAll": "All Monitors",
  "saver.launchSettings": "Open Settings window",
  "saver.launchFullscreen": "Go Full Screen",
  "common.startup": "Startup",
  "system.autostartOn": "Added to autostart",
  "system.autostartOff": "Removed from autostart",
  "system.autostart": "Autostart",
  "system.autostartHint": "Stay in the system tray after signing in to Windows, without a pop-up window",
  "system.closeToTray": "Minimize to system tray when closing the window",
  "system.closeToTrayHint": "When off, clicking Close exits the app entirely",
  "system.saverUpdatedToast": "Screen saver component updated to the current version",
  "system.about": "About",
  "system.version": "Version",
  "system.saverComponent": "Screen Saver Component",
  "system.reinstall": "Reinstall",
  "system.aboutAuthor": "About the Author",
  "system.aboutText":
    "FlipClock is made by ShiXiongZhiDao. Personal site, source code, and feedback links are below. The WeChat Official Account QR code and donation QR codes load live from a public image repository and aren't bundled with the installer.",
  "system.officialWebsite": "Official Website",
  "system.openSource": "Open-Source Projects",
  "system.feedback": "Feedback",
  "system.followWechat": "Follow WeChat Official Account",
  "system.wechatQrAlt": 'WeChat Official Account "ShiXiongZhiDao" QR code',
  "system.wechatQrFail": 'Image source is temporarily unreachable; search for "ShiXiongZhiDao" in WeChat instead.',
  "system.sponsor": "Support the Author",
  "system.sponsorText": "If you like it, buy the author a coffee — it doesn't affect any features.",
  "system.alipayQrAlt": "Alipay payment QR code",
  "system.alipayQrFail": "The Alipay payment QR code failed to load.",
  "system.wechatPayQrAlt": "WeChat Pay payment QR code",
  "system.wechatPayQrFail": "The WeChat Pay QR code failed to load.",
  "system.update.title": "Update",
  "system.update.checking": "Checking for updates…",
  "system.update.upToDate": "You're up to date",
  "system.update.available": "New version v{version} available",
  "system.update.downloading": "Downloading {percent}%",
  "system.update.installing": "Installing — the app will exit and restart",
  "system.update.failed": "Update check failed",
  "system.update.portableHint":
    "This is a portable build: automatic installation only replaces installed builds. Download the new package from the Releases page.",
  "system.update.downloadHint": "After downloading, the app exits briefly while the installer replaces the files.",
  "system.update.installerHint": "The installer will relaunch FlipClock automatically.",
  "system.update.portableIdle": "Portable builds can check for updates but can't install in place.",
  "system.update.retry": "Retry",
  "system.update.check": "Check for Updates",
  "system.update.downloadInstall": "Download and Install",
  "language.title": "Interface Language",
  "language.system": "Follow System",
  "language.zh": "简体中文",
  "language.en": "English",
  "language.hint": "Follow System picks a language at startup. Changing the system language later needs a restart or a manual switch.",
  "error.configUnavailable": "Settings state unavailable",
  "error.unknownAboutLink": "Unknown link ID: {key}",
  "error.openBrowserFailed": "Failed to open browser: {detail}",
  "error.noLocalAppData": "Cannot locate LOCALAPPDATA",
  "error.currentExeFailed": "Cannot locate the current program: {detail}",
  "error.copySaverFailed": "Failed to write the screen saver component: {detail}",
  "error.replaceSaverFailed": "Failed to replace the screen saver component: {detail}",
  "error.createInstallDirFailed": "Failed to create the install directory: {detail}",
  "error.openWindowsSettingsFailed": "Failed to open Windows screen saver settings: {detail}",
  "error.configDirUnavailable": "Cannot locate the config directory",
  "error.createConfigDirFailed": "Failed to create the config directory: {detail}",
  "error.serializeConfigFailed": "Failed to serialize settings: {detail}",
  "error.writeConfigFailed": "Failed to write the temporary config file: {detail}",
  "error.replaceConfigFailed": "Failed to replace the config file: {detail}",
  "error.openRegistryFailed": "Failed to open the registry key: {detail}",
  "error.readPrimaryMonitorFailed": "Failed to read the primary monitor: {detail}",
  "error.enumerateMonitorsFailed": "Failed to enumerate monitors: {detail}",
  "error.noMonitor": "No monitor is available",
  "error.createWindowFailed": "Failed to create window {label}: {detail}",
  "error.locateWindowFailed": "Failed to locate window {label}: {detail}",
  "error.resizeWindowFailed": "Failed to resize window {label}: {detail}",
  "error.showWindowFailed": "Failed to show window {label}: {detail}",
  "error.openSettingsWindowFailed": "Failed to open the Settings window: {detail}",
  "error.readWindowScaleFailed": "Failed to read the window scale: {detail}",
  "error.readWindowSizeFailed": "Failed to read the window size: {detail}",
  "error.invalidPreviewParent": "The preview parent window passed by Windows is invalid",
  "error.systemPreviewUnsupported":
    "System preview is not supported yet: the built-in WebView cannot be embedded reliably in the system preview frame",
  "error.readWindowHandleFailed": "Failed to read the window handle: {detail}",
  "error.roundRegionCreateFailed": "Failed to create the rounded window region",
  "error.roundRegionApplyFailed": "Failed to apply the rounded window region",
  "error.writeRegistryFailed": "Failed to write to the registry: {detail}",
  "error.deleteRegistryValueFailed": "Failed to delete the registry value: {detail}",
  "error.saverRegistrationUnsupported": "Screen saver registration is not supported on this platform",
  "error.autostartKeyFailed": "Failed to open the registry key ({key}): {detail}",
  "error.autostartExeFailed": "Failed to get the program path: {detail}",
  "error.autostartWriteFailed": "Failed to write the autostart entry ({key}): {detail}",
  "error.setLocaleFailed": "Failed to switch the tray language: {detail}",
};

/** 全局生效语言。只有 zh/en 两个值，Rust 侧不需要理解 system 态。 */
export const locale = ref<Locale>("zh");
export const preference = ref<LanguagePreference>("system");

export function t(key: MessageKey, params?: Record<string, string | number>): string {
  return translate({ zh, en }, locale.value, key, params);
}

/**
 * 按启动时算出的生效语言初始化。偏好为 system 时只算一次，运行期不随系统语言重算。
 * 浏览器开发模式下没有 set_locale 命令，跳过托盘推送。
 */
export function applyLanguage(pref: LanguagePreference, system = navigator.language): void {
  preference.value = pref;
  locale.value = resolveLocale(pref, system);
  document.documentElement.lang = locale.value === "zh" ? "zh-CN" : "en";
  if (isTauri()) {
    void invoke("set_locale", { locale: locale.value }).catch((error) => {
      // 托盘语言是次要路径：失败要让开发日志看得见，但不打断界面切换。
      console.warn(t("error.setLocaleFailed", { detail: String(error) }));
    });
  }
}

/**
 * Rust 命令返回的错误串是 JSON：`{"code":"copySaverFailed","detail":"..."}`。
 * 认不出的错误原样返回——第三方（如 updater）的英文错误不该被吞掉。
 */
export function errorText(raw: unknown): string {
  const text = String(raw);
  const payload = parseErrorPayload(text);
  if (!payload) return text;
  // Rust 码是裸名，字典 key 带 error. 前缀。拼接逻辑在 i18n-core.mjs，
  // 由 check-i18n-runtime.mjs 直接验证——这里漏前缀曾经让用户看到裸 JSON。
  const key = errorKey(payload.code) as MessageKey;
  if (!Object.prototype.hasOwnProperty.call(zh, key)) return text;
  return t(key, payload.params);
}
