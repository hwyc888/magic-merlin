#!/bin/sh

source /koolshare/scripts/base.sh

eval "$(dbus export magic 2>/dev/null)"

BIN="/koolshare/bin/magic-core"
PIDFILE="/var/run/magic.pid"
MONITOR_PIDFILE="/var/run/magic-monitor.pid"
LOGFILE="/tmp/upload/magic_log.txt"
INTERNAL_LOGFILE="/tmp/upload/magic_internal.log"
CORE_HEALTH_STATE="/tmp/magic_core_health.state"
LOG_MAX_BYTES="${magic_log_max_bytes:-131072}"
LOG_KEEP_BYTES="65536"
INTERNAL_LOG_MAX_BYTES="65536"
INTERNAL_LOG_KEEP_BYTES="32768"
RSS_LIMIT_KB="${magic_rss_limit_kb:-65536}"
RSS_HARD_LIMIT_KB="${magic_rss_hard_limit_kb:-98304}"
RSS_OVER_LIMIT_MAX=3
RSS_RESTART_STATE="/tmp/magic_rss_restart.state"
RSS_RESTART_WINDOW=600
RSS_RESTART_MAX=3
CRASH_RESTART_STATE="/tmp/magic_crash_restart.state"
CRASH_RESTART_WINDOW=600
CRASH_RESTART_MAX=3
STOP_MARKER="/tmp/magic_intentional_stop"
PERIODIC_RESTART_STATE="/tmp/magic_periodic_restart.state"
PERIODIC_RESTART_MAX=3
PERIODIC_RESTART_ENABLE="${magic_periodic_restart_enable:-0}"
PERIODIC_RESTART_MODE="${magic_periodic_restart_mode:-interval}"
PERIODIC_RESTART_HOURS="${magic_periodic_restart_hours:-24}"
PERIODIC_RESTART_TIME="${magic_periodic_restart_time:-04:00}"
PERIODIC_RESTART_WEEKDAY="${magic_periodic_restart_weekday:-0}"
PERIODIC_RETRY_MINUTES="${magic_periodic_retry_minutes:-5}"
case "${PERIODIC_RESTART_MODE}" in interval|daily|weekly) ;; *) PERIODIC_RESTART_MODE=interval ;; esac
case "${PERIODIC_RESTART_HOURS}" in ''|*[!0-9]*|0) PERIODIC_RESTART_HOURS=24 ;; esac
case "${PERIODIC_RESTART_TIME}" in [0-1][0-9]:[0-5][0-9]|2[0-3]:[0-5][0-9]) ;; *) PERIODIC_RESTART_TIME="04:00" ;; esac
case "${PERIODIC_RESTART_WEEKDAY}" in 0|1|2|3|4|5|6) ;; *) PERIODIC_RESTART_WEEKDAY=0 ;; esac
case "${PERIODIC_RETRY_MINUTES}" in ''|*[!0-9]*|0) PERIODIC_RETRY_MINUTES=5 ;; esac
[ "${PERIODIC_RESTART_HOURS}" -le 8760 ] 2>/dev/null || PERIODIC_RESTART_HOURS=8760
[ "${PERIODIC_RETRY_MINUTES}" -le 1440 ] 2>/dev/null || PERIODIC_RETRY_MINUTES=1440
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

get_wan_ipv4() {
    for KEY in wan0_ipaddr wan_ipaddr wan1_ipaddr; do
        WAN_IP="$(nvram get "${KEY}" 2>/dev/null)"
        case "${WAN_IP}" in
            ''|0.0.0.0|169.254.*) ;;
            *) echo "${WAN_IP}"; return 0 ;;
        esac
    done
    if command -v ip >/dev/null 2>&1; then
        WAN_IP="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}')"
        case "${WAN_IP}" in
            ''|0.0.0.0|169.254.*) ;;
            *) echo "${WAN_IP}"; return 0 ;;
        esac
    fi
    return 1
}

is_init_invocation() {
    case "$0" in
        /koolshare/init.d/S97magic.sh|/koolshare/init.d/N97magic.sh) return 0 ;;
        *) return 1 ;;
    esac
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
    : > "${STOP_MARKER}"
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
    rm -f "${PIDFILE}" "${CORE_HEALTH_STATE}"
}

