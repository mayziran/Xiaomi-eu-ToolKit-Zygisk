# Xiaomi.eu ToolKit Zygisk

配合 [Xiaomi-eu-ToolKit](https://github.com/mayziran/Xiaomi-eu-ToolKit) 使用的一个 Zygisk 模块，
让指定的 App 看到 `Build.BRAND = "Xiaomi"`。

> ⚠️ **如果推送注册没出问题，就不要装这个模块。**
> 大概率**国际版不需要、EU 版需要**，而且**各机型实际情况不同**。
> 先用 [Xiaomi-eu-ToolKit](https://github.com/mayziran/Xiaomi-eu-ToolKit) 看看那个 App
> 是不是真的显示「未注册」，确认了再装。

---

## 说明

精简自 [MiPushZygisk](https://gitlab.com/magisk3171/MiPushZygisk)，只保留了针对 **xiaomi.eu** 的那一条特殊适配。

- 原项目面向**非 MIUI** 设备，需要伪装整套机型 / 区域 / 其它厂商特征；本模块面向 **HyperOS / MIUI 系 ROM**，
  只针对 `brand` 这一项做适配，其它伪装一律不要。
- 以后如果遇到某些 App 卡在别的信号上（比如 `MODEL`、分区属性），再按 App 逐个扩大伪装范围。

只改两条，而且**只在被选中的 App 进程内、只在内存里**：`android.os.Build.BRAND` 与
`SystemProperties` 的 `ro.product.brand`。不动磁盘上任何文件，不影响未选中的 App；
系统设置和 `getprop` 显示的 brand 也不会变。

---

## 安装

需要 Magisk / KernelSU / APatch + Zygisk（推荐 [Zygisk Next](https://github.com/Dr-TSNG/ZygiskNext)）。

从 [Releases](https://github.com/mayziran/Xiaomi-eu-ToolKit-Zygisk/releases) 下载 zip，
在 Magisk / KernelSU 里安装 → **重启设备**。

## 使用

用 root 编辑 `/data/adb/xiaomi_eu_toolkit_zygisk/apps.conf`，一行一个包名：

```bash
su
echo com.taobao.taobao > /data/adb/xiaomi_eu_toolkit_zygisk/apps.conf
am force-stop com.taobao.taobao      # 改完要重启该 App 才生效
```

配置为空、或读不到 → 一律不伪装（不会意外全局生效）。

排障：`logcat -s XiaomiEuToolKitZygisk`，命中会打印 `pkg=... spoofing BRAND -> Xiaomi`。

---

## 致谢

- [MiPushFramework](https://github.com/magisk317/MiPushFramework) 的
  [MiPushZygisk](https://gitlab.com/magisk3171/MiPushZygisk)（GPL-3.0）—— 本模块的精简来源。

## 协议

GNU GPL v3.0（见 [LICENSE](LICENSE)）。
