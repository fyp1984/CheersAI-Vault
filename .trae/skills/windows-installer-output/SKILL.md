---
name: "windows-installer-output"
description: "Moves or archives Windows exe outputs into dist with concise windows-safe OS/CPU/version naming and hash verification. Invoke when renaming, moving, or documenting Windows executable outputs."
---

# Windows Installer Output

Use this skill when the task involves Windows desktop installer packaging, archive naming, post-build file movement, or installer checksum verification.

## Goals

1. Put the target Windows `.exe` into `dist/`.
2. Rename it with a concise Windows-safe file name that includes:
   - operating system family
   - CPU architecture
   - application version
3. Verify file integrity before and after the move with SHA256.
4. If the caller explicitly names a non-installer `.exe`, process that file directly instead of insisting on an NSIS bundle.

## Required Workflow

1. Ensure the target directory exists:
   - `dist/`
2. Locate the requested Windows executable:
   - prefer `src-tauri/target/release/bundle/nsis/*.exe` for installer packaging tasks
   - if the user explicitly points to a release exe such as `src-tauri/target/release/cheersai-vault.exe`, rename and move that file directly
3. Validate source integrity before moving:
   - file exists
   - file is non-empty
   - source SHA256 can be calculated
4. Move the installer into `dist/` only after integrity checks pass.
5. Recompute SHA256 after the move and require it to match the source hash.
6. Use a Windows-safe file name with normalized lowercase segments.
7. Keep the name short enough for broad Windows terminal compatibility; prefer generic labels like `windows` and `x64` over host-specific long strings.

## Naming Convention

Use this format:

`<product>-<os>-<cpu>-v<version>.exe`

Example:

`cheersai-vault-windows-x64-v0.1.40.exe`

## File Name Rules

1. Remove or replace characters that are illegal in Windows file names.
2. Normalize spaces and separators to `-` or `_`.
3. Keep the `.exe` extension.
4. The version segment must stay readable, for example `v0.1.40`.
5. Prefer generic compatibility-oriented segments such as `windows`, `x64`, `arm64`, or `x86`.

## Default Commands

For the standard build flow, use:

```bash
pnpm build:windows
```

For post-processing an already generated Windows exe, use:

```bash
pnpm archive:windows-installer
```

## Implementation Notes

1. The repository build flow is expected to enforce this via:
   - `scripts/build-windows-installer.mjs`
   - `scripts/finalize-windows-installer.mjs`
2. If packaging fails before the installer exists, report that clearly instead of fabricating an archived file.
3. If sandbox or OS permissions block `pnpm` or Tauri cache directories, report the blocked paths explicitly because bundling may fail even when the application binary was built successfully.
