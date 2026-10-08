//! Xiaomi.eu ToolKit Zygisk —— 只做一件事：
//!
//! 让**被选中的 App** 看到 `Build.BRAND = "Xiaomi"` / `ro.product.brand = "Xiaomi"`。
//!
//! 改这一个字段即可修复（已在 LSPosed 上验证）。
//!
//! 为什么必须两层：
//!  * `android.os.Build` 是 zygote 预加载的静态字段，App 用 `sget-object` 直读，
//!    运行期改系统属性**永远不会**影响它 —— 所以必须用 JNI 直接写字段。
//!  * 同时替换 `SystemProperties` 的 native getter，兜住那些直接读属性的代码。
//!
//! 为什么需要 companion：
//!  * 目标 App 的进程是 app uid，而 `/data/adb` 是 `root:root 0700`，读不到配置文件；
//!  * Zygisk 会以 root 另起一个 companion 进程，由它读配置并回答「这个包要不要伪装」。
//!
//! 未启用的包会立刻 `DlCloseModuleLibrary`，模块不驻留内存，做到零残留。
//!
//! 骨架取自 MiPushFramework 的 MiPushZygisk（GPL-3.0），此处按需精简。

use android_logger::Config;
use jni::{Env, EnvUnowned};
use log::{debug, error, info, LevelFilter};
use std::os::unix::net::UnixStream;
use zygisk_api::{
    api::{v4::ZygiskOption, ZygiskApi, V4},
    raw::ZygiskRaw,
    register_companion, register_module, ZygiskModule,
};

mod config;
mod hook;
mod protocol;
mod server;

const TAG: &str = "XiaomiEuToolKitZygisk";

/// 唯一要伪装的品牌值。
const BRAND_VALUE: &str = "Xiaomi";

/// `android.os.Build` 的静态字段（JNI 的 SetStaticObjectField 能写 `static final`）。
static BUILD_PROPERTIES: &[(&str, &str)] = &[("BRAND", BRAND_VALUE)];

/// `SystemProperties` 的替换表，给直接读属性的代码兜底。
static SYSTEM_PROPERTIES: &[(&str, &str)] = &[("ro.product.brand", BRAND_VALUE)];

#[derive(Default)]
struct XiaomiEuToolKitZygiskModule;

impl ZygiskModule for XiaomiEuToolKitZygiskModule {
    type Api = V4;

    fn pre_app_specialize<'a>(
        &self,
        mut api: ZygiskApi<'a, V4>,
        mut env: EnvUnowned<'a>,
        args: &'a mut <V4 as ZygiskRaw<'_>>::AppSpecializeArgs,
    ) {
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag(TAG),
        );

        let raw_env = env.as_raw();
        let process_name = jstring_to_string(&mut env, args.nice_name);
        let app_data_dir = jstring_to_string(&mut env, args.app_data_dir);
        if process_name.is_empty() || app_data_dir.is_empty() {
            api.set_option(ZygiskOption::DlCloseModuleLibrary);
            return;
        }

        let package_name = parse_package_name(&app_data_dir);
        if package_name.is_empty() {
            api.set_option(ZygiskOption::DlCloseModuleLibrary);
            return;
        }
        debug!("pre_app_specialize pkg={package_name} process={process_name}");

        if !query_enabled(&mut api, package_name, &process_name) {
            debug!("pkg={package_name} process={process_name}: not enabled, skipping");
            api.set_option(ZygiskOption::DlCloseModuleLibrary);
            return;
        }

        unsafe { EnvUnowned::from_raw(raw_env) }
            .with_env_no_catch(|env| {
                pre_specialize(api, env, package_name);
                Ok::<_, jni::errors::Error>(())
            })
            .into_outcome();
    }

    fn pre_server_specialize<'a>(
        &self,
        mut api: ZygiskApi<'a, V4>,
        _env: EnvUnowned<'a>,
        _args: &'a mut <V4 as ZygiskRaw<'_>>::ServerSpecializeArgs,
    ) {
        // system_server 不需要伪装机型。
        api.set_option(ZygiskOption::DlCloseModuleLibrary);
    }
}

fn pre_specialize(mut api: ZygiskApi<'_, V4>, env: &mut Env<'_>, package_name: &str) {
    info!("pkg={package_name}: spoofing BRAND -> {BRAND_VALUE}");

    // ① 写 Build 静态字段（治本：App 读的就是它）；第三个参数是 Build.VERSION 的表，我们不用
    hook::hook_build(env, BUILD_PROPERTIES, &[]);

    // ② 替换 SystemProperties 的 native getter（兜底：给直接读属性的代码）
    let unowned = unsafe { EnvUnowned::from_raw(env.get_raw()) };
    hook::hook_system_properties(&mut api, unowned, SYSTEM_PROPERTIES);
}

fn jstring_to_string(env: &mut EnvUnowned<'_>, jstr: &jni::objects::JString<'_>) -> String {
    env.with_env_no_catch(|env| Ok::<_, jni::errors::Error>(jstr.mutf8_chars(env)?.to_string()))
        .resolve::<jni::errors::LogErrorAndDefault>()
}

/// 从 App 的 data 目录反推包名，例如 `/data/user/0/com.taobao.taobao` → `com.taobao.taobao`。
fn parse_package_name(app_data_dir: &str) -> &str {
    app_data_dir
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("")
}

fn query_enabled(api: &mut ZygiskApi<'_, V4>, package_name: &str, process_name: &str) -> bool {
    match api.with_companion(|stream| send_query(stream, package_name, process_name)) {
        Ok(enabled) => enabled,
        Err(err) => {
            // 读不到 companion（例如模块刚装还没重启）→ 一律不伪装，避免意外生效。
            error!("companion unavailable: {err:?}");
            false
        }
    }
}

fn send_query(stream: &mut UnixStream, package_name: &str, process_name: &str) -> bool {
    if let Err(err) = protocol::configure(stream).and_then(|_| {
        protocol::write_frame(
            stream,
            protocol::QUERY,
            &protocol::encode_query(package_name, process_name),
        )
    }) {
        error!("send companion query failed: {err}");
        return false;
    }

    match protocol::read_frame(stream).and_then(|(kind, payload)| {
        if kind != protocol::RESPONSE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response",
            ));
        }
        protocol::decode_response(&payload)
    }) {
        Ok(enabled) => enabled,
        Err(err) => {
            error!("read companion response failed: {err}");
            false
        }
    }
}

register_module!(XiaomiEuToolKitZygiskModule);
register_companion!(server::companion_handler);
