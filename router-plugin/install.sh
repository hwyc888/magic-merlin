#!/bin/sh

source /koolshare/scripts/base.sh

module="magic"
DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="/koolshare/magic"
OWNER_MARKER="${INSTALL_DIR}/.magic-owned"
LEGACY_OWNER_MARKER="${INSTALL_DIR}/.magictier-owned"
TITLE="MagicTier Magic"
DESCR="MagicTier ARMv7/ARM64 mesh networking"
PLVER="$(cat "${DIR}/version" 2>/dev/null || echo 1.0.0)"

alias echo_date='echo 〖$(TZ=UTC-8 date -R +%Y年%m月%d日\ %X)〗:'

get_model() {
    MODEL="$(nvram get productid 2>/dev/null)"
    [ -n "${MODEL}" ] || MODEL="$(nvram get odmpid 2>/dev/null)"
}

platform_test() {
    ARCH="$(uname -m 2>/dev/null)"
    case "${ARCH}" in
        aarch64|arm64|armv7l|armv7) ;;
        *)
            echo_date "不支持的CPU架构：${ARCH}，本插件支持ARMv7/ARM64。"
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

install_dir_test() {
    if [ -e "${INSTALL_DIR}" ] || [ -L "${INSTALL_DIR}" ]; then
        if [ ! -d "${INSTALL_DIR}" ] || [ -L "${INSTALL_DIR}" ]; then
            echo_date "检测到安装路径 ${INSTALL_DIR} 已存在，但不是本插件可安全使用的目录。"
            echo_date "为避免覆盖现有数据，本次安装已取消。请先检查并处理该路径后再安装。"
            exit 1
        fi
        if [ ! -f "${OWNER_MARKER}" ] && [ ! -f "${LEGACY_OWNER_MARKER}" ]; then
            echo_date "检测到安装目录 ${INSTALL_DIR} 已存在，且不属于 MagicTier Magic。"
            echo_date "为避免与其他插件冲突，本次安装已取消。"
            exit 1
        fi
    fi
}

namespace_conflict_test() {
    if [ -f "${OWNER_MARKER}" ] || [ -f "${LEGACY_OWNER_MARKER}" ]; then
        return 0
    fi

    MAGIC_MODULE_INSTALL="$(dbus get softcenter_module_magic_install 2>/dev/null)"
    MAGIC_MODULE_NAME="$(dbus get softcenter_module_magic_name 2>/dev/null)"
    MAGIC_VERSION="$(dbus get magic_version 2>/dev/null)"
    if [ -n "${MAGIC_MODULE_INSTALL}" ] || [ -n "${MAGIC_MODULE_NAME}" ] || [ -n "${MAGIC_VERSION}" ] || \
       [ -e /koolshare/bin/magic-core ] || [ -e /koolshare/scripts/magic_config.sh ] || \
       [ -e /koolshare/scripts/magic_health.sh ] || [ -e /koolshare/scripts/uninstall_magic.sh ] || \
       [ -e /koolshare/webs/Module_magic.asp ] || [ -e /koolshare/res/icon-magic.png ] || \
       [ -e /koolshare/init.d/S97magic.sh ] || [ -e /koolshare/init.d/N97magic.sh ]; then
        echo_date "检测到 magic 插件命名空间已被占用，但没有本插件所有权标记。"
        echo_date "为避免覆盖其他插件，本次安装已取消。"
        exit 1
    fi

    if [ "$(dbus get softcenter_module_magictier_install 2>/dev/null)" = "1" ] || \
       [ -e /koolshare/bin/magictier-core-go ] || [ -d /koolshare/configs/magictier ]; then
        echo_date "检测到原 magictier 插件；MagicTier Magic 使用独立 magic 命名空间，可并存安装。"
    fi
}

