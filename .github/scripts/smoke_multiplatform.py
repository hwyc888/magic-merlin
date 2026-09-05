#!/usr/bin/env python3
"""Validate release binaries and ensure size optimization keeps the CLI surface.

Native runs also exercise RPC against a disposable no-TUN, loopback-only core.
QEMU runs validate executable startup and command parity, not hardware networking.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import socket
import subprocess
import tempfile
import time

COMMANDS = (
    "peer", "connector", "mapped-listener", "stun", "route", "peer-center",
    "vpn-portal", "node", "service", "proxy", "acl", "port-forward",
    "whitelist", "stats", "logger", "gen-autocomplete",
)


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--runner", default="")
    args = parser.parse_args()
    core, cli, baseline = (p.resolve() for p in (args.core, args.cli, args.baseline))
    runner = [args.runner] if args.runner else []
    env = dict(os.environ, LANG="en_US.UTF-8", LC_ALL="en_US.UTF-8", NO_COLOR="1")
    checks = []

    def run(binary, *options, expected=0, timeout=30):
        result = subprocess.run(
            runner + [str(binary)] + list(options), cwd=str(cli.parent),
            env=env, capture_output=True, text=True, encoding="utf-8",
            errors="replace", timeout=timeout,
        )
        if result.returncode != expected:
            raise RuntimeError(
                f"{binary.name} {' '.join(options)}: exit {result.returncode}\n"
                f"{result.stdout[-4000:]}\n{result.stderr[-4000:]}"
            )
        return result.stdout

    # Preserve this fork's existing custom core information flags.
    run(core, "--zzversion")
    run(core, "--zzhelp")
    checks.append("core_version_and_help")
    for options in [("--version",), ("--help",)] + [(cmd, "--help") for cmd in COMMANDS]:
        optimized = run(cli, *options)
        reference = run(baseline, *options)
        if optimized != reference:
            raise RuntimeError(f"CLI differs from standard release: {options}")
    for shell in ("bash", "powershell"):
        if run(cli, "gen-autocomplete", shell) != run(baseline, "gen-autocomplete", shell):
            raise RuntimeError(f"CLI completion differs: {shell}")
    run(cli, "--invalid-ci-option", expected=2)
    checks.extend(["all_16_command_help_matches_standard_release", "completion_parity", "invalid_argument_exit_code"])

    peak_rss = {}
    if not runner:
        rpc_port = free_port()
        portal = f"127.0.0.1:{rpc_port}"
        with tempfile.TemporaryDirectory(prefix="magictier-smoke-") as temp:
            log_path = Path(temp) / "core.log"
            with log_path.open("w", encoding="utf-8") as log:
                process = subprocess.Popen([
                    str(core), "--no-tun", "--no-listener", "--disable-ipv6",
                    "--rpc-portal", portal, "--network-name", "ci-smoke",
                    "--network-secret", "ci-only-not-a-production-secret",
                    "--stun-servers=", "--stun-servers-v6=",
                ], cwd=temp, env=env, stdout=log, stderr=subprocess.STDOUT)
                try:
                    deadline = time.monotonic() + 60
                    while True:
                        if process.poll() is not None:
                            raise RuntimeError(f"Test core exited: {process.returncode}")
                        try:
                            json.loads(run(cli, "-p", portal, "-o", "json", "peer", timeout=5))
                            break
                        except (RuntimeError, ValueError, subprocess.TimeoutExpired):
                            if time.monotonic() >= deadline:
                                raise RuntimeError("Test core RPC did not become ready")
                            time.sleep(1)

                    def rpc(*options, as_json=False):
                        output = run(cli, "-p", portal, "-o", "json", *options)
                        return json.loads(output) if as_json else output

                    for options in [
                        ("node",), ("peer",), ("route",), ("connector",),
                        ("mapped-listener",), ("proxy",), ("acl",),
                        ("port-forward",), ("whitelist",), ("stats",), ("logger",),
                    ]:
                        rpc(*options, as_json=True)
                        checks.append("rpc_" + options[0])

                    rpc("whitelist", "set-tcp", "80,443,8000-8002")
                    assert "443" in json.dumps(rpc("whitelist", as_json=True))
                    rpc("whitelist", "set-udp", "53,5353")
                    assert "5353" in json.dumps(rpc("whitelist", as_json=True))
                    rpc("whitelist", "clear-tcp")
                    rpc("whitelist", "clear-udp")
                    checks.append("whitelist_write_read_clear")

                    url = f"tcp://127.0.0.1:{free_port()}"
                    rpc("mapped-listener", "add", url)
                    assert url in json.dumps(rpc("mapped-listener", as_json=True))
                    rpc("mapped-listener", "remove", url)
                    assert url not in json.dumps(rpc("mapped-listener", as_json=True))
                    checks.append("mapped_listener_add_list_remove")

                    bind = f"127.0.0.1:{free_port()}"
                    rpc("port-forward", "add", "tcp", bind, "127.0.0.1:9")
                    cfgs = rpc("port-forward", as_json=True)["cfgs"]
                    assert any(rule["bind_addr"]["port"] == int(bind.rsplit(":", 1)[1]) for rule in cfgs)
                    rpc("port-forward", "remove", "tcp", bind, "127.0.0.1:9")
                    assert not rpc("port-forward", as_json=True)["cfgs"]
                    checks.append("port_forward_add_list_remove")
                    rpc("logger", "set", "warning")
                    rpc("logger", as_json=True)
                    checks.append("logger_set_get")

                    if platform.system() == "Linux" and Path("/usr/bin/time").is_file():
                        for name, options in {"help": ["--help"], "node_rpc": ["-p", portal, "node"]}.items():
                            measurement = Path(temp) / (name + ".rss")
                            subprocess.run([
                                "/usr/bin/time", "-f", "%M", "-o", str(measurement),
                                str(cli), *options,
                            ], cwd=temp, env=env, stdout=subprocess.DEVNULL,
                                stderr=subprocess.PIPE, check=True, timeout=30)
                            peak_rss[name + "_kib"] = int(measurement.read_text().strip())
                except Exception:
                    log.flush()
                    print(log_path.read_text(encoding="utf-8", errors="replace")[-8000:])
                    raise
                finally:
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=10)
    else:
        checks.append("qemu_startup_only_no_hardware_network_claim")

    def info(path):
        with path.open("rb") as file:
            digest = hashlib.file_digest(file, "sha256").hexdigest()
        return {"bytes": path.stat().st_size, "sha256": digest}

    report = {
        "commit": os.environ.get("GITHUB_SHA", "local"),
        "runner": args.runner or "native", "checks": checks,
        "features": "all original default features retained for core and CLI",
        "core": info(core), "cli": info(cli), "standard_cli": info(baseline),
        "cli_reduction_percent": round(100 * (1 - cli.stat().st_size / baseline.stat().st_size), 2),
        "peak_rss": peak_rss,
        "not_tested": ["real TUN device", "service installation", "cross-host NAT/RDP", "long-duration load"],
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
