# Contributing to MPC Wallet

Thank you for your interest in contributing to MPC Wallet! We welcome contributions from the community and are grateful for any help you can provide.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
- [Development Process](#development-process)
- [Coding Standards](#coding-standards)
- [Testing Requirements](#testing-requirements)
- [Documentation](#documentation)
- [Security](#security)
- [Community](#community)

## Code of Conduct

### Our Pledge

We are committed to providing a friendly, safe, and welcoming environment for all contributors, regardless of experience level, gender identity and expression, sexual orientation, disability, personal appearance, body size, race, ethnicity, age, religion, nationality, or any other characteristic.

### Expected Behavior

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Accept feedback gracefully
- Prioritize the community's best interests

### Unacceptable Behavior

- Harassment, discrimination, or offensive comments
- Personal attacks or trolling
- Publishing private information without consent
- Unethical or unprofessional conduct

## Getting Started

### Prerequisites

Before contributing, ensure you have:

1. **Development Environment**
   ```bash
   # Rust toolchain — 1.85+ required (edition 2024)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add wasm32-unknown-unknown

   # Bun runtime — this is a Bun workspace, not npm/yarn/Node
   curl -fsSL https://bun.sh/install | bash
   ```

   `wasm-pack` is a devDependency of
   `packages/@mpc-wallet/core-wasm/package.json`, so `bun install`
   pulls it in — no separate `cargo install wasm-pack` needed.
   `cargo-watch` isn't used by anything in this repo; earlier
   drafts of this doc suggested installing it, but nothing in the
   workspace exercises it.

2. **Fork and Clone**
   ```bash
   # Fork the repository on GitHub (https://github.com/hecoinfo/mpc-wallet)
   # Then clone your fork:
   git clone https://github.com/YOUR_USERNAME/mpc-wallet.git
   cd mpc-wallet

   # Add upstream remote pointing at the canonical repo
   git remote add upstream https://github.com/hecoinfo/mpc-wallet.git
   ```

3. **Install Dependencies**
   ```bash
   # Install all dependencies
   bun install
   cargo build --workspace
   ```

## How to Contribute

### Types of Contributions

#### 1. Bug Reports
- Search existing issues first
- Use the bug report template
- Include reproduction steps
- Provide system information
- Attach relevant logs

#### 2. Feature Requests
- Check the roadmap and existing requests
- Use the feature request template
- Explain the use case
- Propose implementation approach
- Consider backward compatibility

#### 3. Code Contributions
- Fix bugs from the issue tracker
- Implement approved features
- Improve performance
- Enhance documentation
- Add tests

#### 4. Documentation
- Fix typos and errors
- Improve clarity
- Add examples
- Translate documentation
- Create tutorials

### Contribution Process

1. **Find or Create an Issue**
   ```bash
   # Check existing issues
   # If none exists, create one describing your contribution
   ```

2. **Create a Branch**
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/issue-number-description
   ```

3. **Make Changes**
   - Write clean, documented code
   - Follow coding standards
   - Add/update tests
   - Update documentation

4. **Test Your Changes**
   ```bash
   # Run all workspace tests (Rust + Bun) from repo root
   ./scripts/test-all.sh

   # Or narrow to a specific crate / extension
   cargo test -p tui-node
   cd apps/browser-extension && bun test
   ```

5. **Commit Your Changes**
   ```bash
   # Use conventional commits
   git commit -m "feat(component): add new feature"
   git commit -m "fix(component): resolve issue #123"
   git commit -m "docs: update README"
   ```

6. **Push and Create PR**
   ```bash
   git push origin feature/your-feature-name
   # Create PR on GitHub
   ```

## Development Process

### Branching Strategy

```
main
 ├── feature/new-feature
 ├── fix/bug-description
 └── docs/documentation-update
```

No release branches — the repo has no tagged releases yet
(`git tag -l` is empty; all crates are at 0.1.x). Work lands
on `main`.

### Commit Message Format

```
type(scope): subject

[optional body]

[optional footer(s)]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Test additions or changes
- `chore`: Build process or auxiliary tool changes
- `perf`: Performance improvements

**Examples:**
```bash
feat(wallet): add multi-signature support
fix(webrtc): resolve connection timeout issue
docs(api): update WebSocket protocol documentation
test(frost): add DKG edge case tests
```

### Pull Request Guidelines

#### PR Title
Follow the commit message format for PR titles

#### PR Description Template
```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] Breaking changes documented
```

### Code Review Process

There is currently no CI/CD pipeline configured in this repo
(no `.github/workflows/` directory). Reviewers will run the test
suite locally before merging:

```bash
./scripts/test-all.sh       # full workspace (Rust + Bun)
cargo clippy --workspace    # lint
bun run check               # svelte-check (from apps/browser-extension/)
```

#### What reviewers check

1. **Test pass locally**
   - `cargo test --workspace` clean
   - `bun test` clean under `apps/browser-extension/`
   - No new `cargo clippy` warnings

2. **Manual Review**
   - Code quality and design
   - Security considerations (anything keystore / FROST /
     signing-related gets extra attention)
   - Performance impact
   - Documentation keeps up with code (don't claim features that
     aren't there — this doc tree has been bitten repeatedly by
     drift, see `git log` for the April 2026 doc-accuracy pass)

3. **Approval Requirements**
   - At least 1 approving review
   - All conversations resolved
   - Up to date with main branch

Adding a GitHub Actions workflow that runs `./scripts/test-all.sh`
+ `cargo clippy --all-targets -D warnings` on every PR is open
work worth picking up.

## Coding Standards

### Rust Code

Follow the conventions visible in existing source (e.g.
`apps/tui-node/src/core/wallet_manager.rs`). Illustrative
snippet:

```rust
// Match the existing style: Arc<CoreState> + Arc<dyn UICallback>
// dependency injection, not struct-level HashMaps of wallets.
pub struct WalletManager {
    state: Arc<CoreState>,
    ui_callback: Arc<dyn UICallback>,
    keystore: Arc<Mutex<Option<Keystore>>>,
}

// Async-first — most core methods take &self and return
// CoreResult<T>, not sync Result<T>.
impl WalletManager {
    /// Create a new wallet entry for the active keystore.
    pub async fn create_wallet(
        &self,
        name: String,
        threshold: u16,
        participants: Vec<String>,
    ) -> CoreResult<WalletInfo> {
        // Implementation
    }
}

// Handle errors explicitly — prefer typed error enums over
// anyhow for API surface. Real error types in tui-node:
//   CoreError            (src/core/mod.rs:21)
//   KeystoreError        (src/keystore/mod.rs:24)
//   FrostKeystoreError   (src/keystore/frost_keystore.rs:19)
//   OfflineError         (src/offline/mod.rs:24)
// Plus upstream `FrostError` from packages/@mpc-wallet/frost-core
// which has `SigningError`/etc. variants.
match operation() {
    Ok(result) => process(result),
    Err(e) => {
        error!("Operation failed: {}", e);
        return Err(e.into());
    }
}
```

### TypeScript Code

```typescript
// Use TypeScript strict mode
// Define interfaces for data structures
interface WalletConfig {
  name: string;
  threshold: number;
  participants: number;
  blockchain: 'ethereum' | 'solana';
}

// Use async/await over callbacks
async function createWallet(config: WalletConfig): Promise<Wallet> {
  try {
    const wallet = await initializeWallet(config);
    return wallet;
  } catch (error) {
    logger.error('Wallet creation failed:', error);
    throw error;
  }
}

// Prefer functional programming
const activeWallets = wallets
  .filter(w => w.isActive)
  .map(w => w.id);
```

### General Guidelines

- **DRY** (Don't Repeat Yourself)
- **KISS** (Keep It Simple, Stupid)
- **YAGNI** (You Aren't Gonna Need It)
- Write self-documenting code
- Prefer composition over inheritance
- Keep functions small and focused
- Use meaningful variable names

## Testing Requirements

### Test Coverage

- No hard coverage floor is enforced in CI (there's no CI today,
  see the Code Review Process section). The existing bar is
  "tests land alongside the code they cover":
  - New features include unit tests in the relevant module's
    `#[cfg(test)] mod tests` block + integration coverage where
    appropriate
  - Bug fixes include a regression test that would have caught
    the bug
- Earlier drafts of this section specified "Minimum 80% code
  coverage / Critical paths 100%" — not enforced anywhere. If
  you want coverage numbers, `bun test --coverage` works for
  the extension; Rust-side needs `cargo install cargo-tarpaulin`
  or equivalent (no tarpaulin config ships).

### Test Types

#### Unit Tests (Rust)

Follow existing patterns in
`apps/tui-node/tests/update_transitions.rs` (pure state-machine
transition assertions) + `component_rendering.rs` (ratatui
`TestBackend` snapshot tests). Example shape:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // UICallback has no public No-Op type in the workspace —
    // each test module writes a minimal stub. See the pattern at
    // `apps/tui-node/src/core/signing_manager.rs:184-...` for a
    // full NoopUi example with every method stubbed out.
    struct NoopUi;
    #[async_trait]
    impl UICallback for NoopUi {
        // ... stub every method (show_message, update_connection_status,
        // update_wallets, etc.); returning () or default values
    }

    #[tokio::test]
    async fn create_wallet_registers_address() {
        let state = Arc::new(CoreState::default());
        let cb = Arc::new(NoopUi);
        let mgr = WalletManager::new(state, cb);

        // Real signature: create_wallet(name: String, threshold: u16,
        //                               participants: Vec<String>)
        //                 -> CoreResult<WalletInfo>
        let info = mgr.create_wallet(
            "test".to_string(),
            2,
            vec!["alice".into(), "bob".into(), "charlie".into()],
        ).await.unwrap();
        assert_eq!(info.name, "test");
    }
}
```

Earlier drafts of this example used `NoOpUICallback` as if it were
a public no-op type; no such type exists. The real convention is
an inline `struct NoopUi; impl UICallback for NoopUi { ... }` per
test module (see `src/core/signing_manager.rs:184` for a reference
implementation). `NoOpUIProvider` at `src/elm/provider.rs:71` IS
real but implements the *different* `UIProvider` trait — don't
confuse them.

#### Integration Tests (TypeScript / Bun)

Tests live under `apps/browser-extension/tests/` and import from
`bun:test` — NOT vitest / jest (consistent with
`docs/testing/TESTING.md`). Example shape:

```typescript
import { describe, expect, test } from "bun:test";

describe("DKG Process", () => {
  test("completes DKG with 3 participants", async () => {
    const participants = await createParticipants(3);
    const result = await executeDKG(participants, 2);
    expect(result.success).toBe(true);
  });
});
```

### Running Tests

```bash
# All tests (Rust + TypeScript). Excludes mpc-wallet-native which
# pulls graphics deps unsuitable for headless runs.
./scripts/test-all.sh

# Rust tests only. Same --exclude mpc-wallet-native guard.
cargo test --workspace --lib --tests --exclude mpc-wallet-native

# TypeScript tests only (run from apps/browser-extension).
cd apps/browser-extension && bun test

# With coverage (requires `cargo install cargo-tarpaulin`).
cargo tarpaulin --workspace --exclude mpc-wallet-native --out Html
```

## Documentation

### Code Documentation

- Document all public APIs
- Include examples in documentation
- Explain complex algorithms
- Document assumptions and limitations

### Documentation Updates

When changing code:
1. Update inline documentation
2. Update README if needed
3. Update API documentation
4. Add migration guides for breaking changes

### Documentation Style

- Clear headings (`# / ## / ###`)
- Structured content with bullet lists + tables when the
  content fits a grid
- Code examples must compile against the current source —
  include function signatures as they appear in the code, not
  a hypothetical API. If the signature matters, link to
  `src/<path>.rs:<line>` so future readers can check the doc
  against the source at a glance.
- ASCII art or Mermaid diagrams for complex flows
- Be honest about what ships vs what's proposed. Recent history
  (see `git log` for the April 2026 doc-accuracy pass) found
  extensive drift between earlier doc claims and actual source;
  callouts like "NOT implemented" / "aspirational" / "earlier
  drafts claimed X" keep future readers oriented.

## Security

### Reporting Security Issues

**DO NOT** create public issues for security vulnerabilities.

Instead:
1. Open a private advisory via [GitHub Security Advisories](https://github.com/hecoinfo/mpc-wallet/security/advisories/new)
2. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Security Guidelines

- Never commit secrets or credentials
- Validate all inputs
- Use secure random number generation
- Follow cryptographic best practices
- Implement proper access controls
- Log security-relevant events

## Community

### Getting Help

- **GitHub Discussions**: Ask questions and share ideas
- **GitHub Issues**: Report bugs, request features

### Ways to Contribute

Beyond code:
- Answer questions in discussions
- Review pull requests
- Improve documentation
- Share the project
- Report bugs
- Suggest features
- Create tutorials
- Translate documentation

### Recognition

Contributors are recognized in:
- GitHub contributors page
- Commit history (commits use `Co-Authored-By:` trailers when
  collaborative)
- Release notes, once tagged releases start happening (no tags
  exist yet — `git tag -l` is empty; see `docs/CHANGELOG.md` for
  the current "no-release-yet" framing)

## License

By contributing to MPC Wallet, you agree that your contributions will be
licensed under the license of the crate/package you're modifying. The
workspace default is Apache-2.0 (see the workspace `Cargo.toml`);
individual crates under `packages/` and `apps/signal-server/` set their
own — check each crate's `Cargo.toml` before submitting.

## Questions?

If you have questions about contributing, please ask in
[GitHub Discussions](https://github.com/hecoinfo/mpc-wallet/discussions).

---

Thank you for contributing to MPC Wallet! Your efforts help make secure multi-party computation accessible to everyone.

**Happy Contributing!** 🚀