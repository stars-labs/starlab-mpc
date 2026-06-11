//! Integration tests for `elm::update::update`.
//!
//! These exercise the pure `(Model, Message) → (Model, Option<Command>)`
//! function — no network, no TTY, no async — so they run in milliseconds
//! and can be executed by `cargo test -p starlab-client --test update_transitions`.
//!
//! Scope: the DKG-phase state transitions added in commit 442549d. The smoke
//! test (`scripts/smoke-dkg.sh`) covers the same behaviour end-to-end in ~30s;
//! these run in under a second and catch regressions before the smoke test
//! is even worth booting.

use starlab_client::elm::command::Command;
use starlab_client::elm::message::{DKGRound, Message};
use starlab_client::elm::update::update;
use starlab_client::elm::{Model, Screen};

fn fresh_model() -> Model {
    Model::new("test-device".to_string())
}

/// A `Command::Batch` returned from `update` should contain `StartFrostProtocol`
/// somewhere among its children. We don't care about order / trailing
/// `ForceRemount` / future additions — just that the FROST trigger is present.
fn batch_contains_start_frost(cmd: &Option<Command>) -> bool {
    let Some(cmd) = cmd else { return false };
    match cmd {
        Command::StartFrostProtocol => true,
        Command::Batch(children) => children
            .iter()
            .any(|c| matches!(c, Command::StartFrostProtocol)),
        _ => false,
    }
}

// -----------------------------------------------------------------
// StartDKGProtocol — mesh is ready, we must enter Round 1
// -----------------------------------------------------------------
#[test]
fn start_dkg_protocol_enters_round1_and_flips_in_progress() {
    let mut model = fresh_model();
    assert_eq!(
        model.wallet_state.dkg_round,
        DKGRound::Initialization,
        "new model should start at Initialization"
    );
    assert!(
        !model.wallet_state.dkg_in_progress,
        "new model should not be running DKG"
    );

    let cmd = update(&mut model, Message::StartDKGProtocol);

    assert_eq!(
        model.wallet_state.dkg_round,
        DKGRound::Round1,
        "StartDKGProtocol should advance dkg_round to Round1"
    );
    assert!(
        model.wallet_state.dkg_in_progress,
        "StartDKGProtocol should set dkg_in_progress so subsequent triggers dedupe"
    );
    assert!(
        batch_contains_start_frost(&cmd),
        "StartDKGProtocol must dispatch Command::StartFrostProtocol (bare or inside Batch) — \
         this was the regression that made the UI sit on 'Initialization' forever"
    );
}

// -----------------------------------------------------------------
// ProcessDKGRound2 — first one flips us out of Round1 into Round2
// -----------------------------------------------------------------
#[test]
fn first_process_dkg_round2_advances_to_round2() {
    let mut model = fresh_model();
    // Simulate having passed through the Round 1 broadcast.
    model.wallet_state.dkg_round = DKGRound::Round1;
    model.wallet_state.dkg_in_progress = true;

    let _ = update(
        &mut model,
        Message::ProcessDKGRound2 {
            from_device: "peer-alice".to_string(),
            package_bytes: vec![0u8; 32], // content doesn't matter; update() doesn't deserialize
        },
    );

    assert_eq!(
        model.wallet_state.dkg_round,
        DKGRound::Round2,
        "first ProcessDKGRound2 while in Round1 should advance the UI label"
    );
}

#[test]
fn subsequent_process_dkg_round2_does_not_regress_round() {
    let mut model = fresh_model();
    model.wallet_state.dkg_round = DKGRound::Round2;

    let _ = update(
        &mut model,
        Message::ProcessDKGRound2 {
            from_device: "peer-bob".to_string(),
            package_bytes: vec![],
        },
    );

    assert_eq!(
        model.wallet_state.dkg_round,
        DKGRound::Round2,
        "ProcessDKGRound2 while already in Round2 must NOT regress or loop"
    );
}

#[test]
fn process_dkg_round2_does_not_override_finalization_or_complete() {
    // Unlikely timing (R2 packet arrives after part3 already ran locally) but
    // the guard must hold anyway — we should not rewind Complete back to Round2.
    for terminal in [DKGRound::Finalization, DKGRound::Complete] {
        let mut model = fresh_model();
        model.wallet_state.dkg_round = terminal.clone();
        let _ = update(
            &mut model,
            Message::ProcessDKGRound2 {
                from_device: "peer-late".to_string(),
                package_bytes: vec![],
            },
        );
        assert_eq!(
            model.wallet_state.dkg_round, terminal,
            "late ProcessDKGRound2 must not rewind {:?}",
            terminal
        );
    }
}

// -----------------------------------------------------------------
// DKGKeyGenerated — terminal state: Complete + clear in_progress + notify
// -----------------------------------------------------------------
#[test]
fn dkg_key_generated_transitions_to_complete() {
    let mut model = fresh_model();
    model.wallet_state.dkg_round = DKGRound::Round2;
    model.wallet_state.dkg_in_progress = true;

    let sample_hex =
        "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string();
    let before_notif_count = model.ui_state.notifications.len();

    let _ = update(
        &mut model,
        Message::DKGKeyGenerated {
            group_pubkey_hex: sample_hex.clone(),
        },
    );

    assert_eq!(
        model.wallet_state.dkg_round,
        DKGRound::Complete,
        "DKGKeyGenerated must land the UI on 100% Complete, not 95% Finalization \
         — the stuck-at-95% regression"
    );
    assert!(
        !model.wallet_state.dkg_in_progress,
        "DKGKeyGenerated must clear dkg_in_progress so a subsequent wallet flow can start"
    );

    assert_eq!(
        model.ui_state.notifications.len(),
        before_notif_count + 1,
        "DKGKeyGenerated should push exactly one success notification"
    );
    let notif = model.ui_state.notifications.last().unwrap();
    assert!(
        notif.text.contains(&sample_hex[..16]),
        "notification text should embed the 16-char group-key prefix; got {:?}",
        notif.text
    );
}

#[test]
fn dkg_key_generated_on_progress_screen_emits_force_remount() {
    // On the DKG Progress screen the component is rebuilt from Model on every
    // remount, so we need a ForceRemount to paint the new "Complete" state.
    let mut model = fresh_model();
    model.current_screen = Screen::DKGProgress {
        session_id: "dkg-test".to_string(),
    };
    let cmd = update(
        &mut model,
        Message::DKGKeyGenerated {
            group_pubkey_hex: "00".repeat(33),
        },
    );
    match cmd {
        Some(Command::SendMessage(Message::ForceRemount)) => {}
        other => panic!(
            "expected Some(Command::SendMessage(ForceRemount)) on DKGProgress, got {:?}",
            other
        ),
    }
}

#[test]
fn dkg_key_generated_off_progress_screen_does_not_force_remount() {
    // If the user already navigated away (shouldn't happen normally, but
    // guard anyway) we shouldn't send a stray remount.
    let mut model = fresh_model();
    model.current_screen = Screen::MainMenu;
    let cmd = update(
        &mut model,
        Message::DKGKeyGenerated {
            group_pubkey_hex: "00".repeat(33),
        },
    );
    assert!(
        cmd.is_none(),
        "DKGKeyGenerated off the progress screen should return None; got {:?}",
        cmd
    );
}

// -----------------------------------------------------------------
// SubmitPassword — Substep 1.2 stub contract
// -----------------------------------------------------------------
#[test]
fn fresh_model_has_no_pending_password() {
    let model = fresh_model();
    assert!(
        model.wallet_state.pending_password.is_none(),
        "pending_password must default to None — Stage 2 relies on this to \
         detect 'no password captured' vs 'user typed empty' states"
    );
}

/// Shared: put the model into the state ThresholdConfig-Enter leaves it in
/// before it routes to PasswordPrompt. The creating_wallet has a
/// custom_config so SubmitPassword can retrieve it.
fn creator_on_password_prompt() -> starlab_client::elm::Model {
    use starlab_client::elm::model::{CreateWalletState, WalletConfig, WalletMode};
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.creating_wallet = Some(CreateWalletState {
        mode: Some(WalletMode::Online),
        template: None,
        custom_config: Some(WalletConfig {
            name: "unit-test-wallet".to_string(),
            threshold: 2,
            total_participants: 3,
            mode: WalletMode::Online,
        }),
    });
    model
}

/// Shared: joiner-path setup — active_session is populated by the
/// AcceptSession click before PasswordPrompt is pushed.
fn joiner_on_password_prompt() -> starlab_client::elm::Model {
    use starlab_client::SessionInfo;
    use starlab_client::protocal::signal::SessionType;
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.active_session = Some(SessionInfo {
        session_id: "dkg-test-session".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".to_string(), "mpc-2".to_string(), "mpc-3".to_string()],
        session_type: SessionType::DKG,
        curve_type: "secp256k1".to_string(),
        coordination_type: "online".to_string(),
        signing_message_hex: None,
    });
    model
}

#[test]
fn creator_submit_password_stashes_value_and_dispatches_create_wallet() {
    use starlab_client::elm::command::Command;
    let mut model = creator_on_password_prompt();

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "hunter2-but-longer".to_string(),
        },
    );

    assert_eq!(
        model.wallet_state.pending_password.as_deref(),
        Some("hunter2-but-longer"),
        "SubmitPassword must stash the value for the Stage 2 finaliser to consume"
    );

    // Creator path hands off to CreateWallet via SendMessage, which then
    // navigates to DKGProgress and announces the session.
    match cmd {
        Some(Command::SendMessage(Message::CreateWallet { config })) => {
            assert_eq!(config.name, "unit-test-wallet");
            assert_eq!(config.threshold, 2);
            assert_eq!(config.total_participants, 3);
        }
        other => panic!(
            "creator SubmitPassword should dispatch CreateWallet, got {:?}",
            other
        ),
    }
}

#[test]
fn joiner_submit_password_stashes_value_and_dispatches_join_dkg() {
    use starlab_client::elm::command::Command;
    let mut model = joiner_on_password_prompt();

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "another-long-password".to_string(),
        },
    );

    assert_eq!(
        model.wallet_state.pending_password.as_deref(),
        Some("another-long-password"),
    );

    // Joiner dispatches Command::JoinDKG directly; navigation to
    // DKGProgress happens inside this handler (unlike the creator path,
    // where CreateWallet does the nav).
    match cmd {
        Some(Command::JoinDKG { session_id, .. }) => {
            assert_eq!(session_id, "dkg-test-session");
        }
        other => panic!(
            "joiner SubmitPassword should dispatch JoinDKG, got {:?}",
            other
        ),
    }
    assert!(
        matches!(model.current_screen, Screen::DKGProgress { .. }),
        "joiner SubmitPassword should push DKGProgress; got {:?}",
        model.current_screen
    );
}

#[test]
fn submit_password_with_neither_flow_configured_goes_home() {
    // Pathological case — shouldn't happen in practice (upstream edges
    // always populate one of the two), but we verify the defensive
    // fallback rather than hitting an unwrap.
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "someemergencypassword".to_string(),
        },
    );

    assert!(
        cmd.is_none(),
        "the pathological branch must not dispatch any Command"
    );
    assert!(
        matches!(model.current_screen, Screen::MainMenu),
        "go_home should land us on MainMenu so the user has a way out; got {:?}",
        model.current_screen
    );
}

