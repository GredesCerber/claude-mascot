# MSI installer

The MSI is a per-user Windows Installer package. It installs into
`%LOCALAPPDATA%\claude-mascot-rust`, creates Start Menu and Desktop shortcuts,
and registers normally in Apps & Features / Programs and Features. Its Start Menu
folder includes an uninstall shortcut. The package does not request administrator
access or enable sign-in autostart. The settings file
may share the install directory, but it is not an MSI component and is
preserved during upgrade and uninstall. Windows Installer removes the install
directory only if it is empty.

Uninstall performs a best-effort cleanup of the application's named
`ClaudeMascotRust` startup entry when its command points to this installed EXE.
An entry pointing to a different copy is preserved. Major upgrades skip this
cleanup so an existing sign-in preference survives the upgrade.

The MSI is an independent community build, identified as `GredesCerber` in
Windows' installer listing. The repository's upstream source attribution is
maintained separately from the installer manufacturer field.

## Build

Build the x64 release executable with the repository's normal Rust build
workflow, then install the pinned WiX command line tool for the current user:

```powershell
dotnet tool install --global wix --version 6.0.2
.\Build.ps1
.\Build-Msi.ps1
```

The MSI is written as `dist\claude-mascot-<version>-x64.msi`. `Build-Msi.ps1`
uses the version in `Cargo.toml`. Pass `-ExecutablePath` to package a different
already-built executable or `-OutputDirectory` to choose another output folder.

The `UpgradeCode` in `installer\Package.wxs` is stable across releases. Increase
the Cargo version for each release so Windows Installer can perform a major
upgrade. Do not reuse a release version for distinct MSI builds.

The MSI installs `install-mode-msi.marker` alongside the executable. The
application uses this marker to recognize an MSI-managed installation and
check release metadata and offer the matching release page when a newer version
exists. It reports the current version when no newer release is found. The
in-app EXE updater cannot replace files owned by Windows Installer. The mascot icon is embedded in the MSI and used
for its Apps & Features entry and both shortcuts. Keep the marker name and
location aligned with the application's updater check.

## Package checks

The build script only creates the package; it does not install or launch it.
To inspect the resulting MSI without installing it, use WiX's database
decompiler and validator:

```powershell
wix msi decompile .\dist\claude-mascot-0.2.1-x64.msi -out .\dist\claude-mascot-inspected.wxs
wix msi validate .\dist\claude-mascot-0.2.1-x64.msi
```

Review the decompiled source for the per-user scope, fixed UpgradeCode,
executable and ownership marker, Start Menu/Desktop shortcuts, and absence of
an autostart entry before distributing a package.

The portable `Install.ps1` and `Uninstall.ps1` use the same default directory
for compatibility with existing portable installs. Both scripts check the
MSI marker and stop before changing an MSI-owned installation. Manage an MSI
install through Windows Settings or Apps & Features.

The portable `Install.ps1` copies its uninstaller into the install directory and
adds a Start Menu uninstall shortcut. Portable uninstall removes only its
executable, its installed uninstaller, and shortcuts that point to those files.
It removes the autostart entry only when it points to this install. Settings and
other files are preserved by default. `Uninstall.ps1 -RemoveSettings` also
removes `settings.json` and `last-error.log`; any other files in the directory
are preserved. Both portable scripts check the MSI marker and stop before
changing an MSI-owned installation. Manage an MSI install through Windows
Settings or Apps & Features.
