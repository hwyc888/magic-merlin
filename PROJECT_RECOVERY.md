# MagicTier Router Stable Recovery Baseline

This file is project-scoped. It applies only to the `hwyc888/magic-merlin` project/repository.

## Current stable baseline

- Stable tag: `router-v1.2.7-stable`
- Stable commit: `003e781179554b42d0e5f8a508cd86a97918d88a`
- Router release: `router-v1.2.7`
- Verified GitHub Actions run: `#118` / run ID `37132569538`
- Stable tag policy: do not move or overwrite this tag. Create a new stable tag for a future proven baseline.

## What this baseline means

### TUF-BE3600 V2

- Dedicated ARMv7 package.
- Package profile: `TUF-BE3600-V2`.
- Installer is bound to the TUF-BE3600 V2 model family and ARMv7 architecture.
- Installs MagicTier `S97/N97/V97` init entries.
- Repairs/enables the KoolCenter/JFFS boot chain when needed:
  - `jffs2_scripts=1`
  - `/jffs/scripts/services-start`
  - `/jffs/scripts/wan-start`
  - `/jffs/scripts/nat-start`
- Existing JFFS user script content is preserved.
- Boot flow accepts the KoolCenter no-argument `V*` event plus `start` and `start_nat` recovery events.
- Boot retry logic preserves the persistent enable state.

Stable package:
`magic-TUF-BE3600-V2-koolcenter-armv7.tar.gz`

SHA256:
`257c41ba983e99be28363f4880836ec0e8f1d63cbcd96da89c6ae87b5ab0ca59`

### RT-AX86U

- Dedicated ARM64 package.
- Package profile: `RT-AX86U`.
- Installer is bound to RT-AX86U and ARM64 architecture.
- Installs MagicTier `S97/N97/V97` init entries.
- Does NOT change `jffs2_scripts`.
- Does NOT modify:
  - `/jffs/scripts/services-start`
  - `/jffs/scripts/wan-start`
  - `/jffs/scripts/nat-start`
- TUF-specific JFFS repair logic must not be applied to AX86U.

Stable package:
`magic-RT-AX86U-koolcenter-arm64.tar.gz`

SHA256:
`9a7f03370e37fa09c146bf91f7cd6118e5ca2506136b88ca42f5df7c7a599625`

## Project recovery rules

For future conversations/work on this repository:

1. If the user says **"恢复路由器稳定版"**, **"恢复当前稳定版"**, **"恢复 router-v1.2.7-stable"**, or equivalent:
   - Treat `router-v1.2.7-stable` as the recovery target.
   - First inspect/verify the tag and target commit.
   - Do not substitute a newer branch or cherry-pick later code into the stable baseline.

2. If the user asks to **compile/test the stable version**:
   - Build directly from `router-v1.2.7-stable` (or its commit).
   - Do not rewrite `main`.

3. If the user explicitly asks to **make main fully return to the stable version**, **delete later commits**, or equivalent:
   - Restore code and history to `router-v1.2.7-stable`.
   - Use a protected force update such as `--force-with-lease`, after verifying the requested target.
   - Do not retain later branch code unless the user explicitly requests it.

4. Do not delete or move the stable tag during ordinary development.

5. This recovery rule is ONLY for this project/repository. Do not apply it to FaceSign, teaching platform, or any other project.

## Quick verification

```sh
git fetch origin --tags
git rev-parse router-v1.2.7-stable^{}
# expected:
# 003e781179554b42d0e5f8a508cd86a97918d88a
```

For a non-destructive stable build/test:

```sh
git switch --detach router-v1.2.7-stable
```

For a destructive main rollback, only after the user explicitly requests that main/history be restored:

```sh
git switch main
git reset --hard router-v1.2.7-stable
git push --force-with-lease origin main
```