// -----------------------------------------------------------------
// PasswordPrompt keystroke-level handlers (Substep 1.3 rework)
// -----------------------------------------------------------------
//
// These exercise the path where `app.rs::handle_key_event` routes
// printable chars / backspace / tab / enter through the
// `Password*` messages to mutate `Model.wallet_state`. The component
// only renders from that state; all logic lives here.
//
// Rationale for moving off the tuirealm `on()` API: the codebase's
// `handle_key_event` intercepts crossterm events directly and never
// delegates to per-component `on()`, so the previous substep's
// component-internal input handler was dead code.

#[test]
fn type_char_appends_to_password_field_when_focused() {
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    for c in "abcd".chars() {
        let cmd = update(&mut model, Message::PasswordTypeChar(c));
        assert!(cmd.is_none(), "typing should not produce a Command");
    }
    assert_eq!(model.wallet_state.password_draft, "abcd");
    assert_eq!(model.wallet_state.confirm_draft, "");
}

#[test]
fn toggle_field_routes_typing_to_confirm() {
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;

    update(&mut model, Message::PasswordTypeChar('a'));
    update(&mut model, Message::PasswordToggleField);
    update(&mut model, Message::PasswordTypeChar('z'));

    assert_eq!(model.wallet_state.password_draft, "a");
    assert_eq!(model.wallet_state.confirm_draft, "z");
    assert!(
        model.wallet_state.password_focus_confirm,
        "toggle must flip focus onto confirm"
    );
}

#[test]
fn toggle_field_is_idempotent_pair() {
    // Pair of toggles returns focus to password — the two-field form has
    // no distinction between next/prev, so Tab/BackTab are both just a
    // flip. This verifies the invariant.
    let mut model = fresh_model();
    update(&mut model, Message::PasswordToggleField);
    update(&mut model, Message::PasswordToggleField);
    assert!(!model.wallet_state.password_focus_confirm);
}

#[test]
fn backspace_pops_from_focused_field() {
    let mut model = fresh_model();
    for c in "abcd".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    update(&mut model, Message::PasswordBackspace);
    assert_eq!(model.wallet_state.password_draft, "abc");

    update(&mut model, Message::PasswordToggleField);
    update(&mut model, Message::PasswordTypeChar('X'));
    update(&mut model, Message::PasswordBackspace);
    assert_eq!(
        model.wallet_state.confirm_draft, "",
        "backspace after focus-flip must act on the confirm field, not password"
    );
    assert_eq!(
        model.wallet_state.password_draft, "abc",
        "backspace on empty confirm must not bleed back into password"
    );
}

#[test]
fn typing_clears_stale_error() {
    let mut model = fresh_model();
    model.wallet_state.password_error = Some("old error".to_string());
    update(&mut model, Message::PasswordTypeChar('x'));
    assert!(
        model.wallet_state.password_error.is_none(),
        "any typing should clear the previous validation error"
    );
}

#[test]
fn backspace_clears_stale_error() {
    // Symmetry with typing: editing should also wipe the complaint.
    let mut model = fresh_model();
    update(&mut model, Message::PasswordTypeChar('x'));
    model.wallet_state.password_error = Some("old".to_string());
    update(&mut model, Message::PasswordBackspace);
    assert!(model.wallet_state.password_error.is_none());
}

#[test]
fn submit_draft_short_password_sets_error_and_does_not_route() {
    let mut model = creator_on_password_prompt();
    for c in "short".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    let cmd = update(&mut model, Message::PasswordSubmitDraft);
    assert!(cmd.is_none(), "failed validation must not emit a Command");
    assert!(
        model
            .wallet_state
            .password_error
            .as_deref()
            .unwrap_or("")
            .contains("at least"),
        "error copy should mention the length requirement; got {:?}",
        model.wallet_state.password_error
    );
    // Drafts stay so the user can edit, not retype from scratch.
    assert_eq!(model.wallet_state.password_draft, "short");
}

#[test]
fn submit_draft_mismatched_confirm_sets_error() {
    let mut model = creator_on_password_prompt();
    for c in "longenoughpw".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    update(&mut model, Message::PasswordToggleField);
    for c in "different".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    let cmd = update(&mut model, Message::PasswordSubmitDraft);
    assert!(cmd.is_none());
    assert!(
        model
            .wallet_state
            .password_error
            .as_deref()
            .unwrap_or("")
            .contains("match"),
        "error should mention matching; got {:?}",
        model.wallet_state.password_error
    );
}

#[test]
fn submit_draft_valid_dispatches_submit_password_and_clears_drafts() {
    let mut model = creator_on_password_prompt();
    for c in "longenoughpw".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    update(&mut model, Message::PasswordToggleField);
    for c in "longenoughpw".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }

    let cmd = update(&mut model, Message::PasswordSubmitDraft);

    match cmd {
        Some(Command::SendMessage(Message::SubmitPassword { value })) => {
            assert_eq!(value, "longenoughpw");
        }
        other => panic!(
            "valid submit must dispatch SendMessage(SubmitPassword), got {:?}",
            other
        ),
    }

    // Drafts wiped immediately — cleartext must not outlive the handoff.
    assert_eq!(model.wallet_state.password_draft, "");
    assert_eq!(model.wallet_state.confirm_draft, "");
    assert!(model.wallet_state.password_error.is_none());
    assert!(
        !model.wallet_state.password_focus_confirm,
        "focus should reset to password for any next attempt"
    );
}

#[test]
fn navigate_back_from_password_prompt_wipes_draft() {
    // Security invariant: if the user bails out, the cleartext in the
    // draft buffers must not survive. The same rule applies to
    // NavigateHome / PopScreen (see below).
    let mut model = fresh_model();
    model.push_screen(Screen::PasswordPrompt);
    for c in "typedstuff".chars() {
        update(&mut model, Message::PasswordTypeChar(c));
    }
    assert_eq!(model.wallet_state.password_draft, "typedstuff");

    update(&mut model, Message::NavigateBack);

    assert_eq!(model.wallet_state.password_draft, "");
    assert_eq!(model.wallet_state.confirm_draft, "");
    assert!(model.wallet_state.password_error.is_none());
}

#[test]
fn navigate_home_from_password_prompt_wipes_draft() {
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    update(&mut model, Message::PasswordTypeChar('a'));
    update(&mut model, Message::NavigateHome);
    assert_eq!(model.wallet_state.password_draft, "");
}

// -----------------------------------------------------------------
// DKGFinalized — Stage 2 terminal handler
// -----------------------------------------------------------------

fn finalized_fixture_model() -> starlab_client::elm::Model {
    use starlab_client::elm::model::{CreateWalletState, WalletConfig, WalletMode};
    let mut model = fresh_model();
    model.wallet_state.dkg_in_progress = true;
    model.wallet_state.pending_password = Some("should-be-cleared".to_string());
    model.wallet_state.creating_wallet = Some(CreateWalletState {
        mode: Some(WalletMode::Online),
        template: None,
        custom_config: Some(WalletConfig {
            name: "finalized-test".to_string(),
            threshold: 2,
            total_participants: 3,
            mode: WalletMode::Online,
        }),
    });
    model.current_screen = Screen::DKGProgress {
        session_id: "dkg-done".to_string(),
    };
    model
}

fn sample_dkg_finalized_msg() -> Message {
    Message::DKGFinalized {
        wallet_id: "finalized-test".to_string(),
        group_pubkey_hex:
            "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
        curve_type: "secp256k1".to_string(),
        addresses: vec![
            ("ethereum".to_string(), "0xabc123".to_string()),
            ("bitcoin".to_string(), "bc1qxyz".to_string()),
        ],
    }
}

#[test]
fn dkg_finalized_clears_pending_password_and_creating_wallet() {
    let mut model = finalized_fixture_model();

    let _ = update(&mut model, sample_dkg_finalized_msg());

    assert!(
        model.wallet_state.pending_password.is_none(),
        "DKGFinalized must zero out pending_password — cleartext password \
         must not outlive the keystore write"
    );
    assert!(
        model.wallet_state.creating_wallet.is_none(),
        "DKGFinalized must clear creating_wallet — the flow is done"
    );
    assert!(
        !model.wallet_state.dkg_in_progress,
        "DKGFinalized must flip dkg_in_progress false so a next flow can start"
    );
}

#[test]
fn dkg_finalized_pushes_wallet_complete_and_dispatches_load_wallets() {
    // Stage 3: DKGFinalized lands on WalletComplete (not MainMenu — that
    // was the Stage-2 placeholder). LoadWallets still fires so the
    // MainMenu behind the WalletComplete screen has a refreshed count
    // ready for when the user presses Enter/Esc to dismiss.
    let mut model = finalized_fixture_model();

    let cmd = update(&mut model, sample_dkg_finalized_msg());

    match &model.current_screen {
        Screen::WalletComplete { wallet_id } => {
            assert_eq!(
                wallet_id, "finalized-test",
                "Screen variant must carry the right wallet_id"
            );
        }
        other => panic!("expected WalletComplete, got {:?}", other),
    }
    assert!(
        matches!(cmd, Some(Command::LoadWallets)),
        "DKGFinalized must dispatch LoadWallets so the wallet count \
         refreshes; got {:?}",
        cmd
    );
}

#[test]
fn dkg_finalized_back_navigation_lands_on_main_menu() {
    // The WalletComplete screen's Enter/Esc hints promise "Done" takes
    // the user home, not back to DKGProgress. The handler implements
    // this by resetting the stack before pushing WalletComplete, so a
    // single pop_screen lands on MainMenu. Pin that behaviour.
    use starlab_client::elm::message::Message;
    let mut model = finalized_fixture_model();
    let _ = update(&mut model, sample_dkg_finalized_msg());
    assert!(matches!(model.current_screen, Screen::WalletComplete { .. }));

    let _ = update(&mut model, Message::NavigateBack);
    assert!(
        matches!(model.current_screen, Screen::MainMenu),
        "NavigateBack from WalletComplete must land on MainMenu (not \
         DKGProgress or PasswordPrompt — those are stale); got {:?}",
        model.current_screen
    );
}

#[test]
fn dkg_finalized_stashes_completed_wallet_info() {
    // The WalletComplete component pulls its render data off
    // `wallet_state.last_finalized_wallet` via `set_from_model`. Make
    // sure DKGFinalized actually populates it — otherwise the screen
    // would render empty or fall back to its error diagnostic.
    let mut model = finalized_fixture_model();
    let _ = update(&mut model, sample_dkg_finalized_msg());

    let info = model
        .wallet_state
        .last_finalized_wallet
        .as_ref()
        .expect("DKGFinalized must stash a CompletedWalletInfo snapshot");
    assert_eq!(info.wallet_id, "finalized-test");
    assert_eq!(info.curve_type, "secp256k1");
    assert_eq!(
        info.addresses.len(),
        2,
        "addresses must be copied across verbatim (not truncated or \
         deduped); got {:?}",
        info.addresses
    );
    assert_eq!(
        info.addresses[0],
        ("ethereum".to_string(), "0xabc123".to_string())
    );
}

// -----------------------------------------------------------------
// Stage 5: curve_type sourced from Model (no more "unified")
// -----------------------------------------------------------------