periodic_weekday_name() {
    case "$1" in
        0) echo "周日" ;;
        1) echo "周一" ;;
        2) echo "周二" ;;
        3) echo "周三" ;;
        4) echo "周四" ;;
        5) echo "周五" ;;
        6) echo "周六" ;;
        *) echo "周日" ;;
    esac
}

periodic_next_due() {
    NOW="$(date +%s 2>/dev/null)"
    case "${NOW}" in ''|*[!0-9]*) NOW=0 ;; esac
    if [ "${PERIODIC_RESTART_MODE}" = "daily" ] || [ "${PERIODIC_RESTART_MODE}" = "weekly" ]; then
        TARGET_H="${PERIODIC_RESTART_TIME%:*}"
        TARGET_M="${PERIODIC_RESTART_TIME#*:}"
        NOW_H="$(date +%H 2>/dev/null)"
        NOW_M="$(date +%M 2>/dev/null)"
        NOW_S="$(date +%S 2>/dev/null)"
        TARGET_H="${TARGET_H#0}"; [ -n "${TARGET_H}" ] || TARGET_H=0
        TARGET_M="${TARGET_M#0}"; [ -n "${TARGET_M}" ] || TARGET_M=0
        NOW_H="${NOW_H#0}"; [ -n "${NOW_H}" ] || NOW_H=0
        NOW_M="${NOW_M#0}"; [ -n "${NOW_M}" ] || NOW_M=0
        NOW_S="${NOW_S#0}"; [ -n "${NOW_S}" ] || NOW_S=0
        CURRENT_SECONDS=$((NOW_H * 3600 + NOW_M * 60 + NOW_S))
        TARGET_SECONDS=$((TARGET_H * 3600 + TARGET_M * 60))
        if [ "${PERIODIC_RESTART_MODE}" = "weekly" ]; then
            NOW_W="$(date +%w 2>/dev/null)"
            case "${NOW_W}" in 0|1|2|3|4|5|6) ;; *) NOW_W=0 ;; esac
            DAY_DELTA=$((PERIODIC_RESTART_WEEKDAY - NOW_W))
            [ "${DAY_DELTA}" -ge 0 ] 2>/dev/null || DAY_DELTA=$((DAY_DELTA + 7))
            DELTA=$((DAY_DELTA * 86400 + TARGET_SECONDS - CURRENT_SECONDS))
            [ "${DELTA}" -gt 0 ] 2>/dev/null || DELTA=$((DELTA + 604800))
        else
            DELTA=$((TARGET_SECONDS - CURRENT_SECONDS))
            [ "${DELTA}" -gt 0 ] 2>/dev/null || DELTA=$((DELTA + 86400))
        fi
        echo $((NOW + DELTA))
        return 0
    fi
    echo $((NOW + PERIODIC_RESTART_HOURS * 3600))
}

periodic_schedule_reset() {
    if [ "${PERIODIC_RESTART_ENABLE}" != "1" ]; then
        rm -f "${PERIODIC_RESTART_STATE}"
        return 0
    fi
    NEXT_DUE="$(periodic_next_due)"
    echo "scheduled ${NEXT_DUE} 0 0" > "${PERIODIC_RESTART_STATE}" 2>/dev/null
    if [ "${PERIODIC_RESTART_MODE}" = "daily" ]; then
        log_user "定时重启已启用：每天 ${PERIODIC_RESTART_TIME} 执行一次；组网失败后每 ${PERIODIC_RETRY_MINUTES} 分钟重试，最多3次。"
    elif [ "${PERIODIC_RESTART_MODE}" = "weekly" ]; then
        WEEKDAY_NAME="$(periodic_weekday_name "${PERIODIC_RESTART_WEEKDAY}")"
        log_user "定时重启已启用：每${WEEKDAY_NAME} ${PERIODIC_RESTART_TIME} 执行一次；组网失败后每 ${PERIODIC_RETRY_MINUTES} 分钟重试，最多3次。"
    else
        log_user "定时重启已启用：每 ${PERIODIC_RESTART_HOURS} 小时执行一次；组网失败后每 ${PERIODIC_RETRY_MINUTES} 分钟重试，最多3次。"
    fi
}

