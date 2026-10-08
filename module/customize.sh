#!/system/bin/sh

CONFIG_DIR=/data/adb/xiaomi_eu_toolkit_zygisk
CONFIG_FILE=$CONFIG_DIR/apps.conf

ui_print "- Xiaomi.eu ToolKit Zygisk：只伪装机型品牌（Build.BRAND -> Xiaomi）"

mkdir -p "$CONFIG_DIR"
chmod 700 "$CONFIG_DIR"

if [ ! -f "$CONFIG_FILE" ]; then
  : > "$CONFIG_FILE"
  chmod 600 "$CONFIG_FILE"
fi

ui_print "- 配置文件：$CONFIG_FILE"
ui_print "- 一行一个包名，# 开头为注释"
ui_print "- 例：echo com.taobao.taobao > $CONFIG_FILE"
ui_print "- 改完要强制停止目标 App；装完本模块需要重启设备"
