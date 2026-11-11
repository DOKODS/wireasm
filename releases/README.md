# Releases

This folder holds locally built archives. Release archives are produced by
`.github/workflows/release.yml` when a tag like `v0.2.0` is pushed, and are
attached to the GitHub release.

Expected archive names:

| Archive | Platform |
|---|---|
| `wirestudio-0.2.0-aarch64-macos.tar.gz` | macOS on Apple Silicon |
| `wirestudio-0.2.0-x86_64-macos.tar.gz` | macOS on Intel |
| `wirestudio-0.2.0-x86_64-linux.tar.gz` | Linux x86-64 |
| `wirestudio-0.2.0-x86_64-windows.zip` | Windows x86-64 |

Every archive contains two executables:

- `wirestudio` (`.exe` on Windows) — the graph IDE.
- `wireasm` (`.exe` on Windows) — the command-line compiler.

The macOS builds are not code-signed or notarized. On first launch, use
Finder → right click → Open, or run:

```bash
xattr -dr com.apple.quarantine wirestudio
```

Nothing in this folder is committed except this README.