mesh_ready_since_line() {
    START_LINE="$1"
    [ -f "${LOGFILE}" ] || return 1
    [ -n "${START_LINE}" ] || START_LINE=0
    awk -v start="${START_LINE}" 'NR > start && /组网连接成功/ {found=1} END {exit(found ? 0 : 1)}' "${LOGFILE}" 2>/dev/null
}

periodic_monitor_tick() {
    if [ "${PERIODIC_RESTART_ENABLE}" != "1" ]; then
        rm -f "${PERIODIC_RESTART_STATE}"
        return 0
    fi

    NOW="$(date +%s 2>/dev/null)"
    [ -n "${NOW}" ] || NOW=0
    MODE=""
    DUE=0
    ATTEMPT=0
    START_LINE=0
    if [ -f "${PERIODIC_RESTART_STATE}" ]; then
        read MODE DUE ATTEMPT START_LINE < "${PERIODIC_RESTART_STATE}" 2>/dev/null
    fi
    [ -n "${DUE}" ] || DUE=0
    [ -n "${ATTEMPT}" ] || ATTEMPT=0
    [ -n "${START_LINE}" ] || START_LINE=0

    case "${MODE}" in
        scheduled)
            if [ "${DUE}" -le 0 ] 2>/dev/null; then
                periodic_schedule_reset
                return 0
            fi
            if [ "${NOW}" -ge "${DUE}" ] 2>/dev/null; then
                log_user "⟳ 已到定时维护周期，准备重启 MagicTier 核心服务以释放运行资源。"
                rm -f "${MONITOR_PIDFILE}"
                ( MAGICTIER_PRESERVE_LOG=1 MAGICTIER_PERIODIC_ATTEMPT=1 sh /koolshare/scripts/magic_config.sh periodic-restart >/dev/null 2>&1 ) &
                return 1
            fi
            ;;
        waiting)
            if mesh_ready_since_line "${START_LINE}"; then
                NEXT_DUE="$(periodic_next_due)"
                echo "scheduled ${NEXT_DUE} 0 0" > "${PERIODIC_RESTART_STATE}" 2>/dev/null
                if [ "${PERIODIC_RESTART_MODE}" = "daily" ]; then
                    log_user "✓ 定时重启后组网恢复成功；下一次维护为每天 ${PERIODIC_RESTART_TIME}。"
                elif [ "${PERIODIC_RESTART_MODE}" = "weekly" ]; then
                    WEEKDAY_NAME="$(periodic_weekday_name "${PERIODIC_RESTART_WEEKDAY}")"
                    log_user "✓ 定时重启后组网恢复成功；下一次维护为每${WEEKDAY_NAME} ${PERIODIC_RESTART_TIME}。"
                else
                    log_user "✓ 定时重启后组网恢复成功；下一次维护将在 ${PERIODIC_RESTART_HOURS} 小时后。"
                fi
                return 0
            fi
            if [ "${DUE}" -gt 0 ] 2>/dev/null && [ "${NOW}" -ge "${DUE}" ] 2>/dev/null; then
                if [ "${ATTEMPT}" -ge "${PERIODIC_RESTART_MAX}" ] 2>/dev/null; then
                    log_user "✗ 定时维护连续3次未检测到组网连接成功；已关闭定时重启，MagicTier 核心继续运行并自行重连。"
                    dbus set magic_periodic_restart_enable="0"
                    PERIODIC_RESTART_ENABLE=0
                    rm -f "${PERIODIC_RESTART_STATE}"
                    return 0
                fi
                NEXT_ATTEMPT=$((ATTEMPT + 1))
                log_user "⚠ 定时重启后 ${PERIODIC_RETRY_MINUTES} 分钟内仍未检测到组网成功，准备第 ${NEXT_ATTEMPT} 次重启。"
                rm -f "${MONITOR_PIDFILE}"
                ( MAGICTIER_PRESERVE_LOG=1 MAGICTIER_PERIODIC_ATTEMPT="${NEXT_ATTEMPT}" sh /koolshare/scripts/magic_config.sh periodic-restart >/dev/null 2>&1 ) &
                return 1
            fi
            ;;
        *)
            periodic_schedule_reset
            ;;
    esac
    return 0
}

