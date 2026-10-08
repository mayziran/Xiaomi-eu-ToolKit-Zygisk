use std::{fs, io, os::unix::net::UnixStream, sync::Once};

use android_logger::Config;
use log::{debug, error, LevelFilter};

pub fn companion_handler(stream: &mut UnixStream) {
    static LOG_INIT: Once = Once::new();
    LOG_INIT.call_once(|| {
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag(crate::TAG),
        );
    });

    let _ = crate::protocol::configure(stream);

    if let Err(err) = handle_query(stream) {
        error!("companion query failed: {err}");
        let _ = crate::protocol::write_frame(
            stream,
            crate::protocol::RESPONSE,
            &crate::protocol::encode_response(false),
        );
    }
}

fn handle_query(stream: &mut UnixStream) -> io::Result<()> {
    let (kind, payload) = crate::protocol::read_frame(stream)?;
    if kind != crate::protocol::QUERY {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected request",
        ));
    }
    let (package_name, process_name) = crate::protocol::decode_query(&payload)?;

    let should_hook = check_config(package_name, process_name)?;
    debug!("query pkg={package_name} process={process_name} should_hook={should_hook}");

    crate::protocol::write_frame(
        stream,
        crate::protocol::RESPONSE,
        &crate::protocol::encode_response(should_hook),
    )?;

    Ok(())
}

fn check_config(package_name: &str, process_name: &str) -> io::Result<bool> {
    let content = match fs::read_to_string(crate::config::CONFIG_PATH) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    Ok(resolve_rule(&content, package_name, process_name))
}

fn resolve_rule(content: &str, package_name: &str, process_name: &str) -> bool {
    if !crate::config::is_managed_package(package_name) {
        return false;
    }
    let mut package_allow = false;
    let mut package_deny = false;
    let mut process_allow = false;
    let mut process_deny = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("profile=")
            || line.starts_with("observe=")
            || line.starts_with("auto_scan=")
        {
            continue;
        }
        let denied = line.strip_prefix('-');
        let line = denied.unwrap_or(line);
        let matched = matches_line(line, package_name, process_name);
        if !matched {
            continue;
        }
        if line.contains('|') {
            if denied.is_some() {
                process_deny = true;
            } else {
                process_allow = true;
            }
        } else {
            if denied.is_some() {
                package_deny = true;
            } else {
                package_allow = true;
            }
        }
    }
    if process_deny || package_deny {
        return false;
    }
    process_allow || package_allow
}

fn matches_line(line: &str, package_name: &str, process_name: &str) -> bool {
    if !crate::config::is_managed_package(package_name) {
        return false;
    }

    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return false;
    }

    match line.split_once('|') {
        Some((package, process)) => {
            let package = package.trim();
            let process = process.trim();
            package == package_name
                && crate::config::is_managed_package(package)
                && (process.is_empty()
                    || (process == process_name
                        && crate::config::is_valid_process_name(package, process)))
        }
        None => line == package_name && crate::config::is_managed_package(line),
    }
}

#[cfg(test)]
mod tests {
    use super::{matches_line, resolve_rule};

    #[test]
    fn package_only_matches_all_processes() {
        assert!(matches_line(
            "com.example.app",
            "com.example.app",
            "com.example.app"
        ));
        assert!(matches_line(
            "com.example.app",
            "com.example.app",
            "com.example.app:push"
        ));
    }

    #[test]
    fn process_specific_line_matches_only_that_process() {
        assert!(matches_line(
            "com.example.app|com.example.app:push",
            "com.example.app",
            "com.example.app:push",
        ));
        assert!(!matches_line(
            "com.example.app|com.example.app:push",
            "com.example.app",
            "com.example.app",
        ));
    }

    #[test]
    fn syntactically_valid_vendor_lines_match() {
        assert!(matches_line(
            "com.xiaomi.smarthome",
            "com.xiaomi.smarthome",
            "com.xiaomi.smarthome"
        ));
        assert!(matches_line(
            "com.android.settings",
            "com.android.settings",
            "com.android.settings"
        ));
    }

    #[test]
    fn deny_rule_wins_over_allow_rule() {
        assert!(!resolve_rule(
            "com.example.app\n-com.example.app|com.example.app:push",
            "com.example.app",
            "com.example.app:push"
        ));
        assert!(resolve_rule(
            "com.example.app\n-com.example.app|com.example.app:other",
            "com.example.app",
            "com.example.app:push"
        ));
    }
}
