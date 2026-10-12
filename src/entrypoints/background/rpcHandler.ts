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
import { toHex } from 'viem';
import type { JsonRpcRequest } from "../../types/messages";
import { match } from "ts-pattern";

/**
 * Handles JSON-RPC requests from web applications
 */
export class RpcHandler {
    private accountService: AccountService;
    private networkService: NetworkService;
    private walletClientService: WalletClientService;

    constructor() {
        this.accountService = AccountService.getInstance();
        this.networkService = NetworkService.getInstance();
        this.walletClientService = WalletClientService.getInstance();
    }

    /**
     * Process a JSON-RPC request and return the response
     */
    async handleRpcRequest(request: JsonRpcRequest): Promise<unknown> {
        try {
            console.log(`[RpcHandler] Processing RPC request: ${request.method}`);

            return await match(request.method)
                .with('eth_accounts', async () => await this.handleAccountsRequest(request.method))
                .with('eth_requestAccounts', async () => await this.handleAccountsRequest(request.method))
                .with('eth_chainId', async () => await this.handleChainIdRequest())
                .with('net_version', async () => await this.handleNetVersionRequest())
                .with('eth_getBalance', async () => await this.handleGetBalanceRequest(request.params as unknown[]))
                .with('eth_sendTransaction', async () => await this.handleSendTransactionRequest(request.params as unknown[]))
                .with('eth_signMessage', async () => await this.handleSignMessageRequest(request.params as unknown[]))
                .with('personal_sign', async () => await this.handleSignMessageRequest(request.params as unknown[]))
                .with('eth_getTransactionCount', async () => await this.handleGetTransactionCountRequest(request.params as unknown[]))
                .with('eth_gasPrice', async () => await this.handleGasPriceRequest())
                .with('eth_estimateGas', async () => await this.handleEstimateGasRequest(request.params as unknown[]))
                .otherwise(() => {
                    throw new Error(`Unsupported method: ${request.method}`);
                });
        } catch (error) {
            console.error(`[RpcHandler] RPC request failed: ${request.method}`, error);
            throw error;
        }
    }

    /**
     * Handle eth_accounts and eth_requestAccounts
     */
    private async handleAccountsRequest(method: string): Promise<string[]> {
        // Try to get current account
        let currentAccount = this.accountService.getCurrentAccount();

        // If no account exists and user requested accounts, try to create a default one
        if (!currentAccount && method === 'eth_requestAccounts') {
            console.log('[RpcHandler] No account selected, attempting to create default account');
            currentAccount = await this.accountService.ensureDefaultAccount();
        }

        // If we still don't have an account, handle appropriately
        if (!currentAccount) {
            if (method === 'eth_requestAccounts') {
                // For eth_requestAccounts, this is considered an error
                throw new Error('No accounts available. Please create an account first.');
            } else {
                // For eth_accounts, returning an empty array is acceptable
                return [];
            }
        }

        return [currentAccount.address];
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
        if (!transaction.to || !transaction.value) {
            throw new Error('Invalid transaction parameters');
        }

        // Use wallet client service to send transaction
        return await this.walletClientService.sendTransaction(transaction);
    }

    /**
     * Handle message signing requests
     */
    private async handleSignMessageRequest(params: unknown[]): Promise<string> {
        if (!params || params.length < 1) {
            throw new Error('Missing message parameter');
        }

        const message = params[0] as string;
        // WalletClientService.signMessage() only takes message parameter
        return await this.walletClientService.signMessage(message);
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
