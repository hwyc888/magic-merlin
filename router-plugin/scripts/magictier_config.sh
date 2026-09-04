#!/bin/sh

source /koolshare/scripts/base.sh

eval "$(dbus export magictier 2>/dev/null)"

BIN="/koolshare/bin/magictier-core"
PIDFILE="/var/run/magictier.pid"
LOGFILE="/tmp/upload/magictier_log.txt"

mkdir -p /tmp/upload

is_running() {
    [ -f "${PIDFILE}" ] || return 1
    PID="$(cat "${PIDFILE}" 2>/dev/null)"
    [ -n "${PID}" ] || return 1
    kill -0 "${PID}" 2>/dev/null
}

stop_service() {
    if is_running; then
        kill "$(cat "${PIDFILE}")" 2>/dev/null
        sleep 1
        if is_running; then
            kill -9 "$(cat "${PIDFILE}")" 2>/dev/null
        fi
    fi
    rm -f "${PIDFILE}"
    killall magictier-core >/dev/null 2>&1
}

start_service() {
    [ "${magictier_enable}" = "1" ] || return 0
    [ -x "${BIN}" ] || {
        echo "MagicTier核心程序不存在：${BIN}" >> "${LOGFILE}"
        return 1
    }

    stop_service

    set -- "${BIN}"
    [ -z "${magictier_network_name}" ] || set -- "$@" --network-name "${magictier_network_name}"
    [ -z "${magictier_network_secret}" ] || set -- "$@" --network-secret "${magictier_network_secret}"
    [ -z "${magictier_ipv4}" ] || set -- "$@" --ipv4 "${magictier_ipv4}"
    [ -z "${magictier_peers}" ] || set -- "$@" --peers "${magictier_peers}"
    [ -z "${magictier_listeners}" ] || set -- "$@" --listeners "${magictier_listeners}"
    [ -z "${magictier_proxy_networks}" ] || set -- "$@" --proxy-networks "${magictier_proxy_networks}"

    echo "[$(date '+%Y-%m-%d %H:%M:%S')] 启动MagicTier" > "${LOGFILE}"
    "$@" >> "${LOGFILE}" 2>&1 &
    echo $! > "${PIDFILE}"
    sleep 1

    if ! is_running; then
        echo "MagicTier启动失败，请查看日志。" >> "${LOGFILE}"
        rm -f "${PIDFILE}"
        return 1
    fi

    return 0
}

case "$1" in
    start)
        start_service
        ;;
    stop)
        stop_service
        ;;
    restart)
        stop_service
        start_service
        ;;
    status)
        if is_running; then
            echo "running pid=$(cat "${PIDFILE}")"
            exit 0
        fi
        echo "stopped"
        exit 1
        ;;
    1)
        http_response "$1"
        if [ "${magictier_enable}" = "1" ]; then
            start_service
        else
            stop_service
        fi
        ;;
    *)
        if [ "${magictier_enable}" = "1" ]; then
            start_service
        else
            stop_service
        fi
        ;;
esac

exit $?
