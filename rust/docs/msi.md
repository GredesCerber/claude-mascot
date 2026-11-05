# MSI installer

The MSI is a per-user Windows Installer package. It installs into
`%LOCALAPPDATA%\claude-mascot-rust`, creates Start Menu and Desktop shortcuts,
and registers normally in Apps & Features / Programs and Features. It does not
request administrator access or enable sign-in autostart. The settings file
may share the install directory, but it is not an MSI component and is
preserved during upgrade and uninstall. Windows Installer removes the install
directory only if it is empty.

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
route updates to the release page so an in-app updater cannot replace files
owned by Windows Installer. The mascot icon is embedded in the MSI and used
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
