# Vendored demoparser

CS2 demo parser vendored from [LaihoE/demoparser](https://github.com/LaihoE/demoparser) (MIT, see `LICENSE`).

- Upstream commit: `314a253ed51488093b5f28f648694e42b561079f` (2026-10-29)
- Included: `src/parser` → `parser/`, `src/csgoproto` → `csgoproto/`

## Local changes

1. **No build scripts.** Upstream `build.rs` clones GameTracking-CS2 and runs `protoc` at build time. The generated files (`csgoproto/src/protobuf.rs`, `maps.rs`, `message_type.rs`, `parser/src/maps.rs`) are already committed upstream, so we drop the scripts and build offline.
2. **Recoil field names.** CS2 moved aim punch into `CCSPlayer_AimPunchServices`. The friendly names `aim_punch_angle` / `aim_punch_angle_vel` now map to `m_predictableBaseAngle` / `m_predictableBaseAngleVel`.
3. Removed the test-only module (`e2e_test.rs`), the benchmark binary, and the csgoproto code generator (`main.rs`, `parser.rs`).
4. `Cargo.toml`: removed profiles and Cargo.lock, inlined the csgoproto path dependency, and silenced warnings.

## Updating

Copy the two `src/` trees from a newer upstream commit, then re-apply the changes above. Update the commit hash here. Run `cargo test -p demo-parse` against the sample demos before committing.
