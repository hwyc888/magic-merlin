#!/bin/sh

PIDFILE="/var/run/magictier.pid"
OUT="/tmp/upload/magictier_health.txt"
SAMPLES="${1:-60}"
INTERVAL="${2:-60}"

mkdir -p /tmp/upload
: > "${OUT}"

echo "time,pid,rss_kb,vm_kb,threads,fd_count,load1" >> "${OUT}"
I=0
while [ "${I}" -lt "${SAMPLES}" ]; do
    NOW="$(date '+%Y-%m-%d %H:%M:%S')"
    PID="$(cat "${PIDFILE}" 2>/dev/null)"
    if [ -n "${PID}" ] && [ -r "/proc/${PID}/status" ]; then
        RSS="$(awk '/VmRSS:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        VM="$(awk '/VmSize:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        TH="$(awk '/Threads:/ {print $2; exit}' "/proc/${PID}/status" 2>/dev/null)"
        FD="$(ls "/proc/${PID}/fd" 2>/dev/null | wc -l)"
        LOAD="$(awk '{print $1}' /proc/loadavg 2>/dev/null)"
        echo "${NOW},${PID},${RSS:-0},${VM:-0},${TH:-0},${FD:-0},${LOAD:-0}" >> "${OUT}"
    else
        echo "${NOW},0,0,0,0,0,0" >> "${OUT}"
    fi
    I=$((I + 1))
    [ "${I}" -ge "${SAMPLES}" ] || sleep "${INTERVAL}"
done

echo "${OUT}"
