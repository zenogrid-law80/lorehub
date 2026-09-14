LoreHub Runner - Windows x86_64

MSI package:
1. Run the MSI as an administrator. It installs LoreHub Runner and Lore CLI and
   registers the "LoreHub Runner" startup task.
2. Edit C:\ProgramData\LoreHub\runner.env and add the configured JWT files.
3. For Rust/MSVC server builds, run install-server-build-prerequisites.bat from
   an elevated Command Prompt, then keep the CARGO_HOME, RUSTUP_HOME, and PATH
   entries from runner.env.example in C:\ProgramData\LoreHub\runner.env.
4. Start: Start-ScheduledTask -TaskName "LoreHub Runner"
5. Check: Get-ScheduledTaskInfo -TaskName "LoreHub Runner"

Script package:
Run install-runner.ps1 from an administrator PowerShell session, edit the same
environment file, then start the scheduled task.

The MSI preserves C:\ProgramData\LoreHub during upgrades and uninstall so
configuration, credentials, Runner identity, and work data are not erased.
