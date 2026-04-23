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
   # Rust toolchain
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add wasm32-unknown-unknown
   
   # Bun runtime
   curl -fsSL https://bun.sh/install | bash
   
   # Additional tools
   cargo install wasm-pack
   cargo install cargo-watch
   ```

2. **Fork and Clone**
   ```bash
   # Fork the repository on GitHub
   # Then clone your fork
   git clone https://github.com/YOUR_USERNAME/mpc-wallet.git
   cd mpc-wallet
   
   # Add upstream remote
   git remote add upstream https://github.com/original-org/mpc-wallet.git
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
   # Run all tests
   ./scripts/test-all.sh
   
   # Run specific tests
   cargo test -p tui-node
   bun test browser-extension
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
 ├── docs/documentation-update
 └── release/v2.1.0
```

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

1. **Automated Checks**
   - CI/CD pipeline must pass
   - Code coverage maintained
   - Linting checks pass

2. **Manual Review**
   - Code quality and design
   - Security considerations
   - Performance impact
   - Documentation completeness

3. **Approval Requirements**
   - At least 1 approving review
   - All conversations resolved
   - Up to date with main branch

## Coding Standards

### Rust Code

```rust
// Use descriptive names
pub struct WalletManager {
    wallets: HashMap<WalletId, Wallet>,
    active_wallet: Option<WalletId>,
}

// Document public APIs
/// Creates a new wallet with the specified parameters.
/// 
/// # Arguments
/// * `name` - The wallet name
/// * `threshold` - Minimum signatures required
/// * `participants` - Total number of participants
pub fn create_wallet(
    name: &str,
    threshold: u32,
    participants: u32,
) -> Result<Wallet> {
    // Implementation
}

// Handle errors explicitly
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

- Minimum 80% code coverage
- Critical paths require 100% coverage
- All new features must include tests
- Bug fixes must include regression tests

### Test Types

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = create_wallet("test", 2, 3).unwrap();
        assert_eq!(wallet.name, "test");
        assert_eq!(wallet.threshold, 2);
    }
}
```

#### Integration Tests
```typescript
describe('DKG Process', () => {
  it('should complete DKG with 3 participants', async () => {
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

```markdown
# Clear Headings

## Structured Content

### Code Examples
```rust
// Always include working examples
let wallet = create_wallet("example", 2, 3)?;
```

### Diagrams
Use ASCII art or Mermaid diagrams for complex concepts
```

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
- Release notes

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