#[test]
fn creator_create_wallet_announces_real_curve_from_model() {
    // The `Message::CreateWallet` handler stamps `curve_type` onto
    // `active_session` so joiners can see what we're running. Stage 5
    // moved that from the `"unified"` literal to
    // `model.wallet_state.curve_type`. Test-level fresh_model uses the
    // default empty-str curve type (there's no `C` wired through the
    // pure Model); set it explicitly and assert it propagates.
    use starlab_client::elm::message::Message;
    use starlab_client::elm::model::{WalletConfig, WalletMode};

    let mut model = fresh_model();
    model.wallet_state.curve_type = "secp256k1";

    let cfg = WalletConfig {
        name: "stage5-session".to_string(),
        threshold: 2,
        total_participants: 3,
        mode: WalletMode::Online,
    };
    let _ = update(&mut model, Message::CreateWallet { config: cfg });

    let session = model
        .active_session
        .as_ref()
        .expect("CreateWallet must populate active_session");
    assert_eq!(
        session.curve_type, "secp256k1",
        "active_session.curve_type must come from model.wallet_state.curve_type \
         — not the old 'unified' literal"
    );
}

// -----------------------------------------------------------------
// Phase C.3: SignTransaction screen input
// -----------------------------------------------------------------

#[test]
fn sign_type_char_appends_to_draft() {
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "wallet-test".to_string(),
    };
    for c in "hello".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    assert_eq!(model.wallet_state.sign_message_draft, "hello");
}

#[test]
fn sign_backspace_pops_from_draft() {
    let mut model = fresh_model();
    for c in "hi!".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    update(&mut model, Message::SignBackspace);
    assert_eq!(model.wallet_state.sign_message_draft, "hi");
}

#[test]
fn sign_submit_on_empty_draft_warns_and_returns_none() {
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "wallet-empty".to_string(),
    };
    let before = model.ui_state.notifications.len();
    let cmd = update(&mut model, Message::SignSubmit);
    assert!(cmd.is_none(), "empty-draft submit must not dispatch");
    assert_eq!(
        model.ui_state.notifications.len(),
        before + 1,
        "empty submit must push a warning notification"
    );
    // Draft stays so the user can edit the (empty) input — no clearing
    assert_eq!(model.wallet_state.sign_message_draft, "");
}

#[test]
fn sign_submit_dispatches_initiate_signing_and_clears_draft() {
    // Warm-session SignSubmit: the wallet IS unlocked. After Stage 3
    // (confirmation modal) SignSubmit no longer dispatches directly —
    // it stashes a preview and opens a Modal::Confirm whose on_confirm
    // is ConfirmSigningRequest. That second message is what actually
    // dispatches InitiateSigning. The cold-start path has its own test
    // (`sign_submit_when_wallet_not_unlocked_routes_to_password_prompt`).
    use starlab_client::elm::command::Command;
    use starlab_client::elm::model::Modal;
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "wallet-test".to_string(),
    };
    model.wallet_state.curve_type = "secp256k1";
    // "BIP-44 all the way": signing uses the account-0 CHILD of the root
    // id, so the warm marker carries the child id too.
    model.wallet_state.wallet_unlocked_id = Some("wallet-test-ethereum-0".to_string());
    for c in "Sign this".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }

    // Step 1: SignSubmit opens the confirmation modal; no Command
    // dispatched yet, draft intact for the preview.
    let cmd = update(&mut model, Message::SignSubmit);
    assert!(cmd.is_none(), "SignSubmit no longer dispatches directly — modal first");
    assert!(
        matches!(model.ui_state.modal, Some(Modal::Confirm { .. })),
        "SignSubmit must stage a confirm modal; got {:?}",
        model.ui_state.modal
    );
    let preview = model
        .wallet_state
        .pending_sign_preview
        .as_ref()
        .expect("preview must be stashed for ConfirmSigningRequest");
    assert!(preview.warm, "warm path must be flagged");
    assert_eq!(preview.wallet_id, "wallet-test-ethereum-0");
    assert_eq!(
        preview.bytes_to_sign,
        starlab_client::utils::eth_helper::eip191_hash(b"Sign this"),
    );

    // Step 2: ConfirmSigningRequest dispatches InitiateSigning with
    // the same payload the modal previewed.
    let cmd = update(&mut model, Message::ConfirmSigningRequest);
    match cmd {
        Some(Command::SendMessage(Message::InitiateSigning { request })) => {
            assert_eq!(request.wallet_id, "wallet-test-ethereum-0");
            assert_eq!(request.chain, "secp256k1");
            assert_eq!(
                request.transaction_data,
                starlab_client::utils::eth_helper::eip191_hash(b"Sign this"),
                "secp256k1 SignSubmit must hash with EIP-191 before FROST"
            );
            assert_eq!(
                request.raw_message.as_deref(),
                Some(b"Sign this".as_ref()),
                "raw_message must round-trip through the request so \
                 SignatureComplete can render the user's message"
            );
        }
        other => panic!(
            "ConfirmSigningRequest must dispatch SendMessage(InitiateSigning); got {:?}",
            other
        ),
    }
    // After confirm: draft cleared, preview drained, modal closed.
    assert_eq!(model.wallet_state.sign_message_draft, "");
    assert!(model.wallet_state.pending_sign_preview.is_none());
    assert!(model.ui_state.modal.is_none());
}

#[test]
fn initiate_signing_dispatches_start_signing_command() {
    // The bridge handler: Message::InitiateSigning → Command::StartSigning.
    // This is what the UI sees; protocal layer is a sibling problem.
    use starlab_client::elm::command::Command;
    use starlab_client::elm::message::SigningRequest;
    let mut model = fresh_model();
    let req = SigningRequest {
        wallet_id: "w".into(),
        transaction_data: b"xx".to_vec(),
        chain: "secp256k1".into(),
        metadata: None,
        raw_message: None,
    };
    let cmd = update(
        &mut model,
        Message::InitiateSigning {
            request: req.clone(),
        },
    );
    match cmd {
        Some(Command::StartSigning { request }) => {
            assert_eq!(request.wallet_id, req.wallet_id);
            assert_eq!(request.transaction_data, req.transaction_data);
        }
        other => panic!("expected Command::StartSigning; got {:?}", other),
    }
}

// -----------------------------------------------------------------
// Phase C.5: SigningComplete / SigningFailed terminal handlers
// -----------------------------------------------------------------

#[test]
fn signing_complete_stashes_snapshot_and_navigates_to_signature_complete() {
    use starlab_client::SessionInfo;
    use starlab_client::protocal::signal::SessionType;
    let mut model = fresh_model();
    // Simulate mid-signing state
    model.wallet_state.pending_sign_message = Some(b"hello world".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("wallet-test".to_string());
    model.wallet_state.pending_sign_session_id = Some("sign_abc".to_string());
    model.active_session = Some(SessionInfo {
        session_id: "sign_abc".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".into()],
        session_type: SessionType::Signing {
            wallet_name: "wallet-test".to_string(),
            curve_type: "secp256k1".to_string(),
            blockchain: "secp256k1".to_string(),
            group_public_key: "abcd".to_string(),
        },
        curve_type: "secp256k1".to_string(),
        coordination_type: "Network".to_string(),
        signing_message_hex: None,
    });
    model.current_screen = Screen::SigningProgress {
        request_id: "sign_abc".to_string(),
    };

    let signature_bytes = vec![0xAAu8; 64];
    let _ = update(
        &mut model,
        Message::SigningComplete {
            request_id: "inline".to_string(),
            message: b"hello world".to_vec(),
            signature: signature_bytes.clone(),
        },
    );

    let info = model
        .wallet_state
        .last_completed_signature
        .as_ref()
        .expect("SigningComplete must stash a CompletedSignatureInfo");
    assert_eq!(info.request_id, "inline");
    assert_eq!(info.wallet_id, "wallet-test");
    assert_eq!(info.message, b"hello world");
    assert_eq!(info.signature, signature_bytes);
    assert!(info.verified, "protocol layer gates emit on verify success");

    // Stack reset: we should be on SignatureComplete with a single
    // MainMenu frame behind it so NavigateBack drops us home.
    assert!(
        matches!(model.current_screen, Screen::SignatureComplete { .. }),
        "must land on SignatureComplete; got {:?}",
        model.current_screen
    );

    // All pending-sign state must be drained so a future sign starts clean.
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
    assert_eq!(model.wallet_state.sign_message_draft, "");
}

#[test]
fn signing_complete_back_navigation_lands_on_main_menu() {
    let mut model = fresh_model();
    let _ = update(
        &mut model,
        Message::SigningComplete {
            request_id: "x".into(),
            message: b"x".to_vec(),
            signature: vec![0u8; 64],
        },
    );
    assert!(matches!(model.current_screen, Screen::SignatureComplete { .. }));

    let _ = update(&mut model, Message::NavigateBack);
    assert!(
        matches!(model.current_screen, Screen::MainMenu),
        "NavigateBack from SignatureComplete must land on MainMenu (not \
         SigningProgress — that's stale); got {:?}",
        model.current_screen
    );
}

#[test]
fn signing_failed_surfaces_error_modal_and_clears_pending_state() {
    use starlab_client::elm::model::Modal;
    let mut model = fresh_model();
    model.wallet_state.pending_sign_message = Some(b"x".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("w".to_string());
    model.wallet_state.pending_sign_session_id = Some("s".to_string());

    let _ = update(
        &mut model,
        Message::SigningFailed {
            request_id: "x".into(),
            error: "FROST part3 crashed".to_string(),
        },
    );

    // Modal surfaced so the user can't silently proceed with bad data.
    match model.ui_state.modal.as_ref() {
        Some(Modal::Error { message, .. }) => {
            assert!(
                message.contains("FROST part3"),
                "error modal must carry the failure reason; got {:?}",
                message
            );
        }
        other => panic!("expected Modal::Error; got {:?}", other),
    }
    // Pending state drained.
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
}

#[test]
fn copy_to_clipboard_emits_notification() {
    // End-to-end test of the clipboard copy flow. We can't inspect the
    // real system clipboard from a unit test, but we can assert that
    // the handler pushes a notification in either direction (success
    // in terminals with X11/Wayland/macOS pasteboard, warning in CI
    // where arboard has no display). Both paths MUST push something
    // so the user always gets feedback on their keypress.
    let mut model = fresh_model();
    let before = model.ui_state.notifications.len();
    let cmd = update(
        &mut model,
        Message::CopyToClipboard {
            text: "deadbeef".to_string(),
            label: "test-label".to_string(),
        },
    );
    assert!(cmd.is_none(), "copy-to-clipboard is a terminal message");
    assert_eq!(
        model.ui_state.notifications.len(),
        before + 1,
        "copy-to-clipboard must push a notification"
    );
    let notif = model.ui_state.notifications.last().unwrap();
    assert!(
        notif.text.contains("test-label"),
        "notification must mention the label so the user knows what was \
         (or wasn't) copied; got {:?}",
        notif.text
    );
}

#[test]
fn navigate_home_clears_last_completed_signature() {
    use starlab_client::elm::model::CompletedSignatureInfo;
    let mut model = fresh_model();
    model.wallet_state.last_completed_signature = Some(CompletedSignatureInfo {
        request_id: "x".into(),
        wallet_id: "w".into(),
        message: vec![],
        signed_hash: None,
        signature: vec![],
        verified: true,
    });
    update(&mut model, Message::NavigateHome);
    assert!(model.wallet_state.last_completed_signature.is_none());
}

// -----------------------------------------------------------------
// Cold-start creator signing path
// -----------------------------------------------------------------

#[test]
fn sign_submit_when_wallet_unlocked_dispatches_initiate_signing_directly() {
    // Warm session: DKG just ran, wallet_unlocked_id matches the
    // target wallet → no password roundtrip needed. After Stage 3
    // the flow is SignSubmit → modal → ConfirmSigningRequest →
    // InitiateSigning; assert the warm shortcut survives the split.
    use starlab_client::elm::command::Command;
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "w-warm".to_string(),
    };
    // Warm marker is the account-0 child — that's the id signing uses.
    model.wallet_state.wallet_unlocked_id = Some("w-warm-ethereum-0".to_string());
    model.wallet_state.curve_type = "secp256k1";
    for c in "hi".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    let cmd = update(&mut model, Message::SignSubmit);
    assert!(cmd.is_none(), "modal opens first, no Command yet");
    assert!(
        model.wallet_state.pending_sign_preview.as_ref().unwrap().warm,
        "warm path must be recognised in the preview"
    );
    let cmd = update(&mut model, Message::ConfirmSigningRequest);
    match cmd {
        Some(Command::SendMessage(Message::InitiateSigning { request })) => {
            assert_eq!(request.wallet_id, "w-warm-ethereum-0");
            // secp256k1 → transaction_data is the EIP-191 hash.
            assert_eq!(
                request.transaction_data,
                starlab_client::utils::eth_helper::eip191_hash(b"hi")
            );
            assert_eq!(request.raw_message.as_deref(), Some(b"hi".as_ref()));
        }
        other => panic!(
            "warm-session ConfirmSigningRequest must dispatch InitiateSigning; got {:?}",
            other
        ),
    }
    // Pending-sign state stays empty because we didn't route through
    // PasswordPrompt. Note `pending_raw_message` IS set — it's the
    // user-facing message for SigningComplete to display.
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert_eq!(
        model.wallet_state.pending_raw_message.as_deref(),
        Some(b"hi".as_ref())
    );
}

#[test]
fn sign_submit_when_wallet_not_unlocked_routes_to_password_prompt() {
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "w-cold".to_string(),
    };
    // wallet_unlocked_id deliberately None — simulating a cold start
    // where the user restarted the binary and has never unlocked.
    assert!(model.wallet_state.wallet_unlocked_id.is_none());

    for c in "msg".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    let cmd = update(&mut model, Message::SignSubmit);
    assert!(cmd.is_none(), "modal gate — no Command yet");
    assert!(
        !model.wallet_state.pending_sign_preview.as_ref().unwrap().warm,
        "cold path must be flagged so ConfirmSigningRequest routes to password"
    );
    // Cold path: pending_sign_* fields only populate AFTER the user
    // confirms. Until then, just the preview is staged.
    assert!(model.wallet_state.pending_sign_message.is_none());

    // Now confirm — this is what actually pushes PasswordPrompt.
    let cmd = update(&mut model, Message::ConfirmSigningRequest);
    assert!(
        cmd.is_none(),
        "cold ConfirmSigningRequest routes through navigation, no Command at this step"
    );
    assert!(
        matches!(model.current_screen, Screen::PasswordPrompt),
        "cold ConfirmSigningRequest must push PasswordPrompt; got {:?}",
        model.current_screen
    );
    // Message + wallet_id stashed for the WalletUnlocked handler to
    // consume; session_id left None — creator-cold-start flag.
    assert_eq!(
        model.wallet_state.pending_sign_message.as_deref(),
        Some(b"msg".as_ref())
    );
    assert_eq!(
        model.wallet_state.pending_sign_wallet_id.as_deref(),
        // Non-secp curve in this fixture → the primary chain is solana.
        Some("w-cold-solana-0"),
        "cold path stashes the account-0 child, never the root",
    );
    assert!(
        model.wallet_state.pending_sign_session_id.is_none(),
        "session_id None is the creator-cold-start flag"
    );
    // Draft cleared so the user doesn't accidentally re-submit.
    assert_eq!(model.wallet_state.sign_message_draft, "");
    // Preview drained.
    assert!(model.wallet_state.pending_sign_preview.is_none());
}

