# Runner release directory

Place standalone LoreHub runner executables here, then restart the coordinator.
The coordinator selects the highest semantic version for each OS and architecture.

Expected names:

- `lorehub-runner-linux-x86_64-v0.2.0`
- `lorehub-runner-macos-aarch64-v0.2.0`
- `lorehub-runner-windows-x86_64-v0.2.0.exe`

Use `deploy/publish-runner-release.sh` to copy a built executable with the correct
name and permissions. Release executables are deployment artifacts and should not
be committed to source control.
