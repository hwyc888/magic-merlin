#!/bin/sh

source /koolshare/scripts/base.sh

module="magictier"
DIR="$(cd "$(dirname "$0")" && pwd)"
TITLE="MagicTier"
DESCR="MagicTier ARM64 mesh networking"
PLVER="$(cat "${DIR}/version" 2>/dev/null || echo 1.0.0)"

alias echo_date='echo 〖$(TZ=UTC-8 date -R +%Y年%m月%d日\ %X)〗:'

get_model() {
    MODEL="$(nvram get productid 2>/dev/null)"
    [ -n "${MODEL}" ] || MODEL="$(nvram get odmpid 2>/dev/null)"
}

platform_test() {
    ARCH="$(uname -m 2>/dev/null)"
    case "${ARCH}" in
        aarch64|arm64) ;;
        *)
            echo_date "不支持的CPU架构：${ARCH}，本插件仅支持ARM64。"
            exit 1
            ;;
    esac

    case "${MODEL}" in
        RT-AX86U|TUF-BE3600_V2|TUF-BE3600-V2|TUF_3600_V2) ;;
        *)
            echo_date "警告：当前机型 ${MODEL} 未列入本发布包的验证机型。"
            ;;
    esac

    [ -d /koolshare ] || {
        echo_date "未检测到 /koolshare，无法作为KoolCenter插件安装。"
        exit 1
    }
}

install_now() {
    ENABLE="$(dbus get magictier_enable 2>/dev/null)"
    if [ "${ENABLE}" = "1" ] && [ -x /koolshare/scripts/magictier_config.sh ]; then
        sh /koolshare/scripts/magictier_config.sh stop >/dev/null 2>&1
    fi

    echo_date "安装MagicTier插件文件..."
    mkdir -p /koolshare/bin /koolshare/scripts /koolshare/webs /koolshare/res /koolshare/init.d /koolshare/magictier

    cp -f "${DIR}/bin/magictier-core" /koolshare/bin/magictier-core
    cp -f "${DIR}/bin/magictier-cli" /koolshare/bin/magictier-cli
    cp -f "${DIR}/scripts/magictier_config.sh" /koolshare/scripts/magictier_config.sh
    cp -f "${DIR}/scripts/magictier_health.sh" /koolshare/scripts/magictier_health.sh
    cp -f "${DIR}/webs/Module_magictier.asp" /koolshare/webs/Module_magictier.asp
    cp -f "${DIR}/uninstall.sh" /koolshare/scripts/uninstall_magictier.sh
    cp -f "${DIR}/version" /koolshare/magictier/version
    [ ! -f "${DIR}/res/icon-magictier.png" ] || cp -f "${DIR}/res/icon-magictier.png" /koolshare/res/icon-magictier.png

    chmod 0755 /koolshare/bin/magictier-core /koolshare/bin/magictier-cli
    chmod 0755 /koolshare/scripts/magictier_config.sh /koolshare/scripts/magictier_health.sh /koolshare/scripts/uninstall_magictier.sh
    ln -sf /koolshare/scripts/magictier_config.sh /koolshare/init.d/S97magictier.sh
    ln -sf /koolshare/scripts/magictier_config.sh /koolshare/init.d/N97magictier.sh

    [ -n "$(dbus get magictier_network_name 2>/dev/null)" ] || dbus set magictier_network_name="default"
    [ -n "$(dbus get magictier_ipv4 2>/dev/null)" ] || dbus set magictier_ipv4="10.144.144.1/24"
    [ -n "$(dbus get magictier_listeners 2>/dev/null)" ] || dbus set magictier_listeners="tcp://0.0.0.0:11010,udp://0.0.0.0:11010"
    [ -n "$(dbus get magictier_enable 2>/dev/null)" ] || dbus set magictier_enable="0"
    [ -n "$(dbus get magictier_log_max_bytes 2>/dev/null)" ] || dbus set magictier_log_max_bytes="524288"
    [ -n "$(dbus get magictier_rss_limit_kb 2>/dev/null)" ] || dbus set magictier_rss_limit_kb="262144"

    dbus set magictier_version="${PLVER}"
    dbus set softcenter_module_magictier_version="${PLVER}"
    dbus set softcenter_module_magictier_install="1"
    dbus set softcenter_module_magictier_name="magictier"
    dbus set softcenter_module_magictier_title="${TITLE}"
    dbus set softcenter_module_magictier_description="${DESCR}"

    if [ "${ENABLE}" = "1" ]; then
        dbus set magictier_enable="1"
        sh /koolshare/scripts/magictier_config.sh start >/dev/null 2>&1
    fi

    echo_date "MagicTier ${PLVER} 安装完成。"
}

get_model
platform_test
install_now
exit 0