#[test]
fn submit_password_for_creator_cold_start_dispatches_unlock_wallet() {
    use starlab_client::elm::command::Command;
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.keystore_path = "/tmp/k".to_string();
    // State as SignSubmit would leave it in a cold-start flow.
    model.wallet_state.pending_sign_message = Some(b"hello".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("w-cold".to_string());
    model.wallet_state.pending_sign_session_id = None;

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "mysecret".to_string(),
        },
    );
    match cmd {
        Some(Command::UnlockWallet { wallet_id, keystore_path, .. }) => {
            assert_eq!(wallet_id, "w-cold");
            assert_eq!(keystore_path, "/tmp/k");
        }
        other => panic!(
            "creator-cold-start SubmitPassword must dispatch UnlockWallet; got {:?}",
            other
        ),
    }
    // Payload remains stashed — WalletUnlocked will drain it next.
    assert!(model.wallet_state.pending_sign_message.is_some());
}

#[test]
fn wallet_unlocked_with_pending_sign_no_session_id_dispatches_initiate_signing() {
    // Mirror of the joiner-path test but with session_id = None,
    // which is the creator-cold-start signal.
    use starlab_client::elm::command::Command;
    let mut model = fresh_model();
    model.wallet_state.pending_sign_message = Some(b"cold-sign".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("w-cold".to_string());
    model.wallet_state.pending_sign_session_id = None;
    model.wallet_state.curve_type = "secp256k1";

    let cmd = update(
        &mut model,
        Message::WalletUnlocked {
            wallet_id: "w-cold".to_string(),
        },
    );
    match cmd {
        Some(Command::SendMessage(Message::InitiateSigning { request })) => {
            assert_eq!(request.wallet_id, "w-cold");
            assert_eq!(request.transaction_data, b"cold-sign");
            assert_eq!(request.chain, "secp256k1");
        }
        other => panic!(
            "creator-cold-start WalletUnlocked must dispatch InitiateSigning via SendMessage; got {:?}",
            other
        ),
    }
    // All pending-sign fields drained.
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
}

#[test]
fn dkg_finalized_marks_wallet_unlocked() {
    // DKG leaves KeyPackage live on AppState — mark the wallet as
    // unlocked so the user can sign without retyping their password.
    let mut model = finalized_fixture_model();
    let _ = update(&mut model, sample_dkg_finalized_msg());
    assert_eq!(
        model.wallet_state.wallet_unlocked_id.as_deref(),
        Some("finalized-test"),
        "DKGFinalized must set wallet_unlocked_id to the just-created wallet"
    );
}

#[test]
fn wallet_unlocked_message_sets_unlocked_id() {
    let mut model = fresh_model();
    let _ = update(
        &mut model,
        Message::WalletUnlocked {
            wallet_id: "w-explicit".to_string(),
        },
    );
    assert_eq!(
        model.wallet_state.wallet_unlocked_id.as_deref(),
        Some("w-explicit"),
    );
}

#[test]
fn navigate_home_clears_wallet_unlocked_id() {
    let mut model = fresh_model();
    model.wallet_state.wallet_unlocked_id = Some("w-something".to_string());
    update(&mut model, Message::NavigateHome);
    assert!(
        model.wallet_state.wallet_unlocked_id.is_none(),
        "NavigateHome must clear wallet_unlocked_id so the next sign \
         attempt re-unlocks conservatively"
    );
}

#[test]
fn submit_password_in_creator_dkg_ignores_stale_pending_sign() {
    // Regression guard for the "creator DKG PasswordPrompt produces
    // 'Unlock Failed' modal" bug. Scenario:
    //   1. User does a cold-start sign that fails (mesh timeout, etc).
    //   2. Handlers didn't clear pending_sign_*. State leaks.
    //   3. User navigates away and later hits "Create New Wallet".
    //   4. ThresholdConfig → PasswordPrompt. User types their
    //      brand-new password, hits Enter.
    //   5. Previously: SubmitPassword saw pending_sign_message.is_some()
    //      and dispatched UnlockWallet → "Invalid password" modal
    //      (the wallet file is real but the user typed a NEW password).
    //   6. After the fix: creator-DKG flow dominates — the check is
    //      now gated on creating_wallet.is_none() && !active_session.
    use starlab_client::elm::command::Command;
    use starlab_client::elm::model::{CreateWalletState, WalletConfig, WalletMode};

    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    // Simulate the stale leak from a prior failed sign:
    model.wallet_state.pending_sign_message = Some(b"old sign".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("wallet-dkg_stale".to_string());
    model.wallet_state.pending_sign_session_id = None;
    // And the legitimate creator-DKG state from ThresholdConfig:
    model.wallet_state.creating_wallet = Some(CreateWalletState {
        mode: Some(WalletMode::Online),
        template: None,
        custom_config: Some(WalletConfig {
            name: "new-wallet".to_string(),
            threshold: 2,
            total_participants: 3,
            mode: WalletMode::Online,
        }),
    });

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "brandnew-password".to_string(),
        },
    );

    // MUST dispatch the creator DKG path, NOT UnlockWallet on the stale
    // wallet.
    match cmd {
        Some(Command::SendMessage(Message::CreateWallet { config })) => {
            assert_eq!(config.name, "new-wallet");
        }
        other => panic!(
            "creator DKG SubmitPassword with stale pending_sign_* must \
             dispatch CreateWallet, NOT UnlockWallet; got {:?}",
            other
        ),
    }
}

