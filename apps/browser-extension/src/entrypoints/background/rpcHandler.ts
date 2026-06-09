// ===================================================================
// RPC HANDLER MODULE
// ===================================================================
//
// This module handles JSON-RPC requests from web applications and
// UI requests from the popup interface. It provides standardized
// wallet functionality including account management, network queries,
// and transaction handling.
// ===================================================================

import AccountService from '../../services/accountService';
import NetworkService from '../../services/networkService';
import WalletClientService from '../../services/walletClient';
import WalletController from "../../services/walletController";
import { getPermissionService } from '../../services/permissionService';
import { toHex } from 'viem';
import type { JsonRpcRequest } from "@starlab/types/messages";
import type { SessionManager } from './sessionManager';

/**
 * Handles JSON-RPC requests from web applications
 */
export class RpcHandler {
    private accountService: AccountService;
    private networkService: NetworkService;
    private walletClientService: WalletClientService;
    private permissionService = getPermissionService();
    private origin: string = '';
    /**
     * Pending signature promises keyed by signing/session id. Two
     * producers:
     *   - Legacy path (`handleSignMessageRequest` pre-Ext-4 flow):
     *     key is `msg_<timestamp>_<nonce>`; resolved by
     *     `messageSignatureComplete` via stateManager.
     *   - Ext-4 path (FROST threshold signing): key is the session
     *     id returned from SessionManager.createSigningSession
     *     (`sign_<hex>`); resolved by `signingComplete` via
     *     stateManager. Both map through the same
     *     handleSignatureComplete entrypoint, so a single Map
     *     suffices.
     */
    private pendingSignatures: Map<string, { resolve: (value: string) => void; reject: (reason?: any) => void }> = new Map();
    /**
     * Ext-4-confirm: dApp sign requests awaiting user approval in
     * the popup. Keyed by the placeholder id we assigned when the
     * RPC arrived (before any session exists). On approval, we
     * create the real session and re-key the pending promise
     * above to the actual session id; on rejection, we just reject
     * the pending promise and clean up.
     *
     * Kept distinct from pendingSignatures because a request can
     * live here indefinitely (while the popup is closed) without
     * a FROST ceremony running — aborting via reject() on timeout
     * is the only cleanup trigger.
     */
    private pendingDappRequests: Map<string, {
        walletId: string;
        walletName: string;
        groupPublicKey: string;
        blockchain: "ethereum" | "solana";
        threshold: number;
        total: number;
        messageHex: string;
        originalMessage: string;
        address: string;
        origin: string;
    }> = new Map();
    /**
     * Ext-4: injected so RPC calls (e.g. personal_sign from a dApp)
     * can create the same TUI-compatible signing session the popup
     * does. Kept optional so stateless RPC calls that don't need
     * signing (chainId, getBalance, etc.) work before it's wired.
     */
    private sessionManager?: SessionManager;

    constructor() {
        this.accountService = AccountService.getInstance();
        this.networkService = NetworkService.getInstance();
        this.walletClientService = WalletClientService.getInstance();
    }

    setSessionManager(sm: SessionManager): void {
        this.sessionManager = sm;
    }

    /**
     * Set the origin of the request for permission checking
     */
    setOrigin(origin: string): void {
        this.origin = origin;
    }

    /**
     * Process a JSON-RPC request and return the response
     */
    async handleRpcRequest(request: JsonRpcRequest): Promise<unknown> {
        try {
            console.log(`[RpcHandler] Processing RPC request: ${request.method}`);

            switch (request.method) {
                case 'eth_accounts':
                case 'eth_requestAccounts':
                    return await this.handleAccountsRequest(request.method);

                case 'eth_chainId':
                    return await this.handleChainIdRequest();

                case 'net_version':
                    return await this.handleNetVersionRequest();

                case 'eth_getBalance':
                    return await this.handleGetBalanceRequest(request.params as unknown[]);

                case 'eth_sendTransaction':
                    return await this.handleSendTransactionRequest(request.params as unknown[]);

                case 'eth_signMessage':
                case 'personal_sign':
                    return await this.handleSignMessageRequest(request.params as unknown[]);

                case 'eth_getTransactionCount':
                    return await this.handleGetTransactionCountRequest(request.params as unknown[]);

                case 'eth_gasPrice':
                    return await this.handleGasPriceRequest();

                case 'eth_estimateGas':
                    return await this.handleEstimateGasRequest(request.params as unknown[]);

                default:
                    // Forward read-only methods to RPC provider
                    if (this.isReadOnlyMethod(request.method)) {
                        return await this.forwardToRpcProvider(request);
                    }
                    throw new Error(`Unsupported method: ${request.method}`);
            }
        } catch (error) {
            console.error(`[RpcHandler] RPC request failed: ${request.method}`, error);
            throw error;
        }
    }

