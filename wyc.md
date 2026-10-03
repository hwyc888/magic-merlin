# WYC Project Rules — magic-merlin

> **Scope:** These rules apply **only** to the `hwyc888/magic-merlin` repository/project.
> Do not reuse them for FaceSign, the teaching platform, other repositories, or unrelated conversations.

## 1. Project identity

- Repository: `hwyc888/magic-merlin`
- Primary branch: `main`
- Primary workspace/tooling for this project: **CodexMcp01**
- Do not switch to CodexMcp02 for this project unless the user explicitly asks.
- Do not run both Codex workspaces at the same time unless cross-workspace work is explicitly requested.

## 2. Coding and change discipline

- All code changes follow **karpathy-guidelines**.
- Prefer the smallest surgical change that solves the verified problem.
- Do not refactor unrelated code while fixing a specific issue.
- State important assumptions and uncertainty before making a risky change.
- Define concrete checks/tests and keep iterating until they pass.
- Preserve unrelated untracked/local files unless the user explicitly asks to remove them.
- Before destructive Git operations, verify the exact target commit/tag first.
- Do not silently keep code from later commits when the user asks for a complete rollback.

## 3. Router model isolation

The TUF-BE3600 V2 and RT-AX86U packages use **separate model-specific install behavior**.
Changes for one model must not automatically alter the other model.

### TUF-BE3600 V2

- Package architecture: ARMv7.
- Package profile: `TUF-BE3600-V2`.
- Dedicated package:
  `magic-TUF-BE3600-V2-koolcenter-armv7.tar.gz`
- Installer is model/architecture bound and must reject installation on incompatible models.
- Installs MagicTier `S97/N97/V97` init entries.
- TUF-specific installer logic may repair the KoolCenter/JFFS boot chain when required:
  - enable `jffs2_scripts=1`;
  - maintain `/jffs/scripts/services-start`;
  - maintain `/jffs/scripts/wan-start`;
  - maintain `/jffs/scripts/nat-start`.
- Existing user JFFS content must be preserved; only the required KoolCenter entries may be added/deduplicated.
- Boot handling must support:
  - `V*` invocation with **no argument**;
  - `start`;
  - `start_nat`.
- Boot retry/failure protection must not silently change the persisted enable state to disabled.
- If the current TUF boot/autostart behavior is proven stable on real hardware, do not rewrite it without evidence of a new fault.

### RT-AX86U

- Package architecture: ARM64.
- Package profile: `RT-AX86U`.
- Dedicated package:
  `magic-RT-AX86U-koolcenter-arm64.tar.gz`
- Installer is model/architecture bound and must reject installation on incompatible models.
- Installs MagicTier `S97/N97/V97` init entries.
- **Do not apply the TUF JFFS repair logic to AX86U.**
- The AX86U installer must not change:
  - `jffs2_scripts`;
  - `/jffs/scripts/services-start`;
  - `/jffs/scripts/wan-start`;
  - `/jffs/scripts/nat-start`.
- Preserve the existing AX86U KoolCenter/JFFS environment unless a separately verified AX86U issue requires a model-specific change.

## 4. Current stable recovery baseline

- Stable tag: `router-v1.2.7-stable`
- Stable commit:
  `003e781179554b42d0e5f8a508cd86a97918d88a`
- Stable release: `router-v1.2.7`
- Verified router build: GitHub Actions `#118`, run ID `37132569538`

### Stable package checksums

TUF-BE3600 V2:

`257c41ba983e99be28363f4880836ec0e8f1d63cbcd96da89c6ae87b5ab0ca59`

RT-AX86U:

`9a7f03370e37fa09c146bf91f7cd6118e5ca2506136b88ca42f5df7c7a599625`

### Stable tag policy

- Never move, overwrite, or delete `router-v1.2.7-stable` during ordinary development.
- When a later version becomes the new proven stable baseline, create a **new stable tag**; do not repoint the old one.
- `PROJECT_RECOVERY.md` contains the detailed recovery procedure and remains authoritative for rollback commands.

## 5. Recovery semantics

When the user says **“恢复路由器稳定版”**, **“恢复当前稳定版”**, **“恢复 router-v1.2.7-stable”**, or equivalent:

- Use `router-v1.2.7-stable` as the recovery target.
- Verify the tag target before doing anything destructive.
- Do not substitute a newer commit.
- Do not cherry-pick later code into the requested stable baseline unless the user explicitly asks.

When the user asks to **compile/test the stable version**:

- Build/test directly from `router-v1.2.7-stable` or its commit.
- Do **not** rewrite `main`.

When the user explicitly asks to **fully return main to the stable version**, **delete commits after the stable version**, or equivalent:

- Reset code/history to `router-v1.2.7-stable`.
- Verify the target first.
- Use a protected force update such as `--force-with-lease`.
- Do not preserve later branch code unless the user explicitly requests it.

## 6. GitHub Actions and package delivery

- Router builds must produce **two separate install packages**:
  - TUF-BE3600 V2 ARMv7;
  - RT-AX86U ARM64.
- Each package must contain the correct `package-profile` and correct architecture binary.
- The build must verify package contents before publishing.
- Direct release assets are required in addition to the Actions artifact so the user can download the `.tar.gz` installers directly.
- Router build retention policy:
  - keep the **current successful run + one previous successful run**;
  - remove older completed runs, including failed/cancelled runs;
  - never delete an active run.
- Do not remove or weaken the retention cleanup logic from router/release workflows without an explicit request.

## 7. Validation expectations

For router-plugin changes, validate as applicable:

- shell syntax;
- normal POSIX `sh`;
- BusyBox `ash`;
- router boot-flow tests;
- model-isolation/install-hook tests;
- package-profile/model/architecture checks;
- package contents;
- release asset publication;
- `git diff --check`.

CI success is not a substitute for real-router verification.
For hardware-only boot failures, collect boot diagnostics before changing startup logic again.

## 8. Real-router boot troubleshooting rule

If TUF-BE3600 V2 fails to autostart after a future change:

1. Do not blindly rewrite the boot logic.
2. First inspect the existing boot diagnostic output/logs.
3. Distinguish whether:
   - the KoolCenter `V*` hook did not run;
   - dbus/config was not ready;
   - the MagicTier core exited during boot;
   - the JFFS/KoolCenter top-level hook is missing;
   - the init symlink is missing.
4. Only modify the layer proven to be faulty.
5. Do not overwrite a user's complete `/jffs/scripts/*` file; preserve existing content.

## 9. Delivery rule

- After a requested code change, complete the relevant test/build/publish cycle when the user asked for a usable package.
- Report the final commit SHA and package/release result.
- For TUF work, provide the TUF ARMv7 package.
- For AX86U work, provide the AX86U ARM64 package.
- Do not present an Actions ZIP as if it were the router install package when a direct `.tar.gz` installer is expected.

## 10. Project-only instruction

This file is a persistent **repository-level rule document**.
It must only govern work performed inside this `magic-merlin` repository.

If a future conversation opens this project, read this file before making project-level development, rollback, packaging, or release decisions.
