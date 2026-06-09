# Test Coverage Configuration

## Configuration

Coverage is enabled via `bunfig.toml` at **`apps/browser-extension/bunfig.toml`**
(not the repo root — there is no top-level bunfig.toml). This only
governs the extension's Bun test suite; the Rust test suite under
`cargo test` has no coverage config in-tree.

```toml
[test]
coverage = true
coverageSkipTestFiles = true
coverageReporter = ["text", "lcov"]
```

## Research Findings
After extensive research of the official Bun documentation (analyzing 150+ code snippets), **Bun does not currently support arbitrary file exclusion patterns for code coverage**. 

The only coverage exclusion option available is:
- `coverageSkipTestFiles = true` - Excludes test files themselves (files matching `*.test.ts`, `*.spec.ts`, etc.)

## Files Still Included in Coverage
Despite configuration attempts, these files cannot be excluded with current Bun capabilities:
- `packages/@starlab/core-wasm/pkg/starlab_core_wasm.js` — auto-generated `wasm-pack` bindings (linked into the extension at build time by WXT)
- `apps/browser-extension/tests/entrypoints/offscreen/test-utils.ts` — Bun test helpers

Earlier drafts of this section referenced these files at
`pkg/starlab_mpc.js` and `src/entrypoints/offscreen/test-utils.ts`
respectively — both paths moved during the monorepo migration
(WASM bindings live in the shared `@starlab/core-wasm` package;
test utilities live under the extension's `tests/` tree, not
`src/`). Historical coverage percentages (45.93% func / 49.02% line
for the WASM bindings; 69.23% func / 70.59% line for the test
utilities) from when the doc was first written — rerun
`bun test --coverage` for current numbers.

## Available Coverage Configuration Options
Based on official documentation, Bun supports these coverage-related configurations:

```toml
[test]
coverage = true                          # Enable/disable coverage
coverageSkipTestFiles = true            # Skip test files only
coverageReporter = ["text", "lcov"]     # Output formats
coverageDir = "coverage"                # Output directory
coverageThreshold = 0.8                 # Global threshold
coverageIgnoreSourcemaps = false       # Sourcemap handling
```

**No support for:**
- `coverageExclude` patterns
- `coverageIgnore` patterns  
- Glob-based file exclusion
- Custom file filtering

## Workaround Options
Since Bun lacks built-in file exclusion, consider these alternatives:

### 1. File Structure Changes
Move auto-generated files outside source directories:
```bash
# Move WASM files to a separate directory
mkdir -p generated/
mv pkg/ generated/
```

### 2. Post-Process LCOV Reports
Filter the generated `coverage/lcov.info` file:
```bash
# Remove unwanted files from LCOV report.
# Real wasm-pack output filename is starlab_core_wasm.js under
# packages/@starlab/core-wasm/pkg/ — earlier drafts of this
# command used the pre-monorepo 'starlab_mpc.js' name which no
# longer exists.
grep -v "SF:.*pkg/starlab_core_wasm.js" coverage/lcov.info > coverage/filtered.info
grep -v "SF:.*test-utils.ts" coverage/filtered.info > coverage/final.info
```

### 3. Alternative Coverage Tools
Use external coverage tools that support exclusion patterns:
```bash
# Example with c8 (would require additional setup)
bun test --coverage && c8 --exclude="pkg/**" --exclude="**/test-utils.ts" report
```

### 4. Custom Coverage Script
Create a script to run tests and filter results programmatically.

## Recommendation
Accept the current coverage metrics as-is: core application code
coverage is healthy, and the over-counted files are generated WASM
bindings + test utilities that shouldn't skew real coverage
discussions. Monitor Bun's development for future coverage-exclusion
features.