    /**
     * Handle eth_accounts and eth_requestAccounts
     */
    private async handleAccountsRequest(method: string): Promise<string[]> {
        // For eth_accounts, return already connected accounts
        if (method === 'eth_accounts') {
            // If no origin (e.g., from popup), return current account
            if (!this.origin) {
                const currentAccount = this.accountService.getCurrentAccount();
                return currentAccount ? [currentAccount.address] : [];
            }
            
            const connectedAccounts = this.permissionService.getConnectedAccounts(this.origin);
            console.log(`[RpcHandler] eth_accounts for ${this.origin}: ${connectedAccounts.length} connected`);
            return connectedAccounts;
        }

        // For eth_requestAccounts, we need to prompt user for permission
        if (method === 'eth_requestAccounts') {
            // First ensure we have at least one account
            await this.accountService.ensureInitialized();
            let accounts = this.accountService.getAccountsByBlockchain('ethereum');
            
            if (accounts.length === 0) {
                console.log('[RpcHandler] No accounts exist, creating default account');
                const defaultAccount = await this.accountService.ensureDefaultAccount();
                if (!defaultAccount) {
                    throw new Error('Failed to create default account');
                }
                accounts = [defaultAccount];
            }

            // Check if we already have permissions
            const connectedAccounts = this.permissionService.getConnectedAccounts(this.origin);
            if (connectedAccounts.length > 0) {
                console.log(`[RpcHandler] Returning existing connections for ${this.origin}`);
                return connectedAccounts;
            }

            // For now, auto-connect all accounts (in production, show UI selector)
            // TODO: Implement UI account selector
            const accountAddresses = accounts.map(acc => acc.address);
            const currentNetwork = this.networkService.getCurrentNetwork();
            const chainId = currentNetwork ? toHex(currentNetwork.id) : '0x1';
            
            await this.permissionService.connectAccounts(
                this.origin, 
                accountAddresses,
                chainId
            );

            console.log(`[RpcHandler] Connected ${accountAddresses.length} accounts to ${this.origin}`);
            return accountAddresses;
        }

        return [];
    }

    /**
     * Handle eth_chainId request
     */
    private async handleChainIdRequest(): Promise<string> {
        const currentNetwork = this.networkService.getCurrentNetwork();
        if (!currentNetwork) {
            throw new Error('No current network found');
        }
        return toHex(currentNetwork.id);
    }

    /**
     * Handle net_version request
     */
    private async handleNetVersionRequest(): Promise<string> {
        const network = this.networkService.getCurrentNetwork();
        if (!network) {
            throw new Error('No current network found');
        }
        return network.id.toString();
    }