#[test]
fn wallet_unlock_failed_clears_pending_sign_state() {
    // The other half of the fix: if unlock fails (e.g. wrong password),
    // don't leave the pending_sign_* fields set — they'll hijack the
    // next PasswordPrompt submit.
    let mut model = fresh_model();
    model.wallet_state.pending_sign_message = Some(b"hash".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("w".to_string());
    model.wallet_state.pending_sign_session_id = None;

    update(
        &mut model,
        Message::WalletUnlockFailed {
            error: "Invalid password".to_string(),
        },
    );

    assert!(
        model.wallet_state.pending_sign_message.is_none(),
        "WalletUnlockFailed must clear pending_sign_message"
    );
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
}

#[test]
fn navigate_home_clears_pending_sign_state() {
    // Belt-and-suspenders: if a user Escapes out of a sign flow back
    // to MainMenu for any reason, the pending state must not survive.
    let mut model = fresh_model();
    model.wallet_state.pending_sign_message = Some(b"hash".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("w".to_string());
    model.wallet_state.pending_sign_session_id = None;

    update(&mut model, Message::NavigateHome);

    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
}

#[test]
fn full_chain_failed_sign_then_fresh_dkg_does_not_misroute() {
    // This is the END-TO-END reproduction of the user's bug. Mirror
    // the exact transitions from the log:
    //   1. cold-start SignSubmit stashes pending_sign_* + routes to PasswordPrompt
    //   2. UnlockWallet fails → WalletUnlockFailed fires → modal + state cleanup
    //   3. User dismisses modal and NavigateHome
    //   4. User hits "Create New Wallet" → eventually lands on PasswordPrompt with a
    //      populated creating_wallet
    //   5. SubmitPassword on the NEW password
    // Must route to CreateWallet, not a stale UnlockWallet. The fix
    // is layered (all three gates matter): remove any one and the chain
    // regresses.
    use starlab_client::elm::command::Command;
    use starlab_client::elm::model::{CreateWalletState, WalletConfig, WalletMode};

    let mut model = fresh_model();
    model.wallet_state.keystore_path = "/tmp/k".to_string();
    model.wallet_state.curve_type = "secp256k1";
    model.wallet_state.wallet_unlocked_id = None;

    // --- Step 1: user tries to sign with an existing wallet, cold ---
    // Populate a wallet in the cache so SignSubmit has a target.
    use starlab_client::keystore::WalletMetadata;
    model.wallet_state.wallets = vec![WalletMetadata::new(
        "wallet-dkg_old".to_string(),
        "mpc-1".to_string(),
        "secp256k1".to_string(),
        2,
        3,
        1,
        "aabb".to_string(),
    )];
    model.current_screen = Screen::SignTransaction {
        wallet_id: "wallet-dkg_old".to_string(),
    };
    for c in "hello".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    // Stage 3: SignSubmit opens confirmation modal first, not
    // PasswordPrompt. ConfirmSigningRequest is what pushes to prompt
    // on the cold path.
    let sign_cmd = update(&mut model, Message::SignSubmit);
    assert!(sign_cmd.is_none(), "SignSubmit only stages the modal");
    let confirm_cmd = update(&mut model, Message::ConfirmSigningRequest);
    assert!(
        confirm_cmd.is_none(),
        "cold ConfirmSigningRequest routes via nav push, no command"
    );
    assert!(
        matches!(model.current_screen, Screen::PasswordPrompt),
        "cold confirm pushes PasswordPrompt"
    );
    assert!(
        model.wallet_state.pending_sign_message.is_some(),
        "cold confirm stashes the hash-to-sign"
    );

    // --- Step 2: user types WRONG password, UnlockWallet fails ---
    // (We skip the actual UnlockWallet dispatch — what matters for
    // this regression is the WalletUnlockFailed cleanup.)
    update(
        &mut model,
        Message::WalletUnlockFailed {
            error: "Invalid password".to_string(),
        },
    );
    assert!(
        model.wallet_state.pending_sign_message.is_none(),
        "after WalletUnlockFailed, pending_sign_message must be gone"
    );
    assert!(
        matches!(model.ui_state.modal, Some(starlab_client::elm::model::Modal::Error { .. })),
        "error modal must surface so the user acknowledges"
    );

    // --- Step 3: user navigates home (defense-in-depth clear) ---
    model.ui_state.modal = None; // user dismissed the modal
    update(&mut model, Message::NavigateHome);
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());

    // --- Step 4: user enters Create New Wallet flow ---
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.creating_wallet = Some(CreateWalletState {
        mode: Some(WalletMode::Online),
        template: None,
        custom_config: Some(WalletConfig {
            name: "brand-new".to_string(),
            threshold: 2,
            total_participants: 3,
            mode: WalletMode::Online,
        }),
    });

    // --- Step 5: password submit — must route to CreateWallet ---
    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "brandnew-password".to_string(),
        },
    );
    match cmd {
        Some(Command::SendMessage(Message::CreateWallet { config })) => {
            assert_eq!(
                config.name, "brand-new",
                "creator DKG flow wins — NOT the stale sign flow"
            );
        }
        other => panic!(
            "after the failed-sign → home → create-wallet chain, \
             SubmitPassword must CreateWallet; got {:?}",
            other
        ),
    }
}

#[test]
fn submit_password_in_joiner_flow_ignores_stale_pending_sign() {
    // Symmetric to the creator test: the joiner DKG / joiner signing
    // flow is also gated against stale pending_sign_*. The signal for
    // "this is a joiner" is `active_session.is_some()`.
    use starlab_client::SessionInfo;
    use starlab_client::elm::command::Command;
    use starlab_client::protocal::signal::SessionType;

    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.keystore_path = "/tmp/k".to_string();
    // Leaked state:
    model.wallet_state.pending_sign_message = Some(b"stale".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("wallet-stale".to_string());
    model.wallet_state.pending_sign_session_id = None;
    // Real joiner intent:
    model.active_session = Some(SessionInfo {
        session_id: "dkg-joiner-flow".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".into(), "mpc-2".into(), "mpc-3".into()],
        session_type: SessionType::DKG,
        curve_type: "secp256k1".to_string(),
        coordination_type: "Network".to_string(),
        signing_message_hex: None,
    });

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "joiner-pw-123".to_string(),
        },
    );

    match cmd {
        Some(Command::JoinDKG { session_id, .. }) => {
            assert_eq!(session_id, "dkg-joiner-flow");
        }
        other => panic!(
            "joiner DKG flow must dominate over stale pending_sign_*; got {:?}",
            other
        ),
    }
}

#[test]
fn sign_submit_can_still_route_when_no_dkg_or_joiner_intent() {
    // After the gating fix, make sure the LEGITIMATE cold-sign flow
    // still works — i.e. the gate doesn't over-block. User without
    // creating_wallet AND without active_session CAN route to
    // UnlockWallet.
    use starlab_client::elm::command::Command;
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.keystore_path = "/tmp/k".to_string();
    model.wallet_state.pending_sign_message = Some(b"real hash".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("wallet-real".to_string());
    model.wallet_state.pending_sign_session_id = None;
    // Neither creating_wallet nor active_session — the "legitimate
    // cold-start sign" case.
    assert!(model.wallet_state.creating_wallet.is_none());
    assert!(model.active_session.is_none());

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "sign-unlock-pw".to_string(),
        },
    );
    match cmd {
        Some(Command::UnlockWallet { wallet_id, .. }) => {
            assert_eq!(wallet_id, "wallet-real");
        }
        other => panic!(
            "legitimate cold-start sign MUST still route to UnlockWallet; \
             got {:?}",
            other
        ),
    }
}

#[test]
fn submit_password_on_signing_session_dispatches_unlock_and_stashes_payload() {
    // Joiner path for Phase C.4: when active_session is a Signing
    // variant, SubmitPassword must dispatch Command::UnlockWallet
    // (not JoinDKG) AND stash the message bytes + session id on the
    // Model so WalletUnlocked can pick up the signing flow.
    use starlab_client::SessionInfo;
    use starlab_client::elm::command::Command;
    use starlab_client::protocal::signal::SessionType;

    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.wallet_state.keystore_path = "/tmp/k".to_string();
    model.active_session = Some(SessionInfo {
        session_id: "sign_xyz".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".into(), "mpc-2".into()],
        session_type: SessionType::Signing {
            wallet_name: "wallet-dkg_abcd".to_string(),
            curve_type: "secp256k1".to_string(),
            blockchain: "secp256k1".to_string(),
            group_public_key: "dead".to_string(),
        },
        curve_type: "secp256k1".to_string(),
        coordination_type: "Network".to_string(),
        signing_message_hex: Some("68656c6c6f".to_string()), // "hello"
    });

    let cmd = update(
        &mut model,
        Message::SubmitPassword {
            value: "a-password-123".to_string(),
        },
    );

    match cmd {
        Some(Command::UnlockWallet { wallet_id, keystore_path, .. }) => {
            assert_eq!(wallet_id, "wallet-dkg_abcd");
            assert_eq!(keystore_path, "/tmp/k");
        }
        other => panic!(
            "SubmitPassword on signing session must dispatch UnlockWallet; got {:?}",
            other
        ),
    }
    assert_eq!(
        model.wallet_state.pending_sign_message.as_deref(),
        Some(b"hello".as_ref()),
        "message bytes must be stashed for WalletUnlocked to pick up"
    );
    assert_eq!(
        model.wallet_state.pending_sign_wallet_id.as_deref(),
        Some("wallet-dkg_abcd"),
    );
    assert_eq!(
        model.wallet_state.pending_sign_session_id.as_deref(),
        Some("sign_xyz"),
    );
}

#[test]
fn wallet_unlocked_with_pending_sign_dispatches_join_signing() {
    // The second half of the joiner signing flow: once the wallet is
    // decrypted and key_package is on AppState, WalletUnlocked must
    // consume the stashed payload + session id and dispatch
    // Command::JoinSigning.
    use starlab_client::elm::command::Command;

    let mut model = fresh_model();
    model.wallet_state.pending_sign_message = Some(b"hello".to_vec());
    model.wallet_state.pending_sign_wallet_id = Some("wallet-dkg_abcd".to_string());
    model.wallet_state.pending_sign_session_id = Some("sign_xyz".to_string());

    let cmd = update(
        &mut model,
        Message::WalletUnlocked {
            wallet_id: "wallet-dkg_abcd".to_string(),
        },
    );

    match cmd {
        Some(Command::JoinSigning { session_id, message_bytes }) => {
            assert_eq!(session_id, "sign_xyz");
            assert_eq!(message_bytes, b"hello");
        }
        other => panic!(
            "WalletUnlocked with pending sign must dispatch JoinSigning; got {:?}",
            other
        ),
    }
    // Model fields drained — a later wallet unlock (e.g. re-entering
    // for another sign) must not re-run this chain with stale data.
    assert!(model.wallet_state.pending_sign_message.is_none());
    assert!(model.wallet_state.pending_sign_wallet_id.is_none());
    assert!(model.wallet_state.pending_sign_session_id.is_none());
    // Current screen should be SigningProgress.
    assert!(
        matches!(model.current_screen, Screen::SigningProgress { .. }),
        "must push SigningProgress screen; got {:?}",
        model.current_screen
    );
}

#[test]
fn wallet_unlocked_without_pending_sign_is_a_noop_command() {
    // Plain unlock (not part of a signing flow) doesn't dispatch
    // anything — the success notification already fired earlier in
    // the handler.
    let mut model = fresh_model();
    let cmd = update(
        &mut model,
        Message::WalletUnlocked {
            wallet_id: "wallet-dkg_abcd".to_string(),
        },
    );
    assert!(cmd.is_none(), "plain unlock must not dispatch a Command");
}

#[test]
fn navigate_back_from_sign_transaction_wipes_draft() {
    let mut model = fresh_model();
    model.push_screen(Screen::SignTransaction {
        wallet_id: "w".to_string(),
    });
    update(&mut model, Message::SignTypeChar('x'));
    assert_eq!(model.wallet_state.sign_message_draft, "x");

    update(&mut model, Message::NavigateBack);

    assert_eq!(
        model.wallet_state.sign_message_draft, "",
        "draft must clear on exit so a re-entry starts fresh"
    );
}

#[test]
fn navigate_home_clears_last_finalized_wallet() {
    // If the user runs DKG a second time, the first wallet's snapshot
    // must not bleed into the next render. NavigateHome is the single
    // point where the flow "starts fresh", so clear there.
    use starlab_client::elm::model::CompletedWalletInfo;
    let mut model = fresh_model();
    model.wallet_state.last_finalized_wallet = Some(CompletedWalletInfo {
        wallet_id: "w1".to_string(),
        group_pubkey_hex: "aa".to_string(),
        curve_type: "secp256k1".to_string(),
        addresses: vec![],
    });
    update(&mut model, Message::NavigateHome);
    assert!(model.wallet_state.last_finalized_wallet.is_none());
}

#[test]
fn dkg_finalized_pushes_success_notification_with_wallet_id() {
    let mut model = finalized_fixture_model();
    let before = model.ui_state.notifications.len();

    let _ = update(&mut model, sample_dkg_finalized_msg());

    assert_eq!(
        model.ui_state.notifications.len(),
        before + 1,
        "exactly one success notification should be pushed"
    );
    let notif = model.ui_state.notifications.last().unwrap();
    assert!(
        notif.text.contains("finalized-test"),
        "notification should include the wallet id; got {:?}",
        notif.text
    );
    assert!(
        notif.text.contains("2"),
        "notification should include the address count; got {:?}",
        notif.text
    );
}

