//! TUI Components using tui-realm
//!
//! This module contains all UI components implemented using the tui-realm framework,
//! following the Elm Architecture pattern.

// Core UI components
pub mod create_wallet;
pub mod main_menu;
pub mod modal;
pub mod notification;
pub mod wallet_detail;
pub mod wallet_list;

// Professional wallet creation flow components
pub mod join_session;
pub mod mode_selection;
pub mod password_prompt;
pub mod threshold_config;

// DKG components
pub mod dkg_progress;
pub mod wallet_complete;

// Signing components (Phase C)
pub mod sign_transaction;
pub mod signature_complete;
pub mod wallet_transfer;

// Main exports
pub use create_wallet::CreateWalletComponent;
pub use main_menu::MainMenu;
pub use modal::ModalComponent;
pub use wallet_detail::WalletDetail;
pub use wallet_list::WalletList;

// Professional wallet creation flow components
pub use join_session::JoinSessionComponent;
pub use mode_selection::ModeSelectionComponent;
pub use password_prompt::PasswordPromptComponent;
pub use threshold_config::ThresholdConfigComponent;

// DKG components
pub use dkg_progress::DKGProgressComponent;
pub use sign_transaction::SignTransactionComponent;
pub use signature_complete::SignatureCompleteComponent;
pub use wallet_complete::WalletCompleteComponent;
pub use wallet_transfer::WalletTransferComponent;

use tuirealm::component::AppComponent;

/// Trait for MPC wallet components
pub trait MpcWalletComponent: AppComponent<crate::elm::message::Message, UserEvent> {
    /// Get the component's ID
    fn id(&self) -> Id;

    /// Check if the component should be visible
    fn is_visible(&self) -> bool;

    /// Handle focus change
    fn on_focus(&mut self, focused: bool);
}

/// Component IDs for the view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    MainMenu,
    WalletList,
    WalletDetail,
    CreateWallet,
    Modal, // Alias for ModalDialog
    ModalDialog,
    ModeSelection,
    ThresholdConfig,
    JoinSession,
    DKGProgress,
    /// Mount slot for the pre-DKG password-capture component.
    PasswordPrompt,
    /// Mount slot for the post-DKG success screen that shows the group
    /// verifying key + all derived chain addresses.
    WalletComplete,
    /// Mount slot for the SignTransaction input screen (Phase C).
    SignTransaction,
    /// Mount slot for the SignatureComplete success screen (Phase C.5).
    SignatureComplete,
    /// Mount slot for the Export / Import wallet screens.
    WalletTransfer,
}

/// User events emitted by components
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    MenuItemSelected(usize),
    WalletSelected(usize),
    CreateWalletRequested,
    NavigateBack,
    Quit,
    ModalConfirm,
    ModalCancel,
    FocusGained,
    FocusLost,
}
