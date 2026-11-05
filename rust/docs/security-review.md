# Security and build review

**Historical scope: original 0.1.0 executable.** The findings below describe
that sample, not the current updater-enabled application. Since 0.2.0, the
source includes WinHTTP release discovery/download and a child-process update
helper; downloaded assets require the expected GitHub route and SHA-256, and
download/restart requires user confirmation. Current local verification is in
[verification-0.3.0.md](verification-0.3.0.md).

The original sample was later also quarantined locally and submitted to
Microsoft. The early clean scans below remain historical observations; they
are not a final verdict. The Microsoft review's final result was not rechecked
in the 0.2.2 verification pass.

Review scope: Rust dependency graph, application source, Windows packaging and
install scripts, and the delivered executable identified in the supplied
release metadata. This is a code and artifact identity review, not an antivirus
verdict or a guarantee that every build is safe.

## Findings

- `cargo tree --locked` shows the declared image, font, serialization, and
  Windows API crates and their transitive dependencies. No HTTP client,
  downloader, socket library, updater, or packed-payload crate appears in the
  dependency graph.
- Source inspection found no general-purpose child-process creation, shell
  launch, socket, WinHTTP/WinINet, download, or update path. The executable
  embeds its built-in Tagilla PNG and JSON frame data. Optional user atlases are
  decoded as PNG and parsed as JSON data.
- The Windows code uses WinRT Global System Media Transport Controls to inspect
  current media sessions. It queries a process image path with limited query
  rights to associate a confirmed playing video session with a visible window.
  It does not infer video playback from window titles.
- The app can read, set, and delete its own named value under the current
  user's `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key when the
  user opts into autostart. Settings are written to a temporary file and
  replaced. `Install.ps1` copies the executable and creates shortcuts;
  `Uninstall.ps1` removes this Rust installation and its startup value. Neither
  script downloads or launches a separate payload.
- The delivered executable is unsigned. Its size and SHA-256 match the values
  in the adjacent `outputs/BUILD-INFO.json`. The existing
  `rust/target/release/claude-mascot.exe` also matched the delivered file's
  976,896-byte size and SHA-256 exactly:

  ```text
  72c201b282ca6e66d0e1dcfb1b06c1edb1e9ab81494fb12183e1bf8ba4442295
  ```

## Build comparison and antivirus reports

An additional release build was made from the locked manifest with Rust
1.99.1, host `x86_64-pc-windows-gnu`, and a separate
`CARGO_TARGET_DIR=rust/target/security-audit`. It completed successfully and
produced a 976,896-byte executable. The files differ in four bytes, all in
two PE header fields: bytes `[136, 138)` of the COFF timestamp DWORD at offset
136, and bytes `[216, 218)` of the PE checksum DWORD at offset 216. The original
timestamp is `1793804144`; the fresh build timestamp is `1793806409`.

```text
SHA-256 with only the timestamp and checksum DWORDs zeroed in memory:
de438937cee961b7cb61ffc69811eb396642dcab17ed9e1125280c02fb4de65c
```

The normalized hashes match, so the executable sections, code, and data match
the fresh locked source build; the full-file hash difference is explained by
the PE timestamp and checksum fields. This comparison only normalized those
two DWORDs in memory. Neither executable file was modified.

The original external report named `Trojan:Script/Wacatac.C!ml` for a copy
inside the archive/executable. A separate historical Defender `B!ml` event has
not been correlated to this file. An updated local Defender version
`1.463.122.0` completed scans of both the EXE and ZIP with zero detections.
These results are recorded as reported; they do not establish why another
Defender instance detected the file or justify a false-positive conclusion.