// -----------------------------------------------------------------
// Auto-trigger: DKGKeyGenerated → FinalizeWalletFromDkg
// -----------------------------------------------------------------
//
// `DKGKeyGenerated` is emitted by the async Command executor the
// moment FROST part3 finishes. The update handler must hand the
// cleartext password off to `Command::FinalizeWalletFromDkg` *and*
// clear it from the Model so the plaintext lives in only one place.

fn fixture_ready_to_finalize() -> starlab_client::elm::Model {
    use starlab_client::SessionInfo;
    use starlab_client::protocal::signal::SessionType;
    let mut model = fresh_model();
    model.wallet_state.pending_password = Some("hunter2abc".to_string());
    model.wallet_state.keystore_path = "/tmp/keystore-unittest".to_string();
    model.wallet_state.dkg_round = DKGRound::Round2;
    model.wallet_state.dkg_in_progress = true;
    model.current_screen = Screen::DKGProgress {
        session_id: "dkg-abc12345-more".to_string(),
    };
    model.active_session = Some(SessionInfo {
        session_id: "dkg-abc12345-more".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".to_string(), "mpc-2".to_string(), "mpc-3".to_string()],
        session_type: SessionType::DKG,
        curve_type: "secp256k1".to_string(),
        coordination_type: "online".to_string(),
        signing_message_hex: None,
    });
    model
}

fn sample_dkg_key_generated_msg() -> Message {
    Message::DKGKeyGenerated {
        group_pubkey_hex:
            "021de2d69979f0a03ea413e7ed6a32ad02111b90d1f03793649157d3e4ee952143".to_string(),
    }
}

#[test]
fn dkg_key_generated_auto_dispatches_finalize_with_correct_fields() {
    use starlab_client::elm::command::Command;
    let mut model = fixture_ready_to_finalize();

    let cmd = update(&mut model, sample_dkg_key_generated_msg());

    // Extract the FinalizeWalletFromDkg inside whatever wrapping the
    // handler chose (Batch or bare).
    fn find_finalize(cmd: &Command) -> Option<(String, String, String)> {
        match cmd {
            Command::FinalizeWalletFromDkg {
                password,
                keystore_path,
                wallet_name,
                ..
            } => Some((password.clone(), keystore_path.clone(), wallet_name.clone())),
            Command::Batch(children) => children.iter().find_map(find_finalize),
            _ => None,
        }
    }

    let (password, keystore_path, wallet_name) = cmd
        .as_ref()
        .and_then(find_finalize)
        .expect("expected FinalizeWalletFromDkg (bare or inside Batch) in the command");

    assert_eq!(password, "hunter2abc", "password must be passed through to the Command verbatim");
    assert_eq!(keystore_path, "/tmp/keystore-unittest");
    // Derivation matches protocal::dkg::wallet_id_from_session: the first 12
    // hex digits of the session id ("dkg-abc12345-more" → "dabc12345e"), so
    // every participant ends up with the same identifier.
    assert_eq!(
        wallet_name,
        starlab_client::protocal::dkg::wallet_id_from_session("dkg-abc12345-more"),
        "wallet_name must use the shared wallet_id_from_session derivation"
    );
    assert_eq!(wallet_name, "dabc12345e");
}

#[test]
fn dkg_key_generated_clears_pending_password_from_model() {
    // Security invariant: the Command now owns the plaintext; the Model's
    // copy must be None so it can't be read a second time. This is what
    // stops a later-dispatched handler (or a logging slip) from seeing
    // the password after it's been handed off.
    let mut model = fixture_ready_to_finalize();
    let _ = update(&mut model, sample_dkg_key_generated_msg());
    assert!(
        model.wallet_state.pending_password.is_none(),
        "pending_password must be taken (not cloned) so plaintext lives in one place only"
    );
}

#[test]
fn dkg_key_generated_still_emits_force_remount_on_progress_screen() {
    use starlab_client::elm::command::Command;
    let mut model = fixture_ready_to_finalize();
    let cmd = update(&mut model, sample_dkg_key_generated_msg());

    fn has_force_remount(cmd: &Command) -> bool {
        match cmd {
            Command::SendMessage(Message::ForceRemount) => true,
            Command::Batch(children) => children.iter().any(has_force_remount),
            _ => false,
        }
    }
    assert!(
        cmd.as_ref().is_some_and(has_force_remount),
        "ForceRemount must still be dispatched so the 100% Complete state is visible to the user"
    );
}

#[test]
fn dkg_key_generated_without_password_logs_and_does_not_dispatch_finalize() {
    // If the user somehow reached DKGProgress without going through
    // PasswordPrompt (bug upstream), we must NOT panic, and we must NOT
    // dispatch a FinalizeWalletFromDkg with an empty string for a
    // password — that would produce an unreadable wallet file.
    use starlab_client::elm::command::Command;
    let mut model = fixture_ready_to_finalize();
    model.wallet_state.pending_password = None;

    let cmd = update(&mut model, sample_dkg_key_generated_msg());

    fn has_finalize(cmd: &Command) -> bool {
        match cmd {
            Command::FinalizeWalletFromDkg { .. } => true,
            Command::Batch(children) => children.iter().any(has_finalize),
            _ => false,
        }
    }
    assert!(
        !cmd.as_ref().is_some_and(has_finalize),
        "finalize must not run when pending_password is None"
    );
}

#[test]
fn dkg_key_generated_without_keystore_path_does_not_dispatch_finalize() {
    use starlab_client::elm::command::Command;
    let mut model = fixture_ready_to_finalize();
    model.wallet_state.keystore_path = "".to_string();

    let cmd = update(&mut model, sample_dkg_key_generated_msg());

    fn has_finalize(cmd: &Command) -> bool {
        match cmd {
            Command::FinalizeWalletFromDkg { .. } => true,
            Command::Batch(children) => children.iter().any(has_finalize),
            _ => false,
        }
    }
    assert!(
        !cmd.as_ref().is_some_and(has_finalize),
        "finalize must not run when keystore_path is empty (Model wasn't initialised properly)"
    );
}

#[test]
fn dkg_key_generated_derives_wallet_name_idempotently_per_session() {
    // Regression guard for the cross-node identifier mismatch bug.
    // Every participant must derive the SAME wallet_name from the
    // SAME session_id regardless of the wire ordering of
    // session.participants (which differs per node — each node adds
    // itself to the end). This pins the derivation formula so a future
    // refactor can't accidentally start basing the name on, say,
    // `session.participants[0]`.
    use starlab_client::SessionInfo;
    use starlab_client::elm::command::Command;
    use starlab_client::protocal::signal::SessionType;

    fn run_with_participants(order: Vec<&str>, self_id: &str) -> String {
        let mut model = fresh_model();
        model.wallet_state.pending_password = Some("pw12345678".to_string());
        model.wallet_state.keystore_path = "/tmp/k".to_string();
        model.current_screen = Screen::DKGProgress {
            session_id: "abcd1234rest".to_string(),
        };
        model.device_id = self_id.to_string();
        model.active_session = Some(SessionInfo {
            session_id: "abcd1234rest".to_string(),
            proposer_id: "mpc-1".to_string(),
            total: 3,
            threshold: 2,
            participants: order.iter().map(|s| s.to_string()).collect(),
            session_type: SessionType::DKG,
            curve_type: "secp256k1".to_string(),
            coordination_type: "online".to_string(),
            signing_message_hex: None,
        });
        let cmd = update(&mut model, sample_dkg_key_generated_msg());

        fn find_name(c: &Command) -> Option<String> {
            match c {
                Command::FinalizeWalletFromDkg { wallet_name, .. } => Some(wallet_name.clone()),
                Command::Batch(v) => v.iter().find_map(find_name),
                _ => None,
            }
        }
        cmd.as_ref().and_then(find_name).expect("finalize expected")
    }

    let name_1 = run_with_participants(vec!["mpc-2", "mpc-3", "mpc-1"], "mpc-1");
    let name_2 = run_with_participants(vec!["mpc-1", "mpc-3", "mpc-2"], "mpc-2");
    let name_3 = run_with_participants(vec!["mpc-1", "mpc-2", "mpc-3"], "mpc-3");

    // "abcd1234rest" → first 12 hex digits → "abcd1234e".
    assert_eq!(name_1, "abcd1234e", "mpc-1 name");
    assert_eq!(name_2, "abcd1234e", "mpc-2 name");
    assert_eq!(name_3, "abcd1234e", "mpc-3 name");
    assert_eq!(name_1, name_2);
    assert_eq!(name_2, name_3);
}

#[test]
fn dkg_finalized_handles_truncated_group_key_gracefully() {
    // Defensive: even a short hex string shouldn't panic the truncation slice.
    // Production keys are always 66 chars but the handler must not trust that.
    let mut model = finalized_fixture_model();
    let short_key_msg = Message::DKGFinalized {
        wallet_id: "w".to_string(),
        group_pubkey_hex: "ab".to_string(),
        curve_type: "secp256k1".to_string(),
        addresses: vec![],
    };
    let _ = update(&mut model, short_key_msg);
    // If we got here without panicking, the min() guard worked.
}

// -----------------------------------------------------------------
// ManageWallets keyboard nav: arrows must move the WalletList cursor,
// and Enter must sign the wallet at that cursor. Before the wiring
// landed, arrows fell through the generic ScrollUp/Down arm, which
// only bumped `scroll_position` — WalletList never saw the change
// and Enter always fired on index 0.
// -----------------------------------------------------------------
#[test]
fn scroll_down_on_manage_wallets_advances_wallet_list_selection() {
    use starlab_client::elm::model::ComponentId;
    use starlab_client::keystore::WalletMetadata;

    let mut model = fresh_model();
    model.current_screen = Screen::ManageWallets;
    model.wallet_state.wallets = vec![
        WalletMetadata::new(
            "w1".into(),
            "d".into(),
            "secp256k1".into(),
            2,
            3,
            1,
            "aa".into(),
        ),
        WalletMetadata::new(
            "w2".into(),
            "d".into(),
            "secp256k1".into(),
            2,
            3,
            1,
            "bb".into(),
        ),
    ];

    assert!(!model
        .ui_state
        .selected_indices
        .contains_key(&ComponentId::WalletList));

    let _ = update(&mut model, Message::ScrollDown);
    assert_eq!(
        model.ui_state.selected_indices.get(&ComponentId::WalletList),
        Some(&1),
        "ScrollDown should advance WalletList cursor"
    );

    // Second ScrollDown must clamp to last wallet (no wrap-around
    // is intentional — users expect arrow-down-at-bottom to stop).
    let _ = update(&mut model, Message::ScrollDown);
    assert_eq!(
        model.ui_state.selected_indices.get(&ComponentId::WalletList),
        Some(&1),
        "ScrollDown past end must clamp to last index"
    );

    let _ = update(&mut model, Message::ScrollUp);
    assert_eq!(
        model.ui_state.selected_indices.get(&ComponentId::WalletList),
        Some(&0),
        "ScrollUp should move WalletList cursor toward top"
    );

    // ScrollUp at 0 must not underflow
    let _ = update(&mut model, Message::ScrollUp);
    assert_eq!(
        model.ui_state.selected_indices.get(&ComponentId::WalletList),
        Some(&0),
    );
}

