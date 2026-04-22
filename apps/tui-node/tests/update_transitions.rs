//! Integration tests for `elm::update::update`.
//!
//! These exercise the pure `(Model, Message) → (Model, Option<Command>)`
//! function — no network, no TTY, no async — so they run in milliseconds
//! and can be executed by `cargo test -p tui-node --test update_transitions`.
//!
//! Scope: the DKG-phase state transitions added in commit 442549d. The smoke
//! test (`scripts/smoke-dkg.sh`) covers the same behaviour end-to-end in ~30s;
//! these run in under a second and catch regressions before the smoke test
//! is even worth booting.

use tui_node::elm::command::Command;
use tui_node::elm::message::{DKGRound, Message};
use tui_node::elm::update::update;
use tui_node::elm::{Model, Screen};

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
fn creator_on_password_prompt() -> tui_node::elm::Model {
    use tui_node::elm::model::{CreateWalletState, WalletConfig, WalletMode};
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
fn joiner_on_password_prompt() -> tui_node::elm::Model {
    use tui_node::SessionInfo;
    use tui_node::protocal::signal::SessionType;
    let mut model = fresh_model();
    model.current_screen = Screen::PasswordPrompt;
    model.active_session = Some(SessionInfo {
        session_id: "dkg-test-session".to_string(),
        proposer_id: "mpc-1".to_string(),
        total: 3,
        threshold: 2,
        participants: vec!["mpc-1".to_string(), "mpc-2".to_string(), "mpc-3".to_string()],
        session_type: SessionType::DKG,
        curve_type: "unified".to_string(),
        coordination_type: "online".to_string(),
    });
    model
}

#[test]
fn creator_submit_password_stashes_value_and_dispatches_create_wallet() {
    use tui_node::elm::command::Command;
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
    use tui_node::elm::command::Command;
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
        Some(Command::JoinDKG { session_id }) => {
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

fn finalized_fixture_model() -> tui_node::elm::Model {
    use tui_node::elm::model::{CreateWalletState, WalletConfig, WalletMode};
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
    use tui_node::elm::message::Message;
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

#[test]
fn navigate_home_clears_last_finalized_wallet() {
    // If the user runs DKG a second time, the first wallet's snapshot
    // must not bleed into the next render. NavigateHome is the single
    // point where the flow "starts fresh", so clear there.
    use tui_node::elm::model::CompletedWalletInfo;
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

fn fixture_ready_to_finalize() -> tui_node::elm::Model {
    use tui_node::SessionInfo;
    use tui_node::protocal::signal::SessionType;
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
        curve_type: "unified".to_string(),
        coordination_type: "online".to_string(),
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
    use tui_node::elm::command::Command;
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
    assert_eq!(
        wallet_name, "wallet-dkg-abc1",
        "wallet_name must be derived as `wallet-{{first 8 chars of session_id}}` so every \
         participant ends up with the same identifier"
    );
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
    use tui_node::elm::command::Command;
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
        cmd.as_ref().map_or(false, has_force_remount),
        "ForceRemount must still be dispatched so the 100% Complete state is visible to the user"
    );
}

#[test]
fn dkg_key_generated_without_password_logs_and_does_not_dispatch_finalize() {
    // If the user somehow reached DKGProgress without going through
    // PasswordPrompt (bug upstream), we must NOT panic, and we must NOT
    // dispatch a FinalizeWalletFromDkg with an empty string for a
    // password — that would produce an unreadable wallet file.
    use tui_node::elm::command::Command;
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
        !cmd.as_ref().map_or(false, has_finalize),
        "finalize must not run when pending_password is None"
    );
}

#[test]
fn dkg_key_generated_without_keystore_path_does_not_dispatch_finalize() {
    use tui_node::elm::command::Command;
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
        !cmd.as_ref().map_or(false, has_finalize),
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
    use tui_node::SessionInfo;
    use tui_node::elm::command::Command;
    use tui_node::protocal::signal::SessionType;

    fn run_with_participants(order: Vec<&str>, self_id: &str) -> String {
        let mut model = fresh_model();
        model.wallet_state.pending_password = Some("pw12345678".to_string());
        model.wallet_state.keystore_path = "/tmp/k".to_string();
        model.current_screen = Screen::DKGProgress {
            session_id: "dkg_abcd1234rest".to_string(),
        };
        model.device_id = self_id.to_string();
        model.active_session = Some(SessionInfo {
            session_id: "dkg_abcd1234rest".to_string(),
            proposer_id: "mpc-1".to_string(),
            total: 3,
            threshold: 2,
            participants: order.iter().map(|s| s.to_string()).collect(),
            session_type: SessionType::DKG,
            curve_type: "unified".to_string(),
            coordination_type: "online".to_string(),
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

    assert_eq!(name_1, "wallet-dkg_abcd", "mpc-1 name");
    assert_eq!(name_2, "wallet-dkg_abcd", "mpc-2 name");
    assert_eq!(name_3, "wallet-dkg_abcd", "mpc-3 name");
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
