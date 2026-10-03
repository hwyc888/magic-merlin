#!/usr/bin/env python3
"""Execute the router shell dispatcher with isolated dbus/core doubles, not a router."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[1]
SHELL = os.environ.get('ROUTER_TEST_SHELL') or shutil.which('sh')
if not SHELL and os.name == 'nt':
    SHELL = r'C:\Program Files\Git\bin\bash.exe'


def shell_path(path):
    value = str(path).replace('\\', '/')
    if len(value) > 1 and value[1] == ':':
        value = '/' + value[0].lower() + value[2:]
    return value


class BootFlow(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='magic-boot-test-')
        self.root = Path(self.temp.name)
        self.r = shell_path(self.root)
        for p in ('koolshare/scripts', 'koolshare/init.d', 'koolshare/magic', 'dbus', 'run', 'tmp/upload'):
            (self.root / p).mkdir(parents=True)
        self.set_dbus('magic_enable', '1')
        self.set_dbus('magic_version', 'test')
        (self.root / 'ready').touch()
        (self.root / 'koolshare/magic/.autostart-enabled').write_text('1\n')
        base = '''#!/bin/sh
TEST_ROOT='@ROOT@'
ID="$1"
dbus() {
    [ -f "$TEST_ROOT/ready" ] || return 1
    case "$1" in
        export)
            for k in magic_enable magic_version; do
                printf "%s='%s'\\n" "$k" "$(cat "$TEST_ROOT/dbus/$k")"
            done
            ;;
        get) cat "$TEST_ROOT/dbus/$2" 2>/dev/null ;;
        set) pair="$2"; printf '%s' "${pair#*=}" > "$TEST_ROOT/dbus/${pair%%=*}" ;;
    esac
}
http_response() { printf '%s\\n' "$*"; }
'''.replace('@ROOT@', self.r)
        (self.root / 'koolshare/scripts/base.sh').write_text(base, encoding='utf8')
        source = (REPO / 'router-plugin/scripts/magic_config.sh').read_text(encoding='utf8')
        source = source.replace('source /koolshare/scripts/base.sh', '. /koolshare/scripts/base.sh')
        source = source.replace('/koolshare', self.r + '/koolshare')
        source = source.replace('/var/run/', self.r + '/run/')
        source = source.replace('"/tmp/', '"' + self.r + '/tmp/')
        source = source.replace('mkdir -p /tmp/upload', 'mkdir -p "' + self.r + '/tmp/upload"')
        source = source.replace('BOOT_RETRY_DELAY=10', 'BOOT_RETRY_DELAY=0.05')
        source = source.replace('BOOT_RETRY_MAX=12', 'BOOT_RETRY_MAX=4')
        source = source.replace('sleep 1', 'sleep 0.02')
        # Replace only the core process with a controllable test double. Dispatch,
        # persistence, locking, retry scheduling and logging are the real functions.
        double = '''
is_running() { [ -f "$TEST_ROOT/running" ]; }
stop_service() { : > "${STOP_MARKER}"; rm -f "$TEST_ROOT/running"; }
start_service() {
    n=$(cat "$TEST_ROOT/attempts" 2>/dev/null); n=${n:-0}; n=$((n+1))
    printf '%s' "$n" > "$TEST_ROOT/attempts"
    failures=$(cat "$TEST_ROOT/failures" 2>/dev/null); failures=${failures:-0}
    [ "$n" -gt "$failures" ] || return 1
    rm -f "${STOP_MARKER}"
    : > "$TEST_ROOT/running"
    return 0
}
'''
        marker = 'ACTION="$1"'
        self.assertEqual(source.count(marker), 1)
        source = source.replace(marker, double + '\n' + marker)
        self.script = self.root / 'koolshare/scripts/magic_config.sh'
        self.script.write_text(source, encoding='utf8')
        # Copies retain $0 invocation identity on Windows too (no symlink privileges).
        for hook in ('S97magic.sh', 'N97magic.sh', 'V97magic.sh'):
            (self.root / 'koolshare/init.d' / hook).write_text(source, encoding='utf8')

    def tearDown(self):
        # Let any bounded retry exit without killing unrelated processes.
        self.set_dbus('magic_enable', '0')
        (self.root / 'koolshare/magic/.autostart-enabled').unlink(missing_ok=True)
        (self.root / 'tmp/magic_intentional_stop').touch()
        time.sleep(0.2)
        self.temp.cleanup()

    def set_dbus(self, key, value):
        (self.root / 'dbus' / key).write_text(value, encoding='utf8')

    def invoke(self, name='V97magic.sh', *args):
        path = self.script if name == 'magic_config.sh' else self.root / 'koolshare/init.d' / name
        result = subprocess.run([SHELL, shell_path(path), *args], stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=5, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def wait_for(self, name, timeout=3):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if (self.root / name).exists():
                return
            time.sleep(0.03)
        self.fail('missing ' + name)

    def attempts(self):
        path = self.root / 'attempts'
        return int(path.read_text()) if path.exists() else 0

    def test_v_hook_without_arguments_retries_first_failure(self):
        (self.root / 'failures').write_text('1')
        self.invoke()
        self.wait_for('running')
        self.assertEqual(self.attempts(), 2)

    def test_start_nat_and_duplicate_boot_events_are_idempotent(self):
        self.invoke()
        self.invoke('S97magic.sh', 'start')
        self.invoke('N97magic.sh', 'start_nat')
        self.assertEqual(self.attempts(), 1)

    def test_unavailable_dbus_does_not_start_with_empty_config(self):
        (self.root / 'ready').unlink()
        self.invoke()
        self.assertEqual(self.attempts(), 0)
        (self.root / 'ready').touch()
        self.wait_for('running')
        self.assertEqual(self.attempts(), 1)

    def test_disabled_plugin_stays_disabled(self):
        self.set_dbus('magic_enable', '0')
        (self.root / 'koolshare/magic/.autostart-enabled').unlink()
        self.invoke()
        time.sleep(0.2)
        self.assertEqual(self.attempts(), 0)
        self.assertEqual((self.root / 'dbus/magic_enable').read_text(), '0')

    def test_enabled_marker_restores_enable(self):
        self.set_dbus('magic_enable', '0')
        self.invoke()
        self.wait_for('running')
        self.assertEqual((self.root / 'dbus/magic_enable').read_text(), '1')

    def test_retries_are_bounded_and_preserve_autostart(self):
        (self.root / 'failures').write_text('999')
        self.invoke()
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            if self.attempts() >= 5 and not (self.root / 'run/magic-boot-retry.pid').exists():
                break
            time.sleep(0.05)
        self.assertEqual(self.attempts(), 5)
        self.assertFalse((self.root / 'running').exists())
        self.assertTrue((self.root / 'koolshare/magic/.autostart-enabled').exists())
        self.assertEqual((self.root / 'dbus/magic_enable').read_text(), '1')
        self.assertFalse((self.root / 'run/magic-boot-retry.pid').exists())

    def test_busy_boot_event_is_retried_after_other_operation_finishes(self):
        lock = self.root / 'tmp/magic_config.lock'
        lock.mkdir()
        self.invoke()
        self.assertEqual(self.attempts(), 0)
        lock.rmdir()
        self.wait_for('running')
        self.assertEqual(self.attempts(), 1)

    def test_diagnostic_is_read_only_even_when_lock_is_busy(self):
        lock = self.root / 'tmp/magic_config.lock'
        lock.mkdir()
        result = self.invoke('magic_config.sh', 'diagnose-boot')
        self.assertIn('core=stopped', result.stdout)
        self.assertEqual(self.attempts(), 0)
        self.assertTrue(lock.exists())
        self.assertTrue((self.root / 'koolshare/magic/.autostart-enabled').exists())

    def test_stop_cancels_pending_boot_retry(self):
        (self.root / 'failures').write_text('999')
        self.invoke()
        self.invoke('magic_config.sh', 'stop')
        time.sleep(0.2)
        before = self.attempts()
        time.sleep(0.3)
        self.assertEqual(self.attempts(), before)
        self.assertFalse((self.root / 'running').exists())
        self.assertEqual((self.root / 'dbus/magic_enable').read_text(), '0')


if __name__ == '__main__':
    unittest.main(verbosity=2)