/// Enter on ManageWallets must sign the wallet at the CURRENT cursor,
/// not always wallet[0]. Regression guard for "arrow keys didn't work,
/// Enter always signed the first wallet".
#[test]
fn select_item_on_manage_wallets_uses_selected_indices_for_target() {
    use starlab_client::elm::model::ComponentId;
    use starlab_client::keystore::WalletMetadata;

    let mut model = fresh_model();
    model.current_screen = Screen::ManageWallets;
    model.wallet_state.wallets = vec![
        WalletMetadata::new(
            "wallet-first".into(),
            "d".into(),
            "secp256k1".into(),
            2,
            3,
            1,
            "aa".into(),
        ),
        WalletMetadata::new(
            "wallet-second".into(),
            "d".into(),
            "secp256k1".into(),
            2,
            3,
            1,
            "bb".into(),
        ),
    ];

    // Simulate one ScrollDown (cursor → wallet-second).
    let _ = update(&mut model, Message::ScrollDown);
    assert_eq!(
        model.ui_state.selected_indices.get(&ComponentId::WalletList),
        Some(&1),
    );

    // Enter → SelectItem index is ignored by the handler; it reads
    // selected_indices[WalletList]. Index arg here is a placeholder
    // to mirror what app.rs sends.
    let _ = update(&mut model, Message::SelectItem { index: 1 });

    match &model.current_screen {
        Screen::SignTransaction { wallet_id } => {
            assert_eq!(
                wallet_id, "wallet-second",
                "Enter on ManageWallets must target the highlighted row, not wallet[0]"
            );
        }
        other => panic!("expected SignTransaction after Enter on ManageWallets, got {:?}", other),
    }
    assert_eq!(
        model.selected_wallet.as_deref(),
        Some("wallet-second"),
        "selected_wallet must be populated before entering SignTransaction"
    );
}

// -----------------------------------------------------------------
// Signing-request push notification (Stage 1 of the signing network
// flow): when the WebSocket broadcasts a signing SessionDiscovered
// and we're a listed participant, interrupt the user with a Modal::
// Confirm. Without this the co-signer has to be sitting on JoinSession
// to know a request exists, which defeats "2-of-3, any 2 online is
// enough".
// -----------------------------------------------------------------
fn signing_session(session_id: &str, participants: Vec<&str>, proposer: &str) -> starlab_client::protocal::signal::SessionInfo {
    use starlab_client::protocal::signal::{SessionInfo, SessionType};
    SessionInfo {
        session_id: session_id.to_string(),
        proposer_id: proposer.to_string(),
        total: 3,
        threshold: 2,
        participants: participants.into_iter().map(|s| s.to_string()).collect(),
        session_type: SessionType::Signing {
            wallet_name: "wallet-dkg_xx".to_string(),
            curve_type: "secp256k1".to_string(),
            blockchain: "ethereum".to_string(),
            group_public_key: "aabbccdd".to_string(),
        },
        curve_type: "secp256k1".to_string(),
        coordination_type: "Network".to_string(),
        signing_message_hex: Some(hex::encode(b"hello from alice")),
    }
}

fn dkg_session(session_id: &str, participants: Vec<&str>, proposer: &str) -> starlab_client::protocal::signal::SessionInfo {
    use starlab_client::protocal::signal::{SessionInfo, SessionType};
    SessionInfo {
        session_id: session_id.to_string(),
        proposer_id: proposer.to_string(),
        total: 3,
        threshold: 2,
        participants: participants.into_iter().map(|s| s.to_string()).collect(),
        session_type: SessionType::DKG,
        curve_type: "secp256k1".to_string(),
        coordination_type: "Network".to_string(),
        signing_message_hex: None,
    }
}

#[test]
fn session_discovered_signing_pops_modal_for_participant() {
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model(); // device_id = "test-device"
    model.current_screen = Screen::MainMenu;
    assert!(model.ui_state.modal.is_none());

    let session = signing_session("sign-1", vec!["test-device", "alice", "bob"], "alice");
    let _ = update(&mut model, Message::SessionDiscovered { session });

    match &model.ui_state.modal {
        Some(Modal::Confirm { title, message, .. }) => {
            assert!(title.contains("Signing Request"));
            assert!(message.contains("alice"), "modal should name the proposer");
            assert!(
                message.contains("hello from alice") || message.contains("\""),
                "modal should preview the message body; got: {}",
                message
            );
        }
        other => panic!("expected Modal::Confirm, got {:?}", other),
    }
}

#[test]
fn session_discovered_signing_ignored_when_not_participant() {
    let mut model = fresh_model(); // "test-device" not in participants below
    let session = signing_session("sign-2", vec!["alice", "bob", "charlie"], "alice");
    let _ = update(&mut model, Message::SessionDiscovered { session });
    assert!(
        model.ui_state.modal.is_none(),
        "must not interrupt a non-participant"
    );
}

#[test]
fn session_discovered_signing_ignored_for_self_proposed() {
    // Creator already has SigningProgress pushed; they don't need a
    // self-addressed modal about their own request.
    let mut model = fresh_model();
    let session = signing_session(
        "sign-3",
        vec!["test-device", "alice", "bob"],
        "test-device",
    );
    let _ = update(&mut model, Message::SessionDiscovered { session });
    assert!(
        model.ui_state.modal.is_none(),
        "must not modal-notify the proposer about their own request"
    );
}

#[test]
fn session_discovered_dkg_does_not_pop_signing_modal() {
    let mut model = fresh_model();
    let session = dkg_session("dkg-1", vec!["test-device", "alice", "bob"], "alice");
    let _ = update(&mut model, Message::SessionDiscovered { session });
    assert!(
        model.ui_state.modal.is_none(),
        "DKG discoveries must not trigger the signing-request modal"
    );
}

#[test]
fn session_discovered_suppresses_modal_on_join_session_screen() {
    let mut model = fresh_model();
    // If the user is already looking at JoinSession they'll see the
    // new row appear in the list; a modal would be noise.
    model.current_screen = Screen::JoinSession;
    let session = signing_session("sign-4", vec!["test-device", "alice"], "alice");
    let _ = update(&mut model, Message::SessionDiscovered { session });
    assert!(model.ui_state.modal.is_none());
}

#[test]
fn review_signing_request_navigates_to_join_session_signing_tab() {
    use starlab_client::elm::model::ComponentId;

    let mut model = fresh_model();
    // Populate two invites so we can assert the Signing-tab index is
    // computed against the filtered (signing-only) list, not the
    // mixed `session_invites` vec.
    model.session_invites = vec![
        dkg_session("dkg-a", vec!["test-device"], "proposer-1"),
        signing_session("sign-a", vec!["test-device"], "alice"),
        signing_session("sign-b", vec!["test-device", "alice"], "alice"),
    ];

    let _ = update(
        &mut model,
        Message::ReviewSigningRequest {
            session_id: "sign-b".to_string(),
        },
    );

    assert_eq!(model.current_screen, Screen::JoinSession);
    assert_eq!(model.ui_state.join_session_tab, 1, "must land on Signing tab");
    assert_eq!(
        model
            .ui_state
            .selected_indices
            .get(&ComponentId::JoinSession),
        Some(&1),
        "selected index must be computed vs. the filtered Signing list (sign-b is idx 1 there)"
    );
    assert!(
        model.ui_state.modal.is_none(),
        "must clear the triggering modal"
    );
}

// -----------------------------------------------------------------
// Stage 3: creator-side confirmation modal before FROST broadcast.
// SignSubmit now opens a Modal::Confirm instead of dispatching
// directly; Confirm/Cancel messages drive the real transition.
// -----------------------------------------------------------------

// -----------------------------------------------------------------
// Stage 5 (local-UX half): DeclineSigningRequest drops the session
// from `session_invites`, closes the modal, and pushes a toast. Wire
// propagation back to the creator is a later stage.
// -----------------------------------------------------------------

#[test]
fn decline_signing_request_drops_invite_and_posts_notification() {
    use starlab_client::elm::model::{Modal, NotificationKind};

    let mut model = fresh_model();
    let sess = signing_session("sign-xyz", vec!["test-device", "alice"], "alice");
    model.session_invites = vec![sess];
    model.ui_state.modal = Some(Modal::Confirm {
        title: "t".into(),
        message: "m".into(),
        on_confirm: Box::new(Message::CloseModal),
        on_cancel: Box::new(Message::DeclineSigningRequest {
            session_id: "sign-xyz".to_string(),
        }),
    });

    let before_notifs = model.ui_state.notifications.len();
    let _ = update(
        &mut model,
        Message::DeclineSigningRequest {
            session_id: "sign-xyz".to_string(),
        },
    );

    assert!(
        model.session_invites.is_empty(),
        "decline must purge the invite so we don't re-prompt"
    );
    assert!(model.ui_state.modal.is_none(), "modal dismissed");
    assert_eq!(
        model.ui_state.notifications.len(),
        before_notifs + 1,
        "decline must post one confirmation toast"
    );
    let n = model.ui_state.notifications.last().unwrap();
    assert!(matches!(n.kind, NotificationKind::Info));
    assert!(n.text.contains("Declined"));
}

#[test]
fn decline_signing_request_for_unknown_session_posts_no_toast() {
    // A ghost decline (user already acted, maybe via a duplicate
    // event) should silently no-op — no spurious "Declined X"
    // notification for a session that's already gone.
    let mut model = fresh_model();
    let before = model.ui_state.notifications.len();
    let _ = update(
        &mut model,
        Message::DeclineSigningRequest {
            session_id: "ghost".to_string(),
        },
    );
    assert_eq!(model.ui_state.notifications.len(), before);
    assert!(model.ui_state.modal.is_none());
}

/// End-to-end local flow: SessionDiscovered stages the modal with a
/// DeclineSigningRequest on_cancel; firing CancelModal routes through
/// to DeclineSigningRequest, which drops the invite. Guards against
/// a regression where Cancel used to dispatch plain CloseModal and
/// leave the invite sitting in session_invites forever.
#[test]
fn session_discovered_cancel_chain_purges_invite() {
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model();
    let sess = signing_session("sign-chain", vec!["test-device", "alice"], "alice");
    let _ = update(
        &mut model,
        Message::SessionDiscovered {
            session: sess,
        },
    );
    match &model.ui_state.modal {
        Some(Modal::Confirm { on_cancel, .. }) => {
            // Verify the modal's on_cancel payload carries the session id.
            if !matches!(
                **on_cancel,
                Message::DeclineSigningRequest { ref session_id }
                    if session_id == "sign-chain"
            ) {
                panic!(
                    "on_cancel must be DeclineSigningRequest for this session; got {:?}",
                    on_cancel
                );
            }
        }
        other => panic!("expected Modal::Confirm, got {:?}", other),
    }
    // Simulate the CancelModal → on_cancel dispatch that the key
    // handler wires up.
    let cancel_cmd = update(&mut model, Message::CancelModal);
    // The SendMessage wrapper carries the DeclineSigningRequest payload.
    use starlab_client::elm::command::Command;
    match cancel_cmd {
        Some(Command::SendMessage(ref inner)) => match inner {
            Message::DeclineSigningRequest { session_id } => {
                assert_eq!(session_id, "sign-chain");
            }
            other => panic!("cancel chain inner must be Decline; got {:?}", other),
        },
        other => panic!("CancelModal must dispatch on_cancel; got {:?}", other),
    }
    // We still need to process the DeclineSigningRequest itself —
    // the runtime would do this via process_message. Do it inline.
    let _ = update(
        &mut model,
        Message::DeclineSigningRequest {
            session_id: "sign-chain".to_string(),
        },
    );
    assert!(model.session_invites.is_empty());
}

