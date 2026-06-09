// ===================================================================
// PERMISSION SERVICE - DAPP CONNECTION MANAGEMENT
// ===================================================================
//
// This service manages which accounts are connected to which dApps,
// following the EIP-6963 multi-account pattern. It stores permissions
// persistently and provides methods to manage account connections.
// ===================================================================

import { storage } from "#imports";

export interface DAppPermission {
    origin: string;
    connectedAccounts: string[]; // array of account addresses
    lastConnected: number;
    chainId: string;
    name?: string; // dApp name if available
    icon?: string; // dApp icon if available
}

export class PermissionService {
    private static instance: PermissionService;
    private permissions: Map<string, DAppPermission> = new Map();
    private readonly STORAGE_KEY = "starlab_mpc_dapp_permissions";
    private initialized: boolean = false;
    private initPromise: Promise<void> | null = null;
    
    private constructor() {
        this.initPromise = this.loadPermissions();
    }
    
    public static getInstance(): PermissionService {
        if (!PermissionService.instance) {
            PermissionService.instance = new PermissionService();
        }
        return PermissionService.instance;
    }
    
    /**
     * Ensure the service is initialized before use
     */
    public async ensureInitialized(): Promise<void> {
        if (this.initPromise) {
            await this.initPromise;
            this.initPromise = null;
        }
    }
    
    /**
     * Load permissions from storage
     */
    private async loadPermissions(): Promise<void> {
        try {
            const stored = await storage.getItem<Record<string, DAppPermission>>(
                `local:${this.STORAGE_KEY}`
            );
            
            if (stored) {
                this.permissions = new Map(Object.entries(stored));
                console.log("[PermissionService] Loaded permissions for", this.permissions.size, "dApps");
            }
            this.initialized = true;
        } catch (error) {
            console.error("[PermissionService] Error loading permissions:", error);
            this.initialized = true; // Mark as initialized even on error
        }
    }
    
    /**
     * Save permissions to storage
     */
    private async savePermissions(): Promise<void> {
        try {
            const permissionsObj = Object.fromEntries(this.permissions);
            await storage.setItem(`local:${this.STORAGE_KEY}`, permissionsObj);
            console.log("[PermissionService] Saved permissions");
        } catch (error) {
            console.error("[PermissionService] Error saving permissions:", error);
        }
    }
    
    /**
     * Get connected accounts for a specific origin
     */
    public getConnectedAccounts(origin: string): string[] {
        const permission = this.permissions.get(origin);
        return permission?.connectedAccounts || [];
    }
    
    /**
     * Check if an account is connected to an origin
     */
    public isAccountConnected(origin: string, accountAddress: string): boolean {
        const connectedAccounts = this.getConnectedAccounts(origin);
        return connectedAccounts.includes(accountAddress.toLowerCase());
    }
    
    /**
     * Connect accounts to an origin
     */
    public async connectAccounts(
        origin: string, 
        accounts: string[], 
        chainId: string,
        dAppInfo?: { name?: string; icon?: string }
    ): Promise<void> {
        // Don't store permissions for null/undefined origins (e.g., from popup)
        if (!origin) {
            console.log("[PermissionService] Skipping permission storage for null origin");
            return;
        }
        
        const normalizedAccounts = accounts.map(a => a.toLowerCase());
        const existing = this.permissions.get(origin);
        
        if (existing) {
            // Replace existing accounts (this is the standard behavior for wallet connections)
            existing.connectedAccounts = normalizedAccounts;
            existing.lastConnected = Date.now();
            existing.chainId = chainId;
            if (dAppInfo?.name) existing.name = dAppInfo.name;
            if (dAppInfo?.icon) existing.icon = dAppInfo.icon;
        } else {
            // Create new permission
            this.permissions.set(origin, {
                origin,
                connectedAccounts: normalizedAccounts,
                lastConnected: Date.now(),
                chainId,
                ...dAppInfo
            });
        }
        
        await this.savePermissions();
        console.log("[PermissionService] Connected accounts for", origin, ":", normalizedAccounts);
    }
    
    /**
     * Add additional accounts to an existing permission
     */
    public async addAccountsToPermission(
        origin: string, 
        accounts: string[], 
        chainId: string
    ): Promise<void> {
        if (!origin) {
            return;
        }
        
        const normalizedAccounts = accounts.map(a => a.toLowerCase());
        const existing = this.permissions.get(origin);
        
        if (existing) {
            // Merge with existing accounts (deduplicate)
            const merged = new Set([...existing.connectedAccounts, ...normalizedAccounts]);
            existing.connectedAccounts = Array.from(merged);
            existing.lastConnected = Date.now();
            existing.chainId = chainId;
        } else {
            // If no existing permission, create new one
            this.permissions.set(origin, {
                origin,
                connectedAccounts: normalizedAccounts,
                lastConnected: Date.now(),
                chainId
            });
        }
        
        await this.savePermissions();
        console.log("[PermissionService] Added accounts to", origin, ":", normalizedAccounts);
    }
    
    /**
     * Disconnect accounts from an origin
     */
    public async disconnectAccounts(origin: string, accounts?: string[]): Promise<void> {
        if (!accounts) {
            // Disconnect all accounts
            this.permissions.delete(origin);
        } else {
            const permission = this.permissions.get(origin);
            if (permission) {
                const normalizedAccounts = accounts.map(a => a.toLowerCase());
                permission.connectedAccounts = permission.connectedAccounts.filter(
                    account => !normalizedAccounts.includes(account)
                );
                
                if (permission.connectedAccounts.length === 0) {
                    this.permissions.delete(origin);
                }
            }
        }
        
        await this.savePermissions();
        console.log("[PermissionService] Disconnected accounts for", origin);
    }
    
    /**
     * Get all permissions
     */
    public getAllPermissions(): DAppPermission[] {
        return Array.from(this.permissions.values());
    }
    
    /**
     * Get all connected origins for an account
     */
    public getConnectedDApps(accountAddress: string): DAppPermission[] {
        const normalized = accountAddress.toLowerCase();
        return Array.from(this.permissions.values()).filter(
            permission => permission.connectedAccounts.includes(normalized)
        );
    }
    
    /**
     * Clear all permissions
     */
    public async clearAllPermissions(): Promise<void> {
        this.permissions.clear();
        await this.savePermissions();
        console.log("[PermissionService] Cleared all permissions");
    }
    
    /**
     * Update chain ID for an origin
     */
    public async updateChainId(origin: string, chainId: string): Promise<void> {
        const permission = this.permissions.get(origin);
        if (permission) {
            permission.chainId = chainId;
            await this.savePermissions();
        }
    }
    
    /**
     * Add a single account to an existing permission
     */
    public async addAccount(origin: string, account: string, chainId: string): Promise<void> {
        await this.addAccountsToPermission(origin, [account], chainId);
    }
    
    /**
     * Disconnect a single account from an origin
     */
    public async disconnectAccount(origin: string, account: string): Promise<void> {
        await this.disconnectAccounts(origin, [account]);
    }
}

// Export singleton instance getter
export const getPermissionService = (): PermissionService => {
    return PermissionService.getInstance();
};