install_now() {
    IS_UPGRADE=0
    IS_LEGACY_UPGRADE=0
    if [ -f "${LEGACY_OWNER_MARKER}" ] && [ ! -f "${OWNER_MARKER}" ]; then
        IS_UPGRADE=1
        IS_LEGACY_UPGRADE=1
        OLD_ENABLE="$(dbus get magictier_enable 2>/dev/null)"
        OLD_HOSTNAME="$(dbus get magictier_hostname 2>/dev/null)"
        OLD_INSTANCE_NAME="$(dbus get magictier_instance_name 2>/dev/null)"
        OLD_NETWORK_NAME="$(dbus get magictier_network_name 2>/dev/null)"
        OLD_NETWORK_SECRET="$(dbus get magictier_network_secret 2>/dev/null)"
        OLD_IPV4="$(dbus get magictier_ipv4 2>/dev/null)"
        OLD_PEERS="$(dbus get magictier_peers 2>/dev/null)"
        OLD_LISTENERS="$(dbus get magictier_listeners 2>/dev/null)"
        OLD_PROXY_NETWORKS="$(dbus get magictier_proxy_networks 2>/dev/null)"
        OLD_LOG_MAX_BYTES="$(dbus get magictier_log_max_bytes 2>/dev/null)"
        OLD_RSS_LIMIT_KB="$(dbus get magictier_rss_limit_kb 2>/dev/null)"
        echo_date "检测到本插件旧版配置，将迁移到独立 magic 命名空间；不会删除原 magictier 插件的数据。"
        LEGACY_PID="$(cat /var/run/magictier.pid 2>/dev/null)"
        if [ -n "${LEGACY_PID}" ] && [ -r "/proc/${LEGACY_PID}/cmdline" ]; then
            if tr '\000' ' ' < "/proc/${LEGACY_PID}/cmdline" 2>/dev/null | grep -q '/koolshare/bin/magictier-core'; then
                kill "${LEGACY_PID}" 2>/dev/null
                sleep 2
            fi
        fi
    elif [ "$(dbus get softcenter_module_magic_install 2>/dev/null)" = "1" ] || [ -n "$(dbus get magic_version 2>/dev/null)" ]; then
        IS_UPGRADE=1
        OLD_ENABLE="$(dbus get magic_enable 2>/dev/null)"
        OLD_HOSTNAME="$(dbus get magic_hostname 2>/dev/null)"
        OLD_INSTANCE_NAME="$(dbus get magic_instance_name 2>/dev/null)"
        OLD_NETWORK_NAME="$(dbus get magic_network_name 2>/dev/null)"
        OLD_NETWORK_SECRET="$(dbus get magic_network_secret 2>/dev/null)"
        OLD_IPV4="$(dbus get magic_ipv4 2>/dev/null)"
        OLD_PEERS="$(dbus get magic_peers 2>/dev/null)"
        OLD_LISTENERS="$(dbus get magic_listeners 2>/dev/null)"
        OLD_PROXY_NETWORKS="$(dbus get magic_proxy_networks 2>/dev/null)"
        OLD_LOG_MAX_BYTES="$(dbus get magic_log_max_bytes 2>/dev/null)"
        OLD_RSS_LIMIT_KB="$(dbus get magic_rss_limit_kb 2>/dev/null)"
        echo_date "检测到已有 MagicTier Magic 配置，升级后将原样保留。"
    fi

    if [ "${IS_LEGACY_UPGRADE}" = "1" ]; then
        ENABLE="${OLD_ENABLE}"
    else
        ENABLE="$(dbus get magic_enable 2>/dev/null)"
    fi
    if [ "${ENABLE}" = "1" ] && [ -x /koolshare/scripts/magic_config.sh ]; then
        sh /koolshare/scripts/magic_config.sh stop >/dev/null 2>&1
    fi

    echo_date "安装MagicTier插件文件..."
    mkdir -p /koolshare/bin /koolshare/scripts /koolshare/webs /koolshare/res /koolshare/init.d "${INSTALL_DIR}"

    cp -f "${DIR}/bin/magic-core" /koolshare/bin/magic-core
    cp -f "${DIR}/scripts/magic_config.sh" /koolshare/scripts/magic_config.sh
    cp -f "${DIR}/scripts/magic_health.sh" /koolshare/scripts/magic_health.sh
    cp -f "${DIR}/webs/Module_magic.asp" /koolshare/webs/Module_magic.asp
    cp -f "${DIR}/uninstall.sh" /koolshare/scripts/uninstall_magic.sh
    cp -f "${DIR}/version" "${INSTALL_DIR}/version"
    printf '%s\n' 'magic' > "${OWNER_MARKER}"
    rm -f "${LEGACY_OWNER_MARKER}" >/dev/null 2>&1
    [ ! -f "${DIR}/res/magic.png" ] || cp -f "${DIR}/res/magic.png" /koolshare/res/icon-magic.png

    chmod 0755 /koolshare/bin/magic-core
    chmod 0755 /koolshare/scripts/magic_config.sh /koolshare/scripts/magic_health.sh /koolshare/scripts/uninstall_magic.sh
    ln -sf /koolshare/scripts/magic_config.sh /koolshare/init.d/S97magic.sh
    ln -sf /koolshare/scripts/magic_config.sh /koolshare/init.d/N97magic.sh

    if [ "${IS_UPGRADE}" = "1" ]; then
        dbus set magic_enable="${OLD_ENABLE}"
        dbus set magic_hostname="${OLD_HOSTNAME}"
        dbus set magic_instance_name="${OLD_INSTANCE_NAME}"
        dbus set magic_network_name="${OLD_NETWORK_NAME}"
        dbus set magic_network_secret="${OLD_NETWORK_SECRET}"
        dbus set magic_ipv4="${OLD_IPV4}"
        dbus set magic_peers="${OLD_PEERS}"
        dbus set magic_listeners="${OLD_LISTENERS}"
        dbus set magic_proxy_networks="${OLD_PROXY_NETWORKS}"
        [ -z "${OLD_LOG_MAX_BYTES}" ] || dbus set magic_log_max_bytes="${OLD_LOG_MAX_BYTES}"
        [ -z "${OLD_RSS_LIMIT_KB}" ] || dbus set magic_rss_limit_kb="${OLD_RSS_LIMIT_KB}"
    else
        dbus set magic_enable="0"
        dbus set magic_instance_name="default"
        dbus set magic_network_name="default"
        dbus set magic_ipv4="10.144.144.1/24"
        dbus set magic_listeners="tcp://0.0.0.0:11010,udp://0.0.0.0:11010"
    fi

    LOG_MAX_CURRENT="$(dbus get magic_log_max_bytes 2>/dev/null)"
    if [ -z "${LOG_MAX_CURRENT}" ] || [ "${LOG_MAX_CURRENT}" = "524288" ]; then
        dbus set magic_log_max_bytes="131072"
    fi
    RSS_LIMIT_CURRENT="$(dbus get magic_rss_limit_kb 2>/dev/null)"
    if [ -z "${RSS_LIMIT_CURRENT}" ] || [ "${RSS_LIMIT_CURRENT}" = "262144" ]; then
        dbus set magic_rss_limit_kb="65536"
    fi

    dbus set magic_version="${PLVER}"
    dbus set softcenter_module_magic_version="${PLVER}"
    dbus set softcenter_module_magic_install="1"
    dbus set softcenter_module_magic_name="magic"
    dbus set softcenter_module_magic_title="${TITLE}"
    dbus set softcenter_module_magic_description="${DESCR}"
    dbus set softcenter_module_magic_home_url="Module_magic.asp"

    if [ "${ENABLE}" = "1" ]; then
        dbus set magic_enable="1"
        sh /koolshare/scripts/magic_config.sh start >/dev/null 2>&1
    fi

    echo_date "MagicTier ${PLVER} 安装完成。"
}

get_model
platform_test
install_dir_test
namespace_conflict_test
install_now
exit 0
