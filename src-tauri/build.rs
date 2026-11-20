fn main() {
    // 改 key 必须触发重编：这个文件只在构建期读一次，缓存住就等于改了不生效。
    println!("cargo:rerun-if-changed=.env");
    // Aptabase App Key（ADR-0019）：CI 用环境变量注入，本地从同目录的 .env 读。
    // .env 不入库、也被 PUBLIC-MANIFEST 的 denyPaths（含 `**/.env`）拦着，
    // 不会流到公开仓库。
    // 两者都没有时不注入：lib.rs 的 option_env! 得到 None，插件静默禁用，
    // 任何人 clone 下来都能编译，本地开发默认不发数据。
    if std::env::var_os("FLIPCLOCK_APTABASE_KEY").is_none() {
        if let Ok(contents) = std::fs::read_to_string(".env") {
            for line in contents.lines() {
                if let Some((key, value)) = line.trim().split_once('=') {
                    if key.trim() == "FLIPCLOCK_APTABASE_KEY" {
                        let value = value.trim().trim_matches(['"', '\'']);
                        if !value.is_empty() {
                            println!("cargo:rustc-env=FLIPCLOCK_APTABASE_KEY={value}");
                        }
                        break;
                    }
                }
            }
        }
    }
    tauri_build::build()
}
