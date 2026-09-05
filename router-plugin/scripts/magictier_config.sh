#!/bin/sh

source /koolshare/scripts/base.sh

eval "$(dbus export magictier 2>/dev/null)"

BIN="/koolshare/bin/magictier-core"
PIDFILE="/var/run/magictier.pid"
MONITOR_PIDFILE="/var/run/magictier-monitor.pid"
LOGFILE="/tmp/upload/magictier_log.txt"
LOG_MAX_BYTES="${magictier_log_max_bytes:-524288}"
LOG_KEEP_BYTES="262144"
RSS_LIMIT_KB="${magictier_rss_limit_kb:-262144}"

mkdir -p /tmp/upload

pid_is_core() {
    [ -n "$1" ] || return 1
    [ -r "/proc/$1/cmdline" ] || return 1
    tr '\000' ' ' < "/proc/$1/cmdline" 2>/dev/null | grep -q '/koolshare/bin/magictier-core'
}

is_running() {
    [ -f "${PIDFILE}" ] || return 1
    PID="$(cat "${PIDFILE}" 2>/dev/null)"
    pid_is_core "${PID}" && kill -0 "${PID}" 2>/dev/null
}

trim_log() {
    [ -f "${LOGFILE}" ] || return 0
    SIZE="$(wc -c < "${LOGFILE}" 2>/dev/null)"
    [ -n "${SIZE}" ] || SIZE=0
    if [ "${SIZE}" -gt "${LOG_MAX_BYTES}" ] 2>/dev/null; then
        tail -c "${LOG_KEEP_BYTES}" "${LOGFILE}" > "${LOGFILE}.tmp" 2>/dev/null
        cat "${LOGFILE}.tmp" > "${LOGFILE}" 2>/dev/null
        rm -f "${LOGFILE}.tmp"
    fi
}

stop_monitor() {
    if [ -f "${MONITOR_PIDFILE}" ]; then
        MPID="$(cat "${MONITOR_PIDFILE}" 2>/dev/null)"
        [ -z "${MPID}" ] || kill "${MPID}" 2>/dev/null
    fi
    rm -f "${MONITOR_PIDFILE}"
}

stop_service() {
    stop_monitor
    if is_running; then
        PID="$(cat "${PIDFILE}")"
        kill "${PID}" 2>/dev/null
        I=0
        while kill -0 "${PID}" 2>/dev/null && [ "${I}" -lt 10 ]; do
            sleep 1
            I=$((I + 1))
        done
        if pid_is_core "${PID}" && kill -0 "${PID}" 2>/dev/null; then
            kill -9 "${PID}" 2>/dev/null
        fi
    fi
    rm -f "${PIDFILE}"
}

start_monitor() {
    stop_monitor
    (
        while :; do
            sleep 60
            [ -f "${PIDFILE}" ] || exit 0
            PID="$(cat "${PIDFILE}" 2>/dev/null)"
            pid_is_core "${PID}" || exit 0
            trim_log
            RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
            [ -n "${RSS}" ] || RSS=0
            if [ "${RSS}" -gt "${RSS_LIMIT_KB}" ] 2>/dev/null; then
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] 内存保护触发：RSS=${RSS}KB，限制=${RSS_LIMIT_KB}KB，已停止MagicTier并关闭自动启动。" >> "${LOGFILE}"
                dbus set magictier_enable="0"
                kill "${PID}" 2>/dev/null
                sleep 2
                pid_is_core "${PID}" && kill -9 "${PID}" 2>/dev/null
                rm -f "${PIDFILE}"
                exit 0
            fi
        done
    ) >/dev/null 2>&1 &
    echo $! > "${MONITOR_PIDFILE}"
}

start_service() {
    [ "${magictier_enable}" = "1" ] || return 0
    [ -x "${BIN}" ] || {
        echo "MagicTier核心程序不存在：${BIN}" >> "${LOGFILE}"
        return 1
    }

    stop_service
    trim_log

    set -- "${BIN}" --console-log-level warn --file-log-level off
    [ -z "${magictier_network_name}" ] || set -- "$@" --network-name "${magictier_network_name}"
    [ -z "${magictier_network_secret}" ] || set -- "$@" --network-secret "${magictier_network_secret}"
    [ -z "${magictier_ipv4}" ] || set -- "$@" --ipv4 "${magictier_ipv4}"
    [ -z "${magictier_peers}" ] || set -- "$@" --peers "${magictier_peers}"
    [ -z "${magictier_listeners}" ] || set -- "$@" --listeners "${magictier_listeners}"
    [ -z "${magictier_proxy_networks}" ] || set -- "$@" --proxy-networks "${magictier_proxy_networks}"

    echo "[$(date '+%Y-%m-%d %H:%M:%S')] 启动MagicTier" >> "${LOGFILE}"
    if command -v nice >/dev/null 2>&1; then
        nice -n 5 "$@" >> "${LOGFILE}" 2>&1 &
    else
        "$@" >> "${LOGFILE}" 2>&1 &
    fi
    echo $! > "${PIDFILE}"
    sleep 2

    if ! is_running; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] MagicTier启动失败，已停止自动运行。" >> "${LOGFILE}"
        dbus set magictier_enable="0"
        rm -f "${PIDFILE}"
        return 1
    fi

    start_monitor
    return 0
}

print_status() {
    if is_running; then
        PID="$(cat "${PIDFILE}")"
        RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        [ -n "${RSS}" ] || RSS=0
        echo "running|${PID}|${RSS}"
    else
        echo "stopped|0|0"
    fi
}

case "$1" in
    start)
        dbus set magictier_enable="1"
        magictier_enable="1"
        start_service
        ;;
    stop)
        dbus set magictier_enable="0"
        magictier_enable="0"
        stop_service
        ;;
    restart)
        dbus set magictier_enable="1"
        magictier_enable="1"
        stop_service
        start_service
        ;;
    status|2)
        print_status
        ;;
    log|3)
        trim_log
        tail -n 200 "${LOGFILE}" 2>/dev/null
        ;;
    clearlog|4)
        : > "${LOGFILE}"
        echo "日志已清空"
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
