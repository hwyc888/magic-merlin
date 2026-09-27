#!/bin/sh

PIDFILE="/var/run/magic.pid"
CORE_HEALTH_STATE="/tmp/magic_core_health.state"
OUT="/tmp/upload/magic_health.txt"
SAMPLES="${1:-60}"
INTERVAL="${2:-60}"

mkdir -p /tmp/upload
: > "${OUT}"

echo "time,pid,rss_kb,vm_kb,threads,fd_count,socket_fd_count,mem_available_kb,slab_kb,sreclaimable_kb,sunreclaim_kb,socket_mem_kb,tcp_inuse,tcp_orphan,tcp_tw,udp_inuse,conntrack_count,conntrack_max,kmalloc_65536_kb,kmalloc_8192_kb,kmalloc_512_kb,skbuff_head_cache_kb,peer_count,conn_count,reconnect_count,load1" >> "${OUT}"
I=0
while [ "${I}" -lt "${SAMPLES}" ]; do
    NOW="$(date '+%Y-%m-%d %H:%M:%S')"
    PID="$(cat "${PIDFILE}" 2>/dev/null)"
    RSS=0
    VM=0
    TH=0
    FD=0
    SOCKET_FD=0
    PEER_COUNT=0
    CONN_COUNT=0
    RECONNECT_COUNT=0

    if [ -n "${PID}" ] && [ -r "/proc/${PID}/status" ]; then
        RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        VM="$(awk '/VmSize:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        TH="$(awk '/Threads:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        FD="$(ls "/proc/${PID}/fd" 2>/dev/null | wc -l)"
        for ITEM in /proc/"${PID}"/fd/*; do
            [ -L "${ITEM}" ] || continue
            TARGET="$(readlink "${ITEM}" 2>/dev/null)"
            case "${TARGET}" in socket:\[*\]) SOCKET_FD=$((SOCKET_FD + 1)) ;; esac
        done
        if [ -r "${CORE_HEALTH_STATE}" ]; then
            HEALTH_PID=0
            HEALTH_TS=0
            read HEALTH_PID PEER_COUNT CONN_COUNT RECONNECT_COUNT HEALTH_TS < "${CORE_HEALTH_STATE}" 2>/dev/null
            if [ "${HEALTH_PID}" != "${PID}" ]; then
                PEER_COUNT=0
                CONN_COUNT=0
                RECONNECT_COUNT=0
            fi
        fi
    else
        PID=0
    fi

    MEM_AVAILABLE="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    [ -n "${MEM_AVAILABLE}" ] || MEM_AVAILABLE="$(awk '/^MemFree:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    SLAB="$(awk '/^Slab:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    SRECLAIMABLE="$(awk '/^SReclaimable:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"
    SUNRECLAIM="$(awk '/^SUnreclaim:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"

    SOCKET_MEM_PAGES=0
    TCP_INUSE=0
    TCP_ORPHAN=0
    TCP_TW=0
    UDP_INUSE=0
    for SOCKSTAT in /proc/net/sockstat /proc/net/sockstat6; do
        [ -r "${SOCKSTAT}" ] || continue
        PAGES="$(awk '{for(i=1;i<=NF;i++) if($i=="mem" && (i+1)<=NF) sum += $(i+1)} END {print sum+0}' "${SOCKSTAT}" 2>/dev/null)"
        case "${PAGES}" in ''|*[!0-9]*) PAGES=0 ;; esac
        SOCKET_MEM_PAGES=$((SOCKET_MEM_PAGES + PAGES))
        TCP_INUSE=$((TCP_INUSE + $(awk '/^TCP/ {for(i=1;i<=NF;i++) if($i=="inuse") {print $(i+1)+0; exit}}' "${SOCKSTAT}" 2>/dev/null || echo 0)))
        TCP_ORPHAN=$((TCP_ORPHAN + $(awk '/^TCP/ {for(i=1;i<=NF;i++) if($i=="orphan") {print $(i+1)+0; exit}}' "${SOCKSTAT}" 2>/dev/null || echo 0)))
        TCP_TW=$((TCP_TW + $(awk '/^TCP/ {for(i=1;i<=NF;i++) if($i=="tw") {print $(i+1)+0; exit}}' "${SOCKSTAT}" 2>/dev/null || echo 0)))
        UDP_INUSE=$((UDP_INUSE + $(awk '/^UDP/ {for(i=1;i<=NF;i++) if($i=="inuse") {print $(i+1)+0; exit}}' "${SOCKSTAT}" 2>/dev/null || echo 0)))
    done

    CONNTRACK_COUNT="$(cat /proc/sys/net/netfilter/nf_conntrack_count 2>/dev/null)"
    CONNTRACK_MAX="$(cat /proc/sys/net/netfilter/nf_conntrack_max 2>/dev/null)"
    [ -n "${CONNTRACK_COUNT}" ] || CONNTRACK_COUNT=0
    [ -n "${CONNTRACK_MAX}" ] || CONNTRACK_MAX=0

    slab_cache_kb() {
        awk -v name="$1" '$1 == name {print int($3 * $4 / 1024); found=1; exit} END {if (!found) print 0}' /proc/slabinfo 2>/dev/null
    }
    KMALLOC_65536="$(slab_cache_kb kmalloc-65536)"
    KMALLOC_8192="$(slab_cache_kb kmalloc-8192)"
    KMALLOC_512="$(slab_cache_kb kmalloc-512)"
    SKBUFF_HEAD="$(slab_cache_kb skbuff_head_cache)"
    PAGE_SIZE=4096
    if command -v getconf >/dev/null 2>&1; then
        DETECTED_PAGE_SIZE="$(getconf PAGESIZE 2>/dev/null)"
        case "${DETECTED_PAGE_SIZE}" in ''|*[!0-9]*) ;; *) PAGE_SIZE="${DETECTED_PAGE_SIZE}" ;; esac
    fi
    SOCKET_MEM_KB=$((SOCKET_MEM_PAGES * PAGE_SIZE / 1024))
    LOAD="$(awk '{print $1}' /proc/loadavg 2>/dev/null)"

    echo "${NOW},${PID},${RSS:-0},${VM:-0},${TH:-0},${FD:-0},${SOCKET_FD:-0},${MEM_AVAILABLE:-0},${SLAB:-0},${SRECLAIMABLE:-0},${SUNRECLAIM:-0},${SOCKET_MEM_KB:-0},${TCP_INUSE:-0},${TCP_ORPHAN:-0},${TCP_TW:-0},${UDP_INUSE:-0},${CONNTRACK_COUNT:-0},${CONNTRACK_MAX:-0},${KMALLOC_65536:-0},${KMALLOC_8192:-0},${KMALLOC_512:-0},${SKBUFF_HEAD:-0},${PEER_COUNT:-0},${CONN_COUNT:-0},${RECONNECT_COUNT:-0},${LOAD:-0}" >> "${OUT}"
    I=$((I + 1))
    [ "${I}" -ge "${SAMPLES}" ] || sleep "${INTERVAL}"
done

echo "${OUT}"