// -----------------------------------------------------------------
// Stage 4: signing-ceremony acceptance roster on WalletState.
// ProcessSigningRound1/2 messages record the sender in
// signing_commitments_received / signing_shares_received so the
// creator's SigningProgress screen can show live "Bob ✓ committed"
// rows. Resets fire on InitiateSigning (fresh ceremony),
// SigningComplete, SigningFailed, NavigateHome.
// -----------------------------------------------------------------

#[test]
fn process_signing_round1_records_committer() {
    let mut model = fresh_model();
    assert!(model.wallet_state.signing_commitments_received.is_empty());

    let _ = update(
        &mut model,
        Message::ProcessSigningRound1 {
            from_device: "bob".to_string(),
            commitment_bytes: vec![0u8; 32],
        },
    );

    assert!(
        model
            .wallet_state
            .signing_commitments_received
            .contains("bob"),
        "round-1 commitment from bob must land on the roster"
    );
    // A commitment doesn't imply a share yet.
    assert!(model.wallet_state.signing_shares_received.is_empty());
}

#[test]
fn process_signing_round2_records_sharer() {
    let mut model = fresh_model();
    let _ = update(
        &mut model,
        Message::ProcessSigningRound2 {
            from_device: "charlie".to_string(),
            share_bytes: vec![0u8; 32],
        },
    );
    assert!(
        model
            .wallet_state
            .signing_shares_received
            .contains("charlie"),
    );
}

#[test]
fn initiate_signing_clears_stale_roster_from_prior_ceremony() {
    use starlab_client::elm::message::SigningRequest;

    let mut model = fresh_model();
    // Pollute the roster as if a prior ceremony had run.
    model
        .wallet_state
        .signing_commitments_received
        .insert("ghost".to_string());
    model
        .wallet_state
        .signing_shares_received
        .insert("ghost".to_string());

    let req = SigningRequest {
        wallet_id: "w".into(),
        transaction_data: b"new".to_vec(),
        chain: "secp256k1".into(),
        metadata: None,
        raw_message: None,
    };
    let _ = update(&mut model, Message::InitiateSigning { request: req });

    assert!(
        model.wallet_state.signing_commitments_received.is_empty(),
        "a fresh ceremony must start with an empty commitments roster"
    );
    assert!(model.wallet_state.signing_shares_received.is_empty());
}

#[test]
fn signing_failed_clears_roster_so_retry_starts_fresh() {
    let mut model = fresh_model();
    model
        .wallet_state
        .signing_commitments_received
        .insert("alice".to_string());
    model
        .wallet_state
        .signing_shares_received
        .insert("alice".to_string());

    let _ = update(
        &mut model,
        Message::SigningFailed {
            request_id: "r".to_string(),
            error: "peer dropped".to_string(),
        },
    );
    assert!(model.wallet_state.signing_commitments_received.is_empty());
    assert!(model.wallet_state.signing_shares_received.is_empty());
}

#[test]
fn navigate_home_clears_signing_roster() {
    let mut model = fresh_model();
    model
        .wallet_state
        .signing_commitments_received
        .insert("bob".to_string());
    model
        .wallet_state
        .signing_shares_received
        .insert("bob".to_string());

    let _ = update(&mut model, Message::NavigateHome);

    assert!(model.wallet_state.signing_commitments_received.is_empty());
    assert!(model.wallet_state.signing_shares_received.is_empty());
}

// -----------------------------------------------------------------
// Modal Enter/Esc routing: Enter dispatches ConfirmModal (fires
// on_confirm), Esc dispatches CancelModal (fires on_cancel). Prior
// to the fix both keys dispatched CloseModal, silently dropping both
// handlers — a real bug that was only surfaced when Stages 1 + 3
// started depending on Confirm modals.
// -----------------------------------------------------------------

#[test]
fn confirm_modal_dispatches_on_confirm_message() {
    use starlab_client::elm::command::Command;
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model();
    // Arbitrary no-op inner payload; we just want to see it emerge.
    model.ui_state.modal = Some(Modal::Confirm {
        title: "t".into(),
        message: "m".into(),
        on_confirm: Box::new(Message::CancelDKG),
        on_cancel: Box::new(Message::CloseModal),
    });

    let cmd = update(&mut model, Message::ConfirmModal);

    assert!(model.ui_state.modal.is_none(), "modal must close");
    match cmd {
        Some(Command::SendMessage(Message::CancelDKG)) => {}
        other => panic!(
            "ConfirmModal must dispatch on_confirm as a SendMessage; got {:?}",
            other
        ),
    }
}

#[test]
fn cancel_modal_dispatches_on_cancel_message() {
    use starlab_client::elm::command::Command;
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model();
    model.ui_state.modal = Some(Modal::Confirm {
        title: "t".into(),
        message: "m".into(),
        on_confirm: Box::new(Message::CancelDKG),
        on_cancel: Box::new(Message::NavigateHome),
    });

    let cmd = update(&mut model, Message::CancelModal);

    assert!(model.ui_state.modal.is_none());
    match cmd {
        Some(Command::SendMessage(Message::NavigateHome)) => {}
        other => panic!(
            "CancelModal must dispatch on_cancel; got {:?}",
            other
        ),
    }
}

#[test]
fn confirm_modal_on_non_confirm_variant_just_closes() {
    // Error/Success/Progress don't have on_confirm. ConfirmModal and
    // CancelModal must both reduce to "close the modal" with no
    // command dispatched. Preserves the pre-fix behaviour for plain
    // notification-style modals.
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model();
    model.ui_state.modal = Some(Modal::Error {
        title: "e".into(),
        message: "bad".into(),
    });
    let cmd = update(&mut model, Message::ConfirmModal);
    assert!(cmd.is_none());
    assert!(model.ui_state.modal.is_none());

    model.ui_state.modal = Some(Modal::Success {
        title: "s".into(),
        message: "ok".into(),
    });
    let cmd = update(&mut model, Message::CancelModal);
    assert!(cmd.is_none());
    assert!(model.ui_state.modal.is_none());
}

#[test]
fn sign_submit_stages_modal_with_message_preview_and_hash() {
    use starlab_client::elm::model::Modal;
    use starlab_client::keystore::WalletMetadata;

    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "wallet-preview".to_string(),
    };
    model.wallet_state.curve_type = "secp256k1";
    model.wallet_state.wallet_unlocked_id = Some("wallet-preview".to_string());
    // Wallet metadata populates the threshold line in the preview body.
    model.wallet_state.wallets = vec![WalletMetadata::new(
        "wallet-preview".into(),
        "d".into(),
        "secp256k1".into(),
        2,
        3,
        1,
        "abcd".into(),
    )];
    for c in "pay bob 1 ETH".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }

    let _ = update(&mut model, Message::SignSubmit);

    match &model.ui_state.modal {
        Some(Modal::Confirm { title, message, .. }) => {
            assert!(title.contains("Confirm"));
            assert!(
                message.contains("pay bob 1 ETH"),
                "modal must show the user-typed message; got: {}",
                message
            );
            assert!(
                message.contains("EIP-191"),
                "secp256k1 modal must label the hash as EIP-191"
            );
            assert!(
                message.contains("2-of-3"),
                "modal must show the wallet threshold"
            );
        }
        other => panic!("expected Modal::Confirm, got {:?}", other),
    }
    // Draft preserved until confirm so cancel can go back to editing.
    assert_eq!(model.wallet_state.sign_message_draft, "pay bob 1 ETH");
}

#[test]
fn cancel_signing_request_clears_preview_and_keeps_draft() {
    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "w".to_string(),
    };
    model.wallet_state.curve_type = "secp256k1";
    model.wallet_state.wallet_unlocked_id = Some("w".to_string());
    for c in "keep me".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    let _ = update(&mut model, Message::SignSubmit);
    assert!(model.wallet_state.pending_sign_preview.is_some());

    let cmd = update(&mut model, Message::CancelSigningRequest);

    assert!(cmd.is_none());
    assert!(model.ui_state.modal.is_none());
    assert!(model.wallet_state.pending_sign_preview.is_none());
    // Draft preserved so the user can edit and resubmit.
    assert_eq!(
        model.wallet_state.sign_message_draft, "keep me",
        "cancel must NOT wipe the user's typed message"
    );
    // Nothing dispatched to the network.
    assert!(model.wallet_state.pending_sign_message.is_none());
}

#[test]
fn double_confirm_signing_request_is_a_safe_noop() {
    // take()-semantics on the preview — a stuck-key or fast-clicker
    // cannot cause two InitiateSigning dispatches.
    use starlab_client::elm::command::Command;

    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "w2".to_string(),
    };
    model.wallet_state.curve_type = "secp256k1";
    model.wallet_state.wallet_unlocked_id = Some("w2-ethereum-0".to_string());
    for c in "once".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    let _ = update(&mut model, Message::SignSubmit);

    let first = update(&mut model, Message::ConfirmSigningRequest);
    assert!(
        matches!(first, Some(Command::SendMessage(Message::InitiateSigning { .. }))),
        "first confirm must dispatch"
    );

    let second = update(&mut model, Message::ConfirmSigningRequest);
    assert!(
        second.is_none(),
        "second confirm must be a no-op; got {:?}",
        second
    );
    assert!(model.wallet_state.pending_sign_preview.is_none());
}

#[test]
fn sign_submit_preview_for_ed25519_omits_eip191_line() {
    // ed25519 signs the message bytes directly — no hash-then-sign —
    // so the modal should NOT advertise an EIP-191 hash line.
    use starlab_client::elm::model::Modal;

    let mut model = fresh_model();
    model.current_screen = Screen::SignTransaction {
        wallet_id: "w-sol".to_string(),
    };
    model.wallet_state.curve_type = "ed25519";
    model.wallet_state.wallet_unlocked_id = Some("w-sol".to_string());
    for c in "solmsg".chars() {
        update(&mut model, Message::SignTypeChar(c));
    }
    let _ = update(&mut model, Message::SignSubmit);
    match &model.ui_state.modal {
        Some(Modal::Confirm { message, .. }) => {
            assert!(
                !message.contains("EIP-191"),
                "ed25519 modal must NOT say EIP-191; got: {}",
                message
            );
            assert!(message.contains("solmsg"));
        }
        other => panic!("expected Modal::Confirm, got {:?}", other),
    }
}

#[test]
fn review_signing_request_for_unknown_session_is_safe_noop() {
    let mut model = fresh_model();
    let screen_before = model.current_screen.clone();
    let _ = update(
        &mut model,
        Message::ReviewSigningRequest {
            session_id: "ghost".to_string(),
        },
    );
    assert_eq!(model.current_screen, screen_before);
}

/// ScrollUp/Down on ManageWallets with NO wallets must be a safe no-op.
/// Guards against an underflow or panic when the list is empty (e.g.
/// cold-start before any DKG).
#[test]
fn scroll_on_manage_wallets_with_no_wallets_is_noop() {
    use starlab_client::elm::model::ComponentId;

    let mut model = fresh_model();
    model.current_screen = Screen::ManageWallets;
    assert!(model.wallet_state.wallets.is_empty());

    let _ = update(&mut model, Message::ScrollDown);
    let _ = update(&mut model, Message::ScrollUp);
    let _ = update(&mut model, Message::ScrollDown);

    let idx = model
        .ui_state
        .selected_indices
        .get(&ComponentId::WalletList)
        .copied();
    // Either None (never inserted) or Some(0) are both acceptable;
    // the invariant is "no panic, no invalid index".
    assert!(idx.is_none() || idx == Some(0));
}