start_monitor() {
    stop_monitor
    (
        LAST_WAN_IP="$(get_wan_ipv4 2>/dev/null)"
        WAN_WAS_DOWN=0
        RSS_OVER_LIMIT_COUNT=0
        while :; do
            sleep 20
            PID=""
            [ ! -f "${PIDFILE}" ] || PID="$(cat "${PIDFILE}" 2>/dev/null)"
            if [ -z "${PID}" ] || ! pid_is_core "${PID}" || ! kill -0 "${PID}" 2>/dev/null; then
                [ ! -f "${STOP_MARKER}" ] || exit 0
                ENABLED="$(dbus get magic_enable 2>/dev/null)"
                [ "${ENABLED}" = "1" ] || exit 0

                NOW="$(date +%s 2>/dev/null)"
                [ -n "${NOW}" ] || NOW=0
                WINDOW_START=0
                RESTART_COUNT=0
                if [ -f "${CRASH_RESTART_STATE}" ]; then
                    read WINDOW_START RESTART_COUNT < "${CRASH_RESTART_STATE}" 2>/dev/null
                fi
                [ -n "${WINDOW_START}" ] || WINDOW_START=0
                [ -n "${RESTART_COUNT}" ] || RESTART_COUNT=0
                if [ "${NOW}" -eq 0 ] || [ $((NOW - WINDOW_START)) -gt "${CRASH_RESTART_WINDOW}" ] 2>/dev/null; then
                    WINDOW_START="${NOW}"
                    RESTART_COUNT=0
                fi
                RESTART_COUNT=$((RESTART_COUNT + 1))
                echo "${WINDOW_START} ${RESTART_COUNT}" > "${CRASH_RESTART_STATE}" 2>/dev/null
                rm -f "${PIDFILE}" "${MONITOR_PIDFILE}"

                if [ "${RESTART_COUNT}" -le "${CRASH_RESTART_MAX}" ] 2>/dev/null; then
                    log_user "⚠ 检测到 MagicTier 核心程序异常退出。"
                    log_user "正在自动恢复组网连接，不会重启路由器。"
                    ( sleep 3; MAGICTIER_PRESERVE_LOG=1 sh /koolshare/scripts/magic_config.sh boot >/dev/null 2>&1 ) &
                else
                    log_user "✗ MagicTier 核心程序在10分钟内连续异常超过3次，已停止自动运行以保护路由器。"
                    dbus set magic_enable="0"
                fi
                exit 0
            fi
            trim_logs

            CURRENT_WAN_IP="$(get_wan_ipv4 2>/dev/null)"
            if [ -z "${CURRENT_WAN_IP}" ]; then
                [ -z "${LAST_WAN_IP}" ] || WAN_WAS_DOWN=1
            else
                if [ "${WAN_WAS_DOWN}" = "1" ]; then
                    log_user "⚠ 检测到 WAN 连接恢复，当前地址：${CURRENT_WAN_IP}。"
                    log_user "保持 MagicTier 核心运行，由核心自动恢复 Peer 连接，避免远程桌面因重启核心而断开。"
                elif [ -n "${LAST_WAN_IP}" ] && [ "${CURRENT_WAN_IP}" != "${LAST_WAN_IP}" ]; then
                    log_user "⚠ 检测到 WAN 地址变化：${LAST_WAN_IP} → ${CURRENT_WAN_IP}。"
                    log_user "保持 MagicTier 核心运行，由核心自动重连，不主动重启核心。"
                fi
                LAST_WAN_IP="${CURRENT_WAN_IP}"
                WAN_WAS_DOWN=0
            fi

            RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
            [ -n "${RSS}" ] || RSS=0
            RSS_TRIGGER=0
            if [ "${RSS}" -gt "${RSS_HARD_LIMIT_KB}" ] 2>/dev/null; then
                RSS_TRIGGER=1
                RSS_OVER_LIMIT_COUNT="${RSS_OVER_LIMIT_MAX}"
            elif [ "${RSS}" -gt "${RSS_LIMIT_KB}" ] 2>/dev/null; then
                RSS_OVER_LIMIT_COUNT=$((RSS_OVER_LIMIT_COUNT + 1))
                if [ "${RSS_OVER_LIMIT_COUNT}" -eq 1 ]; then
                    log_user "⚠ MagicTier RSS ${RSS}KB 超过观察线 ${RSS_LIMIT_KB}KB，将继续观察，避免瞬时流量峰值误重启。"
                fi
                if [ "${RSS_OVER_LIMIT_COUNT}" -ge "${RSS_OVER_LIMIT_MAX}" ] 2>/dev/null; then
                    RSS_TRIGGER=1
                fi
            else
                RSS_OVER_LIMIT_COUNT=0
            fi

            if [ "${RSS_TRIGGER}" = "1" ]; then
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

                if [ "${RSS}" -gt "${RSS_HARD_LIMIT_KB}" ] 2>/dev/null; then
                    log_user "⚠ 内存保护触发：MagicTier RSS ${RSS}KB 超过硬限制 ${RSS_HARD_LIMIT_KB}KB。"
                else
                    log_user "⚠ 内存保护触发：MagicTier RSS 已连续 ${RSS_OVER_LIMIT_COUNT} 次超过 ${RSS_LIMIT_KB}KB。"
                fi
                kill "${PID}" 2>/dev/null
                sleep 2
                pid_is_core "${PID}" && kill -9 "${PID}" 2>/dev/null
                rm -f "${PIDFILE}"

                if [ "${RESTART_COUNT}" -le "${RSS_RESTART_MAX}" ] 2>/dev/null; then
                    log_user "正在自动重启 MagicTier 核心程序，不会重启路由器。"
                    rm -f "${MONITOR_PIDFILE}"
                    ( sleep 3; MAGICTIER_PRESERVE_LOG=1 sh /koolshare/scripts/magic_config.sh boot >/dev/null 2>&1 ) &
                else
                    log_user "✗ 10分钟内多次触发内存保护，已停止 MagicTier 自动运行以保护路由器。"
                    dbus set magic_enable="0"
                fi
                exit 0
            fi

            if ! periodic_monitor_tick; then
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
    rm -f "${STOP_MARKER}"
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

    set -- "${BIN}" --console-log-level warn --file-log-level off --dev-name magic0
    [ -z "${magic_hostname}" ] || set -- "$@" --hostname "${magic_hostname}"
    [ -z "${magic_instance_name}" ] || set -- "$@" --instance-name "${magic_instance_name}"
    [ -z "${magic_network_name}" ] || set -- "$@" --network-name "${magic_network_name}"
    [ -z "${magic_network_secret}" ] || set -- "$@" --network-secret "${magic_network_secret}"
    [ -z "${magic_ipv4}" ] || set -- "$@" --ipv4 "${magic_ipv4}"
    [ -z "${magic_peers}" ] || set -- "$@" --peers "${magic_peers}"
    [ -z "${magic_listeners}" ] || set -- "$@" --listeners "${magic_listeners}"
    [ -z "${magic_proxy_networks}" ] || set -- "$@" --proxy-networks "${magic_proxy_networks}"

    rm -f "${CORE_HEALTH_STATE}"
    MAGICTIER_HEALTH_STATUS_FILE="${CORE_HEALTH_STATE}" MAGICTIER_USER_EVENT_LOG="${LOGFILE}" "$@" >> "${INTERNAL_LOGFILE}" 2>&1 &
    echo $! > "${PIDFILE}"
    sleep 2

    if ! is_running; then
        log_user "✗ MagicTier启动失败，已停止自动运行。"
        dbus set magic_enable="0"
        rm -f "${PIDFILE}"
        trim_logs
        return 1
    fi

    if [ "${MAGICTIER_PERIODIC_RESTART:-0}" != "1" ]; then
        periodic_schedule_reset
    fi
    trim_logs
    start_monitor
    return 0
}

periodic_status_fields() {
    if [ "${magic_enable:-0}" != "1" ] || [ "${PERIODIC_RESTART_ENABLE}" != "1" ]; then
        echo '"periodic_enabled":0,"periodic_mode":"disabled","periodic_due":0,"periodic_remaining":0,"periodic_attempt":0,"periodic_max":3'
        return 0
    fi

    MODE="initializing"
    DUE=0
    ATTEMPT=0
    START_LINE=0
    if [ -f "${PERIODIC_RESTART_STATE}" ]; then
        read MODE DUE ATTEMPT START_LINE < "${PERIODIC_RESTART_STATE}" 2>/dev/null
    fi
    case "${MODE}" in scheduled|waiting) ;; *) MODE="initializing" ;; esac
    case "${DUE}" in ''|*[!0-9]*) DUE=0 ;; esac
    case "${ATTEMPT}" in ''|*[!0-9]*) ATTEMPT=0 ;; esac
    [ "${ATTEMPT}" -le "${PERIODIC_RESTART_MAX}" ] 2>/dev/null || ATTEMPT="${PERIODIC_RESTART_MAX}"

    NOW="$(date +%s 2>/dev/null)"
    case "${NOW}" in ''|*[!0-9]*) NOW=0 ;; esac
    REMAINING=0
    if [ "${DUE}" -gt "${NOW}" ] 2>/dev/null; then
        REMAINING=$((DUE - NOW))
    fi
    printf '"periodic_enabled":1,"periodic_mode":"%s","periodic_due":%s,"periodic_remaining":%s,"periodic_attempt":%s,"periodic_max":%s' \
        "${MODE}" "${DUE}" "${REMAINING}" "${ATTEMPT}" "${PERIODIC_RESTART_MAX}"
}

