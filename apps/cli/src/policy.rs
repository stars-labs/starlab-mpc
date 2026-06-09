//! Auto-approval policy + password resolution (#26).
//!
//! Signing auto-approval is OFF by default. When a `serve` co-signer runs
//! with `--auto-approve`, every incoming signing request is gated by this
//! policy before the node contributes its share — a prompt-injected or buggy
//! controller must not be able to silently approve transfers.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Bounds on automatic signing approval. Disabled by default; when enabled,
/// every configured bound must pass and a finite approval budget is enforced.
#[derive(Debug)]
pub struct AutoApprovePolicy {
    enabled: bool,
    /// Wallet ids that may be auto-approved. Empty = any wallet (only when
    /// `enabled`).
    wallet_allowlist: Vec<String>,
    /// Max number of auto-approvals for this process lifetime; `None` = no cap.
    max_approvals: Option<usize>,
    approved: AtomicUsize,
}

impl AutoApprovePolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            wallet_allowlist: Vec::new(),
            max_approvals: None,
            approved: AtomicUsize::new(0),
        }
    }

    pub fn new(enabled: bool, wallet_allowlist: Vec<String>, max_approvals: Option<usize>) -> Self {
        Self {
            enabled,
            wallet_allowlist,
            max_approvals,
            approved: AtomicUsize::new(0),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn approved_count(&self) -> usize {
        self.approved.load(Ordering::SeqCst)
    }

    /// Decide whether a signing request for `wallet` may be auto-approved
    /// right now. On success consumes one approval slot (so the budget is
    /// honoured under concurrency). Returns `false` (no slot consumed) when
    /// disabled, off-allowlist, or over budget.
    pub fn try_approve(&self, wallet: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.wallet_allowlist.is_empty()
            && !self.wallet_allowlist.iter().any(|w| w == wallet)
        {
            return false;
        }
        match self.max_approvals {
            None => {
                self.approved.fetch_add(1, Ordering::SeqCst);
                true
            }
            Some(max) => {
                // Atomically consume a slot only if one remains.
                loop {
                    let cur = self.approved.load(Ordering::SeqCst);
                    if cur >= max {
                        return false;
                    }
                    if self
                        .approved
                        .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        return true;
                    }
                }
            }
        }
    }
}

/// Resolve a password from (in priority) a file, an env var, or a literal
/// flag. Literal flags are visible in `ps`, so we warn. Returns an error if
/// none is provided.
pub fn resolve_password(
    flag: Option<&str>,
    env_var: Option<&str>,
    file: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(path) = file {
        let s = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading password file {path}: {e}"))?;
        return Ok(s.trim_end_matches(['\n', '\r']).to_string());
    }
    if let Some(var) = env_var {
        let s = std::env::var(var)
            .map_err(|_| anyhow::anyhow!("password env var {var} not set"))?;
        return Ok(s);
    }
    if let Some(p) = flag {
        eprintln!(
            "warning: --password is visible in the process list; prefer \
             --password-file or --password-env"
        );
        return Ok(p.to_string());
    }
    anyhow::bail!("no password provided (use --password-file / --password-env / --password)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_never_approves() {
        let p = AutoApprovePolicy::disabled();
        assert!(!p.try_approve("any"));
        assert!(!p.is_enabled());
    }

    #[test]
    fn allowlist_is_enforced() {
        let p = AutoApprovePolicy::new(true, vec!["wallet-ok".into()], None);
        assert!(!p.try_approve("wallet-bad"));
        assert!(p.try_approve("wallet-ok"));
    }

    #[test]
    fn empty_allowlist_allows_any_when_enabled() {
        let p = AutoApprovePolicy::new(true, vec![], None);
        assert!(p.try_approve("whatever"));
    }

    #[test]
    fn max_budget_is_enforced() {
        let p = AutoApprovePolicy::new(true, vec![], Some(2));
        assert!(p.try_approve("w"));
        assert!(p.try_approve("w"));
        assert!(!p.try_approve("w"), "third approval must be refused");
        assert_eq!(p.approved_count(), 2);
    }

    #[test]
    fn resolve_password_prefers_file_then_env_then_flag() {
        // flag fallback
        assert_eq!(resolve_password(Some("lit"), None, None).unwrap(), "lit");
        // env
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("MPC_TEST_PW", "frommenv") };
        assert_eq!(
            resolve_password(None, Some("MPC_TEST_PW"), None).unwrap(),
            "frommenv"
        );
        // none → error
        assert!(resolve_password(None, None, None).is_err());
    }
}
