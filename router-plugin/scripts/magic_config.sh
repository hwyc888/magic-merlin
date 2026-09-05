#!/bin/sh

source /koolshare/scripts/base.sh

eval "$(dbus export magic 2>/dev/null)"

BIN="/koolshare/bin/magic-core"
PIDFILE="/var/run/magic.pid"
MONITOR_PIDFILE="/var/run/magic-monitor.pid"
LOGFILE="/tmp/upload/magic_log.txt"
INTERNAL_LOGFILE="/tmp/upload/magic_internal.log"
LOG_MAX_BYTES="${magic_log_max_bytes:-131072}"
LOG_KEEP_BYTES="65536"
INTERNAL_LOG_MAX_BYTES="65536"
INTERNAL_LOG_KEEP_BYTES="32768"
RSS_LIMIT_KB="${magic_rss_limit_kb:-65536}"
RSS_RESTART_STATE="/tmp/magic_rss_restart.state"
RSS_RESTART_WINDOW=600
RSS_RESTART_MAX=3
LOCK_DIR="/tmp/magic_config.lock"

mkdir -p /tmp/upload

acquire_lock() {
    if mkdir "${LOCK_DIR}" >/dev/null 2>&1; then
        echo "$$" > "${LOCK_DIR}/pid" 2>/dev/null
        trap 'rm -rf "${LOCK_DIR}" >/dev/null 2>&1' EXIT
        return 0
    fi
    return 1
}

lock_or_exit() {
    acquire_lock && return 0
    if [ -n "$2" ]; then
        http_response '{"ok":0,"msg":"busy"}'
    fi
    exit 0
}

pid_is_core() {
    [ -n "$1" ] || return 1
    [ -r "/proc/$1/cmdline" ] || return 1
    tr '\000' ' ' < "/proc/$1/cmdline" 2>/dev/null | grep -q '/koolshare/bin/magic-core'
}

is_running() {
    [ -f "${PIDFILE}" ] || return 1
    PID="$(cat "${PIDFILE}" 2>/dev/null)"
    pid_is_core "${PID}" && kill -0 "${PID}" 2>/dev/null
}

trim_one_log() {
    FILE="$1"
    MAX_BYTES="$2"
    KEEP_BYTES="$3"
    [ -f "${FILE}" ] || return 0
    SIZE="$(wc -c < "${FILE}" 2>/dev/null)"
    [ -n "${SIZE}" ] || SIZE=0
    if [ "${SIZE}" -gt "${MAX_BYTES}" ] 2>/dev/null; then
        tail -c "${KEEP_BYTES}" "${FILE}" > "${FILE}.tmp" 2>/dev/null
        cat "${FILE}.tmp" > "${FILE}" 2>/dev/null
        rm -f "${FILE}.tmp"
    fi
}

trim_logs() {
    trim_one_log "${LOGFILE}" "${LOG_MAX_BYTES}" "${LOG_KEEP_BYTES}"
    trim_one_log "${INTERNAL_LOGFILE}" "${INTERNAL_LOG_MAX_BYTES}" "${INTERNAL_LOG_KEEP_BYTES}"
}

log_user() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOGFILE}"
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
            sleep 20
            [ -f "${PIDFILE}" ] || exit 0
            PID="$(cat "${PIDFILE}" 2>/dev/null)"
            pid_is_core "${PID}" || exit 0
            trim_logs
            RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
            [ -n "${RSS}" ] || RSS=0
            if [ "${RSS}" -gt "${RSS_LIMIT_KB}" ] 2>/dev/null; then
                NOW="$(date +%s 2>/dev/null)"
                [ -n "${NOW}" ] || NOW=0
                WINDOW_START=0
                RESTART_COUNT=0
                if [ -f "${RSS_RESTART_STATE}" ]; then
                    read WINDOW_START RESTART_COUNT < "${RSS_RESTART_STATE}" 2>/dev/null
                fi
                [ -n "${WINDOW_START}" ] || WINDOW_START=0
                [ -n "${RESTART_COUNT}" ] || RESTART_COUNT=0
                if [ "${NOW}" -eq 0 ] || [ $((NOW - WINDOW_START)) -gt "${RSS_RESTART_WINDOW}" ] 2>/dev/null; then
                    WINDOW_START="${NOW}"
                    RESTART_COUNT=0
                fi
                RESTART_COUNT=$((RESTART_COUNT + 1))
                echo "${WINDOW_START} ${RESTART_COUNT}" > "${RSS_RESTART_STATE}" 2>/dev/null

                log_user "⚠ 内存保护触发：MagicTier RSS ${RSS}KB 超过 ${RSS_LIMIT_KB}KB。"
                kill "${PID}" 2>/dev/null
                sleep 2
                pid_is_core "${PID}" && kill -9 "${PID}" 2>/dev/null
                rm -f "${PIDFILE}"

                if [ "${RESTART_COUNT}" -le "${RSS_RESTART_MAX}" ] 2>/dev/null; then
                    log_user "正在自动重启 MagicTier 核心程序，不会重启路由器。"
                    rm -f "${MONITOR_PIDFILE}"
                    ( sleep 3; MAGICTIER_PRESERVE_LOG=1 sh /koolshare/scripts/magic_config.sh start >/dev/null 2>&1 ) &
                else
                    log_user "✗ 10分钟内多次触发内存保护，已停止 MagicTier 自动运行以保护路由器。"
                    dbus set magic_enable="0"
                fi
                exit 0
            fi
        done
    ) >/dev/null 2>&1 &
    echo $! > "${MONITOR_PIDFILE}"
}