health_status_fields() {
    MEM_AVAILABLE_KB="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    [ -n "${MEM_AVAILABLE_KB}" ] || MEM_AVAILABLE_KB="$(awk '/^MemFree:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    SLAB_KB="$(awk '/^Slab:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    [ -n "${MEM_AVAILABLE_KB}" ] || MEM_AVAILABLE_KB=0
    [ -n "${SLAB_KB}" ] || SLAB_KB=0

    SOCKET_MEM_PAGES=0
    for SOCKSTAT in /proc/net/sockstat /proc/net/sockstat6; do
        [ -r "${SOCKSTAT}" ] || continue
        PAGES="$(awk '{for(i=1;i<=NF;i++) if($i=="mem" && (i+1)<=NF) sum += $(i+1)} END {print sum+0}' "${SOCKSTAT}" 2>/dev/null)"
        case "${PAGES}" in ''|*[!0-9]*) PAGES=0 ;; esac
        SOCKET_MEM_PAGES=$((SOCKET_MEM_PAGES + PAGES))
    done
    PAGE_SIZE=4096
    if command -v getconf >/dev/null 2>&1; then
        DETECTED_PAGE_SIZE="$(getconf PAGESIZE 2>/dev/null)"
        case "${DETECTED_PAGE_SIZE}" in ''|*[!0-9]*) ;; *) PAGE_SIZE="${DETECTED_PAGE_SIZE}" ;; esac
    fi
    SOCKET_MEM_KB=$((SOCKET_MEM_PAGES * PAGE_SIZE / 1024))

    SOCKET_COUNT=0
    PEER_COUNT=0
    CONN_COUNT=0
    RECONNECT_COUNT=0
    if is_running; then
        HEALTH_PID="$(cat "${PIDFILE}" 2>/dev/null)"
        for FD in /proc/"${HEALTH_PID}"/fd/*; do
            [ -L "${FD}" ] || continue
            TARGET="$(readlink "${FD}" 2>/dev/null)"
            case "${TARGET}" in socket:\[*\]) SOCKET_COUNT=$((SOCKET_COUNT + 1)) ;; esac
        done

        if [ -r "${CORE_HEALTH_STATE}" ]; then
            CORE_HEALTH_PID=0
            CORE_HEALTH_TS=0
            read CORE_HEALTH_PID PEER_COUNT CONN_COUNT RECONNECT_COUNT CORE_HEALTH_TS < "${CORE_HEALTH_STATE}" 2>/dev/null
            if [ "${CORE_HEALTH_PID}" != "${HEALTH_PID}" ]; then
                PEER_COUNT=0
                CONN_COUNT=0
                RECONNECT_COUNT=0
            fi
        fi
    fi

    for VALUE_NAME in MEM_AVAILABLE_KB SLAB_KB SOCKET_MEM_KB SOCKET_COUNT PEER_COUNT CONN_COUNT RECONNECT_COUNT; do
        eval 'VALUE=$'"${VALUE_NAME}"
        case "${VALUE}" in ''|*[!0-9]*) eval "${VALUE_NAME}=0" ;; esac
    done

    printf '"mem_available_kb":%s,"slab_kb":%s,"socket_mem_kb":%s,"socket_count":%s,"peer_count":%s,"conn_count":%s,"reconnect_count":%s' \
        "${MEM_AVAILABLE_KB}" "${SLAB_KB}" "${SOCKET_MEM_KB}" "${SOCKET_COUNT}" "${PEER_COUNT}" "${CONN_COUNT}" "${RECONNECT_COUNT}"
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
    boot)
        [ "${magic_enable}" = "1" ] || exit 0
        start_service
        exit $?
        ;;
    periodic-restart)
        ENABLED="$(dbus get magic_enable 2>/dev/null)"
        PERIODIC_ENABLED="$(dbus get magic_periodic_restart_enable 2>/dev/null)"
        [ "${ENABLED}" = "1" ] && [ "${PERIODIC_ENABLED}" = "1" ] || exit 0
        ATTEMPT="${MAGICTIER_PERIODIC_ATTEMPT:-1}"
        case "${ATTEMPT}" in ''|*[!0-9]*|0) ATTEMPT=1 ;; esac
        [ "${ATTEMPT}" -le "${PERIODIC_RESTART_MAX}" ] 2>/dev/null || ATTEMPT="${PERIODIC_RESTART_MAX}"
        stop_service
        log_user "⟳ 正在执行定时维护重启（第 ${ATTEMPT} 次尝试），不会重启路由器。"
        START_LINE="$(wc -l < "${LOGFILE}" 2>/dev/null)"
        [ -n "${START_LINE}" ] || START_LINE=0
        NOW="$(date +%s 2>/dev/null)"
        [ -n "${NOW}" ] || NOW=0
        RETRY_DUE=$((NOW + PERIODIC_RETRY_MINUTES * 60))
        echo "waiting ${RETRY_DUE} ${ATTEMPT} ${START_LINE}" > "${PERIODIC_RESTART_STATE}" 2>/dev/null
        MAGICTIER_PERIODIC_RESTART=1
        MAGICTIER_PRESERVE_LOG=1
        start_service
        exit $?
        ;;
    start)
        if is_init_invocation; then
            [ "${magic_enable}" = "1" ] || exit 0
        else
            dbus set magic_enable="1"
            magic_enable="1"
        fi
        start_service
        exit $?
        ;;
    stop)
        if ! is_init_invocation; then
            dbus set magic_enable="0"
            magic_enable="0"
        fi
        stop_service
        rm -f "${PERIODIC_RESTART_STATE}"
        log_user "MagicTier已停止。"
        exit $?
        ;;
    restart)
        if is_init_invocation; then
            [ "${magic_enable}" = "1" ] || exit 0
        else
            dbus set magic_enable="1"
            magic_enable="1"
        fi
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
            rm -f "${PERIODIC_RESTART_STATE}"
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
        rm -f "${PERIODIC_RESTART_STATE}"
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
        PERIODIC_FIELDS="$(periodic_status_fields)"
        HEALTH_FIELDS="$(health_status_fields)"
        if is_running; then
            PID="$(cat "${PIDFILE}")"
            RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
            [ -n "${RSS}" ] || RSS=0
            http_response "{\"state\":\"running\",\"pid\":${PID},\"rss_kb\":${RSS},${PERIODIC_FIELDS},${HEALTH_FIELDS}}"
        else
            http_response "{\"state\":\"stopped\",\"pid\":0,\"rss_kb\":0,${PERIODIC_FIELDS},${HEALTH_FIELDS}}"
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