    /**
     * Handle eth_getBalance request
     */
    private async handleGetBalanceRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing address parameter');
        }

        const address = params[0] as string;
        // WalletClientService.getBalance() only takes address as optional parameter
        return await this.walletClientService.getBalance(address);
    }

    /**
     * Handle eth_sendTransaction request
     */
    private async handleSendTransactionRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing transaction parameters');
        }

        const transaction = params[0] as any;

        // Validate transaction parameters
        if (!transaction.to) {
            throw new Error('Invalid transaction parameters: missing "to" address');
        }

        // Generate a unique transaction signing ID
        const signingId = `tx_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        
        // Serialize transaction for MPC signing
        const transactionData = JSON.stringify({
            to: transaction.to,
            value: transaction.value || '0x0',
            data: transaction.data || '0x',
            nonce: transaction.nonce,
            gasLimit: transaction.gas || transaction.gasLimit,
            gasPrice: transaction.gasPrice,
            maxFeePerGas: transaction.maxFeePerGas,
            maxPriorityFeePerGas: transaction.maxPriorityFeePerGas,
            chainId: await this.handleChainIdRequest()
        });

        // Create a promise that will resolve when we get the signed transaction
        const signaturePromise = new Promise<string>((resolve, reject) => {
            if (!this.pendingSignatures) {
                this.pendingSignatures = new Map();
            }
            this.pendingSignatures.set(signingId, { resolve, reject });

            // Set a timeout
            setTimeout(() => {
                if (this.pendingSignatures?.has(signingId)) {
                    this.pendingSignatures.delete(signingId);
                    reject(new Error('Transaction signing timed out'));
                }
            }, 300000); // 5 minute timeout
        });

        // Send transaction signing request to offscreen document
        chrome.runtime.sendMessage({
            type: 'fromBackground',
            payload: {
                type: 'requestTransactionSignature',
                signingId,
                transactionData,
                fromAddress: transaction.from || (this.accountService.getCurrentAccount()?.address)
            }
        });

        // Also notify popup if open for user approval
        chrome.runtime.sendMessage({
            type: 'transactionRequest',
            signingId,
            transaction,
            origin: this.origin || 'Unknown',
            fromAddress: transaction.from || (this.accountService.getCurrentAccount()?.address)
        });

        return signaturePromise;
    }

    /**
     * Ext-4: handle personal_sign / eth_sign by routing the request
     * through the TUI-compatible FROST threshold signing flow.
     *
     * This replaces the legacy `requestMessageSignature` path that
     * asked the offscreen DkgManager to sign single-party over the
     * loaded keystore. That path produced a lone share, never a
     * threshold signature — fine for internal use but useless for
     * dApp `ecrecover` verification since no real aggregation
     * happened.
     *
     * New flow:
     *   1. Parse params (personal_sign: [msg, addr] / eth_sign
     *      reversed — detect by which param looks like an address).
     *   2. Look up the wallet by address via KeystoreManager. Reject
     *      if the user has no wallet for that address (dApp sent a
     *      stale/wrong account).
     *   3. EIP-191 wrap via viem's `hashMessage` so the FROST output
     *      pairs with standard ecrecover (matches the popup's
     *      Sign Message flow exactly).
     *   4. Call sessionManager.createSigningSession — same entrypoint
     *      the popup's manual-sign button uses. Announces on the
     *      signal server; co-signers get notified; ceremony starts
     *      at threshold; stateManager.signingComplete eventually
     *      calls rpcHandler.handleSignatureComplete(sessionId, sig)
     *      to resolve the promise returned here.
     *   5. Popup receives a `signatureRequest` broadcast to surface
     *      the dApp origin to the user (existing UI hook reused for
     *      origin display).
     */
    private async handleSignMessageRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing message parameter');
        }

        if (!this.sessionManager) {
            throw new Error(
                'SessionManager not initialized — cannot route signing request',
            );
        }

        let message: string;
        let address: string;

        // Handle different parameter formats for eth_sign vs personal_sign
        // personal_sign: [message, address]
        // eth_sign: [address, message]
        if (params.length >= 2) {
            const param0 = params[0] as string;
            const param1 = params[1] as string;

            if (param0.startsWith('0x') && param0.length === 42) {
                // eth_sign format: [address, message]
                address = param0;
                message = param1;
            } else {
                // personal_sign format: [message, address]
                message = param0;
                address = param1;
            }
        } else {
            // Single param: message only, use current account.
            message = params[0] as string;
            const currentAccount = this.accountService.getCurrentAccount();
            if (!currentAccount) {
                throw new Error('No account selected');
            }
            address = currentAccount.address;
        }

        // Look up the wallet backing this address. KeystoreManager's
        // wallets carry `address` from the save-wallet flow (Ext-1d).
        // Case-insensitive compare — dApps frequently lowercase
        // addresses but our storage uses checksummed-or-mixed case.
        const { KeystoreManager } = await import('../../services/keystoreManager');
        const km = KeystoreManager.getInstance();
        const wallets = km.getWallets();
        const normalized = address.toLowerCase();
        const wallet = wallets.find(
            (w: any) => typeof w.address === 'string' && w.address.toLowerCase() === normalized,
        );
        if (!wallet) {
            throw new Error(
                `No wallet found for address ${address}. Unlock a wallet in the extension that owns this address before signing.`,
            );
        }

        const blockchain: 'ethereum' | 'solana' =
            (wallet as any).blockchain === 'solana' ? 'solana' : 'ethereum';

        // Pull threshold/total/groupPublicKey from the KeyShareData.
        const keyShare = (km as any).getKeyShareData?.((wallet as any).id);
        const groupPublicKey =
            keyShare?.group_public_key ?? (wallet as any).group_public_key ?? '';
        const threshold = keyShare?.threshold ?? 2;
        const total = keyShare?.total_participants ?? 3;

        // EIP-191 wrap for secp256k1, raw UTF-8 bytes for ed25519.
        // Mirrors handleCreateSigningSessionRequest in messageHandlers.
        let messageHex: string;
        if (blockchain === 'ethereum') {
            const { hashMessage } = await import('viem');
            // `message` may already be 0x-hex bytes (dApp convention)
            // or raw UTF-8 text. hashMessage accepts string and treats
            // 0x-prefixed hex as raw bytes only if wrapped in `{raw}`.
            // For personal_sign, the standard treats the param as raw
            // bytes if 0x-prefixed. Match that:
            const isHex = /^0x[0-9a-fA-F]*$/.test(message);
            const hash = isHex
                ? hashMessage({ raw: message as `0x${string}` })
                : hashMessage(message);
            messageHex = hash.startsWith('0x') ? hash.slice(2) : hash;
        } else {
            const encoder = new TextEncoder();
            const bytes = encoder.encode(message);
            messageHex = Array.from(bytes, (b) =>
                b.toString(16).padStart(2, '0'),
            ).join('');
        }

        // Ext-4-confirm: DO NOT announce the session yet. A dApp
        // RPC arriving at the extension MUST gate on explicit user
        // approval before any signing session hits the signal
        // server — otherwise a malicious dApp could trigger
        // notifications / offscreen wake on all co-signer devices
        // just by calling personal_sign. Stash the context, show
        // the request in the popup, and only call createSigningSession
        // when the user clicks Approve (approveDappSignature below).
        const requestId = `dapp_req_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
        this.pendingDappRequests.set(requestId, {
            walletId: (wallet as any).id,
            walletName: (wallet as any).name ?? (wallet as any).id,
            groupPublicKey,
            blockchain,
            threshold,
            total,
            messageHex,
            originalMessage: message,
            address,
            origin: this.origin || 'Unknown',
        });

        // Surface the request in the popup so users see the dApp
        // origin before approving. SignatureRequest.svelte already
        // renders this shape + dispatches `approveMessageSignature`
        // when the user picks Sign or Reject.
        chrome.runtime.sendMessage({
            type: 'signatureRequest',
            signingId: requestId,
            message,
            origin: this.origin || 'Unknown',
            fromAddress: address,
        });

        // Fire a desktop notification so users know to open the
        // popup. Without this, a signature request arriving while
        // the popup is closed is silent — the dApp would spin
        // until timeout with no user-facing signal.
        if (typeof chrome !== 'undefined' && chrome.notifications) {
            try {
                chrome.notifications.create(`mpc-dapp-sig:${requestId}`, {
                    type: 'basic',
                    iconUrl: 'icon/128.png',
                    title: 'Signature requested',
                    message: `${this.origin || 'A dApp'} wants you to sign a message with ${address.slice(0, 6)}…${address.slice(-4)}`,
                    priority: 2,
                    requireInteraction: true,
                });
            } catch (e) {
                console.warn('[RpcHandler] Failed to create notification:', e);
            }
        }

        // Promise resolves (from handleSignatureComplete) when the
        // FROST ceremony that the popup kicks off on approval
        // produces the aggregated signature. On rejection, the
        // popup's handler calls handleSignatureError which rejects
        // this promise. 5-min timeout catches abandoned requests
        // (user ignored popup + never approved).
        return new Promise<string>((resolve, reject) => {
            this.pendingSignatures.set(requestId, { resolve, reject });
            setTimeout(() => {
                if (this.pendingSignatures.has(requestId)) {
                    this.pendingSignatures.delete(requestId);
                    this.pendingDappRequests.delete(requestId);
                    reject(new Error('Signature request timed out'));
                }
            }, 300_000);
        });
    }

    /**
     * Ext-4-confirm: called from messageHandlers.handleApproveMessageSignature
     * after the user clicks Sign/Reject in the popup's SignatureRequest
     * component. This is where the actual createSigningSession call
     * happens — deferred from handleSignMessageRequest so users get
     * to review origin + message first.
     *
     * On approve:
     *   - Pull stashed context from pendingDappRequests.
     *   - Call sessionManager.createSigningSession (real announce).
     *   - Re-key the pending promise from the placeholder requestId
     *     to the actual session_id, so when stateManager.signingComplete
     *     later fires with that session_id, handleSignatureComplete
     *     finds the right pending entry.
     *
     * On reject:
     *   - Reject the pending promise with "User rejected".
     *   - Clean up pendingDappRequests.
     */
    async approveDappSignature(
        requestId: string,
        approved: boolean,
    ): Promise<{ success: boolean; error?: string; sessionId?: string }> {
        const context = this.pendingDappRequests.get(requestId);
        if (!context) {
            return {
                success: false,
                error: `No pending dApp signature request ${requestId}`,
            };
        }
        this.pendingDappRequests.delete(requestId);
        // Cancel any pending notification — user engaged with the
        // popup, no need to keep the OS banner up.
        if (typeof chrome !== 'undefined' && chrome.notifications) {
            try {
                chrome.notifications.clear(`mpc-dapp-sig:${requestId}`);
            } catch {
                /* non-fatal */
            }
        }

        if (!approved) {
            this.handleSignatureError(requestId, 'User rejected signature request');
            return { success: true };
        }

        if (!this.sessionManager) {
            this.handleSignatureError(
                requestId,
                'SessionManager not initialized',
            );
            return { success: false, error: 'SessionManager not initialized' };
        }

        const result = await this.sessionManager.createSigningSession({
            walletId: context.walletId,
            walletName: context.walletName,
            groupPublicKey: context.groupPublicKey,
            blockchain: context.blockchain,
            threshold: context.threshold,
            total: context.total,
            signingMessageHex: context.messageHex,
        });
        if (!result.success || !result.sessionId) {
            this.handleSignatureError(
                requestId,
                result.error ?? 'Failed to create signing session',
            );
            return {
                success: false,
                error: result.error ?? 'Failed to create signing session',
            };
        }

        // Re-key the pending promise from the placeholder requestId
        // to the actual session id. signingComplete from stateManager
        // will arrive with the session id, not the requestId.
        const pending = this.pendingSignatures.get(requestId);
        if (pending) {
            this.pendingSignatures.delete(requestId);
            this.pendingSignatures.set(result.sessionId, pending);
        }
        return { success: true, sessionId: result.sessionId };
    }

    /**
     * Handle eth_getTransactionCount request
     */
    private async handleGetTransactionCountRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing address parameter');
        }

        const address = params[0] as string;
        // WalletClientService.getTransactionCount() returns number, convert to string
        const count = await this.walletClientService.getTransactionCount(address);
        return count.toString();
    }

    /**
     * Handle eth_gasPrice request
     */
    private async handleGasPriceRequest(): Promise<string> {
        return await this.walletClientService.getGasPrice();
    }

    /**
     * Handle eth_estimateGas request
     */
    private async handleEstimateGasRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing transaction parameters');
        }

        const transaction = params[0] as any;
        return await this.walletClientService.estimateGas(transaction);
    }

    /**
     * Generic RPC request method that can be used by other modules
     */
    async makeRpcRequest(request: JsonRpcRequest): Promise<unknown> {
        // For now, just delegate to handleRpcRequest
        return this.handleRpcRequest(request);
    }

    /**
     * Check if a method is read-only and should be forwarded to RPC provider
     */
    private isReadOnlyMethod(method: string): boolean {
        const readOnlyMethods = [
            'eth_blockNumber',
            'eth_getBlockByHash',
            'eth_getBlockByNumber',
            'eth_getTransactionByHash',
            'eth_getTransactionReceipt',
            'eth_getBlockTransactionCountByHash',
            'eth_getBlockTransactionCountByNumber',
            'eth_getUncleCountByBlockHash',
            'eth_getUncleCountByBlockNumber',
            'eth_getCode',
            'eth_call',
            'eth_getLogs',
            'eth_getFilterChanges',
            'eth_getFilterLogs',
            'eth_newFilter',
            'eth_newBlockFilter',
            'eth_newPendingTransactionFilter',
            'eth_uninstallFilter',
            'eth_getStorageAt',
            'eth_getProof',
            'eth_feeHistory',
            'eth_maxPriorityFeePerGas'
        ];
        
        return readOnlyMethods.includes(method);
    }

    /**
     * Forward RPC request to the network's RPC provider
     */
    private async forwardToRpcProvider(request: JsonRpcRequest): Promise<any> {
        try {
            const network = this.networkService.getCurrentNetwork();
            if (!network || !network.rpcUrls?.default?.http?.[0]) {
                throw new Error('No RPC URL available for current network');
            }

            const rpcUrl = network.rpcUrls.default.http[0];
            console.log(`[RpcHandler] Forwarding ${request.method} to RPC provider: ${rpcUrl}`);

            const response = await fetch(rpcUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    id: request.id || 1,
                    method: request.method,
                    params: request.params || []
                })
            });

            if (!response.ok) {
                throw new Error(`RPC request failed with status ${response.status}`);
            }

            const result = await response.json();
            
            if (result.error) {
                throw new Error(result.error.message || 'RPC request failed');
            }

            return result.result;
        } catch (error) {
            console.error(`[RpcHandler] Failed to forward to RPC provider:`, error);
            throw error;
        }
    }

    /**
     * Handle signature completion from either the legacy
     * `messageSignatureComplete` path (offscreen DkgManager output,
     * keyed by `msg_<ts>_<nonce>`) or the Ext-4 `signingComplete`
     * path (FROST aggregated signature, keyed by `sign_<hex>`
     * session id). Normalizes the hex to `0x...` before resolving
     * so dApp callers can feed it straight into ecrecover /
     * verifyMessage without additional formatting.
     */
    handleSignatureComplete(signingId: string, signature: string): void {
        const pending = this.pendingSignatures.get(signingId);
        if (pending) {
            const normalized = signature.startsWith('0x')
                ? signature
                : `0x${signature}`;
            pending.resolve(normalized);
            this.pendingSignatures.delete(signingId);
        }
    }

    /**
     * Handle signature error from offscreen document
     */
    handleSignatureError(signingId: string, error: string): void {
        const pending = this.pendingSignatures.get(signingId);
        if (pending) {
            pending.reject(new Error(error));
            this.pendingSignatures.delete(signingId);
        }
    }
}

/**
 * Handles UI requests from the popup interface
 */
export class UIRequestHandler {
    private walletController: WalletController;

    constructor() {
        this.walletController = WalletController.getInstance();
    }

    /**
     * Handle UI requests from popup
     */
    async handleUIRequest(request: { method: string; params: unknown[] }): Promise<{ success: boolean; data?: unknown; error?: string }> {
        const { method, params } = request;

        console.log(`[UIRequestHandler] Processing UI request: ${method}`);

        if (typeof this.walletController[method as keyof WalletController] === 'function') {
            try {
                const result = await (this.walletController[method as keyof WalletController] as (...args: unknown[]) => unknown)(...params);
                return { success: true, data: result };
            } catch (error) {
                console.error(`[UIRequestHandler] UI request failed: ${method}`, error);
                return { success: false, error: error instanceof Error ? error.message : 'Unknown error' };
            }
        }

        return { success: false, error: `Method ${method} not found on WalletController` };
    }
}