start_service() {
    [ "${magic_enable}" = "1" ] || return 0
    [ -x "${BIN}" ] || {
        log_user "✗ MagicTier核心程序不存在，无法启动。"
        return 1
    }

    stop_service
    [ "${MAGICTIER_PRESERVE_LOG}" = "1" ] || : > "${LOGFILE}"
    : > "${INTERNAL_LOGFILE}"

    log_user "正在启动 MagicTier..."
    [ -z "${magic_network_name}" ] || log_user "组网名称：${magic_network_name}"
    [ -z "${magic_ipv4}" ] || log_user "虚拟 IP：${magic_ipv4}"
    if [ -n "${magic_peers}" ]; then
        PEER_COUNT="$(printf '%s' "${magic_peers}" | awk -F',' '{print NF}')"
        log_user "Peer 节点：已配置 ${PEER_COUNT} 个"
    else
        log_user "Peer 节点：未配置，等待其他节点主动连接"
    fi
    if [ -n "${magic_proxy_networks}" ]; then
        PROXY_DISPLAY="$(printf '%s' "${magic_proxy_networks}" | sed 's/,/, /g')"
        log_user "发布子网：${PROXY_DISPLAY}"
    fi
    log_user "正在建立组网连接..."

    set -- "${BIN}" --console-log-level warn --file-log-level off
    [ -z "${magic_hostname}" ] || set -- "$@" --hostname "${magic_hostname}"
    [ -z "${magic_instance_name}" ] || set -- "$@" --instance-name "${magic_instance_name}"
    [ -z "${magic_network_name}" ] || set -- "$@" --network-name "${magic_network_name}"
    [ -z "${magic_network_secret}" ] || set -- "$@" --network-secret "${magic_network_secret}"
    [ -z "${magic_ipv4}" ] || set -- "$@" --ipv4 "${magic_ipv4}"
    [ -z "${magic_peers}" ] || set -- "$@" --peers "${magic_peers}"
    [ -z "${magic_listeners}" ] || set -- "$@" --listeners "${magic_listeners}"
    [ -z "${magic_proxy_networks}" ] || set -- "$@" --proxy-networks "${magic_proxy_networks}"

    if command -v nice >/dev/null 2>&1; then
        MAGICTIER_USER_EVENT_LOG="${LOGFILE}" nice -n 5 "$@" >> "${INTERNAL_LOGFILE}" 2>&1 &
    else
        MAGICTIER_USER_EVENT_LOG="${LOGFILE}" "$@" >> "${INTERNAL_LOGFILE}" 2>&1 &
    fi
    echo $! > "${PIDFILE}"
    sleep 2

    if ! is_running; then
        log_user "✗ MagicTier启动失败，已停止自动运行。"
        dbus set magic_enable="0"
        rm -f "${PIDFILE}"
        trim_logs
        return 1
    fi

    trim_logs
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

ACTION="$1"

case "${ACTION}:$2" in
    status:*|*:6) ;;
    *) lock_or_exit "$@" ;;
esac

case "${ACTION}" in
    start)
        dbus set magic_enable="1"
        magic_enable="1"
        start_service
        exit $?
        ;;
    stop)
        dbus set magic_enable="0"
        magic_enable="0"
        stop_service
        log_user "MagicTier已停止。"
        exit $?
        ;;
    restart)
        dbus set magic_enable="1"
        magic_enable="1"
        stop_service
        start_service
        exit $?
        ;;
    status)
        print_status
        exit $?
        ;;
    clearlog)
        : > "${LOGFILE}"
        : > "${INTERNAL_LOGFILE}"
        exit 0
        ;;
esac

case "$2" in
    1)
        if [ "${magic_enable}" = "1" ]; then
            start_service
        else
            stop_service
            log_user "MagicTier已停止。"
        fi
        http_response "$1"
        ;;
    2)
        dbus set magic_enable="1"
        magic_enable="1"
        start_service
        http_response "$1"
        ;;
    3)
        dbus set magic_enable="0"
        magic_enable="0"
        stop_service
        log_user "MagicTier已停止。"
        http_response "$1"
        ;;
    4)
        dbus set magic_enable="1"
        magic_enable="1"
        stop_service
        start_service
        http_response "$1"
        ;;
    5)
        : > "${LOGFILE}"
        : > "${INTERNAL_LOGFILE}"
        http_response "$1"
        ;;
    6)
        if is_running; then
            PID="$(cat "${PIDFILE}")"
            RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
            [ -n "${RSS}" ] || RSS=0
            http_response "{\"state\":\"running\",\"pid\":${PID},\"rss_kb\":${RSS}}"
        else
            http_response '{"state":"stopped","pid":0,"rss_kb":0}'
        fi
        ;;
    *)
        if [ "${magic_enable}" = "1" ]; then
            start_service
        else
            stop_service
        fi
        ;;
esac

exit $?
