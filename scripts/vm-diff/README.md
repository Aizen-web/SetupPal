# Clean-VM install / Rewind diffs

These scripts capture and compare machine state around SetupPal installs. They are **read-only** unless *you* install or Rewind packages in the app.

Do **not** run destructive install/Rewind tests on a daily-driver Windows account. Use a clean Windows 11 VM snapshot.

## Capture

```powershell
powershell -File scripts/vm-diff/snapshot.ps1 -OutFile before.json
# …install and Rewind through SetupPal in the VM…
powershell -File scripts/vm-diff/snapshot.ps1 -OutFile after.json
powershell -File scripts/vm-diff/compare.ps1 -Before before.json -After after.json
```

Exit code `0` means the after snapshot matches the before snapshot for:

- `winget list`
- user and machine `PATH`
- `%LOCALAPPDATA%\Programs`, Program Files, Start Menu folders
- `%USERPROFILE%\.venvs` and `%USERPROFILE%\.setuppal`
- no leftover `winget` / `msiexec` / `unins*` processes

## Suggested matrix

On a clean VM, install then Rewind each of: OBS, VLC, LMMS, Audacity, Python 3.12 (venv recipe), ripgrep, and a TypeScript npm package in a chosen folder. Diff after every Rewind.

On a second VM that already has Git, Node, and Python 3.13, ask for each of those plus “web dev setup” and “everyday apps”. Nothing that was already installed should be reinstalled; Rewinding SetupPal’s own installs must leave the preinstalled tools alone.
