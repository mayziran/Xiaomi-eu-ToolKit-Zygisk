//! 配置：唯一真源是 `/data/adb/xiaomi_eu_toolkit_zygisk/apps.conf`。
//!
//! 文件格式（由 App 侧或手动通过 root 写入）：
//!
//! ```text
//! # 注释
//! com.taobao.taobao              # 启用：该包的全部进程
//! com.taobao.idlefish|:push     # 只对该进程启用
//! -com.example.app              # 显式排除
//! ```
//!
//! companion 以 root 运行，所以能读这个 0600 的文件；目标 App 进程读不到（`/data/adb` 是 0700 root），
//! 这正是必须有 companion 的原因。

/// 启用的包列表。
pub const CONFIG_PATH: &str = "/data/adb/xiaomi_eu_toolkit_zygisk/apps.conf";

/// 包名合法性：至少两段、每段以字母/下划线开头、其余为字母数字下划线。
///
/// 用来把配置文件里明显不是包名的行挡掉，避免误匹配。
pub fn is_managed_package(package: &str) -> bool {
    if package.is_empty() || package.len() > 255 {
        return false;
    }
    let mut segments = 0usize;
    for segment in package.split('.') {
        segments += 1;
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    segments >= 2
}

/// 支持的进程写法：主进程名本身，或 `<包名>:<后缀>`。
pub fn is_valid_process_name(package: &str, process: &str) -> bool {
    match process.strip_prefix(package) {
        Some("") => true,
        Some(rest) => rest.starts_with(':') && rest.len() > 1,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_managed_package, is_valid_process_name};

    #[test]
    fn accepts_real_package_names() {
        assert!(is_managed_package("com.taobao.taobao"));
        assert!(is_managed_package("com.taobao.idlefish"));
        assert!(is_managed_package("com.android.settings"));
        assert!(is_managed_package("com.xiaomi.smarthome"));
        assert!(is_managed_package("a.b"));
    }

    #[test]
    fn rejects_junk() {
        assert!(!is_managed_package(""));
        assert!(!is_managed_package("taobao"));
        assert!(!is_managed_package("com..taobao"));
        assert!(!is_managed_package("com.1taobao"));
        assert!(!is_managed_package("com.taobao."));
        assert!(!is_managed_package("-com.taobao.taobao"));
    }

    #[test]
    fn process_matcher_accepts_main_and_subprocesses() {
        assert!(is_valid_process_name(
            "com.taobao.taobao",
            "com.taobao.taobao"
        ));
        assert!(is_valid_process_name(
            "com.taobao.taobao",
            "com.taobao.taobao:channel"
        ));
        assert!(!is_valid_process_name(
            "com.taobao.taobao",
            "com.taobao.taobao:"
        ));
        assert!(!is_valid_process_name(
            "com.taobao.taobao",
            "com.taobao.idlefish"
        ));
    }
}
