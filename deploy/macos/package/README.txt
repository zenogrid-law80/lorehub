LoreHub Runner for macOS arm64

1. Run: sudo ./install-runner.sh
2. Edit: /Library/Application Support/LoreHub Runner/runner.env
3. Put the configured JWT private key and JWKS files in that directory.
4. Start: sudo launchctl kickstart -k system/co.kr.zenogrid.lorehub.runner

The launch daemon starts at boot. Configuration and work data are retained by
uninstall-runner.sh so an upgrade or reinstall does not erase credentials.
