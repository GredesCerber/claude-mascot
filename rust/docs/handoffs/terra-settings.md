# Settings integration handoff

- `Settings.frequencies` stores five 0..100 rates with backward-compatible defaults of 50. `Pet.frequencies` is assigned by the main loop; walking, exploration, cursor pursuit and speech use the rates. Automatic social interactions still require `social.frequency = settings.frequencies.social` after integrating the social worker. Default 50 retains the old scheduler. Speech uses its own random stream.
- `Settings.ui_perches` defaults false and the native preferences event persists it. Connect this to the UI-perches worker before exposing the option. Remove the obsolete page text saying UI elements are unsupported once connected.
- Company buttons add, remove the selected pet (one to eight), or return everyone to the current screen. New pets get distinct palette colors; existing per-ID colors and roster are saved. Names and named sets remain unimplemented.
- Native preferences events update the model and save settings. Changes are in `preferences.rs`, `settings.rs`, `main.rs`, `engine.rs`, `engine_exploration.rs`, and `dialogue.rs`. Engine overlaps the perches worker.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Full `cargo test` was running, with lib (120 passed, 1 ignored), binary (3 passed), and numerous integration suites including speech RNG passing. Run the full suite after integration and check the final result; a linker collision occurred only when two test invocations overlapped.
- No desktop automation or user config changes. User's pre-existing install/uninstall edits were not staged.

## Second changeset

Names are kept in `Settings.pet_names` keyed by ID; up to eight `CompanySet` records retain a name, roster IDs, colors and names. The Friends page provides rename, save/replace by name and load; loading rebuilds native hosts and makes the first loaded pet the tray owner. Empty names, control characters, names over 64 characters, duplicate IDs and invalid colors are rejected at load; sets are limited to eight. The Return All grid accounts for scale, available work-area width and number of pets. Three frequency regression tests cover independent exploration, disabled automatic cursor pursuit with manual chase, and speech RNG isolation. UI event and serialized set tests pass. The combo has no delete-set button: an existing set can be overwritten by saving under the same name. The ninth distinct set is not added; UI currently does not explain this limit.

## Company controls follow-up

The Friends page now deletes a selected company and reports save, replace, load, delete and rename results inline. Empty or invalid names and the eight-set limit have inline explanations; save with an existing name still replaces it. Load and Delete are disabled with no selection. The previous paragraph's deletion and silent-limit limitations are resolved. Tests cover limit, replacement, delete event, disabled state and persistence after deletion; `cargo clippy --all-targets -- -D warnings` passed.
