// ===================================================================
// SIMPLIFIED OFFSCREEN MANAGEMENT MODULE
// ===================================================================
//
// Streamlined offscreen document manager with comprehensive logging:
// - Simple create-once-and-wait approach
// - Enhanced message routing with categorization
// - Performance tracking and visual logging
// - Simplified error handling
// ===================================================================

import type {
    BackgroundToOffscreenWrapper,
    OffscreenMessage
} from "../../types/messages";
import { AppState } from "../../types/appstate";
import { match } from "ts-pattern";

/**
 * Simplified offscreen document manager with enhanced logging
 */
export class OffscreenManager {
    private offscreenReady = false;
    private appState: AppState;
    private messageCount = 0;

    constructor(appState: AppState) {
        this.appState = appState;
        console.log("🖥️ [OffscreenManager] Initialized with simple create-and-wait strategy");
        console.log("🖥️ [OffscreenManager] Constructor received appState:", appState);
        console.log("🖥️ [OffscreenManager] Constructor appState.deviceId:", appState.deviceId);
        console.log("🖥️ [OffscreenManager] Constructor appState reference type:", typeof appState);
        console.log("🖥️ [OffscreenManager] Constructor appState keys:", Object.keys(appState));
    }

    /**
     * Get message category for logging
     */
    private getMessageCategory(message: OffscreenMessage): string {
        const type = message.type;
        return match(type)
            .with('init', () => '🔧 Init')
            .with('getState', () => '📊 Get State')
            .with('sendDirectMessage', () => '📨 Direct Message')
            .with('getWebRTCStatus', () => '📡 WebRTC Status')
            .with('relayViaWs', () => '🔄 WS Relay')
            .with('sessionAccepted', () => '✅ Session Accepted')
            .with('sessionAllAccepted', () => '🎉 All Sessions Accepted')
            .with('sessionResponseUpdate', () => '🔄 Session Update')
            .with('getEthereumAddress', () => '💰 ETH Address')
            .with('getSolanaAddress', () => '🔮 SOL Address')
            .with('getDkgStatus', () => '🔐 DKG Status')
            .with('getGroupPublicKey', () => '🔑 Group Key')
            .with('setBlockchain', () => '⛓️ Set Blockchain')
            .otherwise(() => '📝 Message');
    }

    /**
     * Create offscreen document - simple approach
     */
    async createOffscreenDocument(): Promise<{ success: boolean; error?: string }> {
        if (!chrome.offscreen) {
            console.error("❌ [OffscreenManager] Offscreen API not available");
            return { success: false, error: "Offscreen API not available" };
        }

        // Check if document already exists
        if (await chrome.offscreen.hasDocument()) {
            console.log("✅ [OffscreenManager] Offscreen document already exists");
            return { success: true };
        }

        try {
            const startTime = performance.now();
            console.log("🔄 [OffscreenManager] Creating offscreen document...");

            await chrome.offscreen.createDocument({
                url: chrome.runtime.getURL('offscreen.html'),
                reasons: [chrome.offscreen.Reason.DOM_SCRAPING],
                justification: 'Manages WebRTC connections and signaling for MPC sessions using DOM capabilities.',
            });

            const duration = performance.now() - startTime;
            console.log(`✅ [OffscreenManager] Document created successfully (${duration.toFixed(2)}ms)`);
            return { success: true };
        } catch (error: any) {
            if (error.message?.includes("Only a single offscreen document may be created")) {
                console.log("✅ [OffscreenManager] Document already exists (creation conflict)");
                return { success: true };
            }
            console.error("❌ [OffscreenManager] Creation failed:", error);
            return { success: false, error: error.message };
        }
    }

    /**
     * Send message to offscreen with enhanced logging
     */
    async sendToOffscreen(message: OffscreenMessage, description?: string): Promise<{ success: boolean; error?: string }> {
        const messageId = ++this.messageCount;
        const category = this.getMessageCategory(message);
        const desc = description || message.type;

        if (!this.offscreenReady) {
            console.warn(`⚠️ [OffscreenManager] ${category} blocked: offscreen not ready (${desc})`);
            return { success: false, error: "Offscreen document not ready" };
        }

        if (!chrome.offscreen || !await chrome.offscreen.hasDocument()) {
            console.warn(`⚠️ [OffscreenManager] ${category} blocked: no document (${desc})`);
            return { success: false, error: "Offscreen document does not exist" };
        }

        try {
            const startTime = performance.now();
            const wrappedMessage: BackgroundToOffscreenWrapper = {
                type: "fromBackground",
                payload: message
            };

            console.log(`🚀 [OffscreenManager #${messageId}] ${category} → ${desc}`);
            await chrome.runtime.sendMessage(wrappedMessage);

            const duration = performance.now() - startTime;
            console.log(`✅ [OffscreenManager #${messageId}] ${category} sent (${duration.toFixed(2)}ms)`);
            return { success: true };
        } catch (error: any) {
            console.error(`❌ [OffscreenManager #${messageId}] ${category} failed:`, error);
            return { success: false, error: error.message };
        }
    }

    /**
     * Handle offscreen ready signal - simplified
     */
    handleOffscreenReady(): void {
        console.log("🎉 [OffscreenManager] Offscreen document ready - message routing enabled");
        this.offscreenReady = true;
    }

    /**
     * Send initialization data to offscreen
     */
    async sendInitData(deviceId: string, wsUrl: string = "wss://auto-life.tech"): Promise<{ success: boolean; error?: string }> {
        console.log(`🔧 [OffscreenManager] Initializing offscreen with deviceId: ${deviceId}`);
        return await this.sendToOffscreen({
            type: "init",
            deviceId,
            wsUrl
        }, `init(${deviceId})`);
    }

    /**
     * Handle initialization request from offscreen
     */
    async handleInitRequest(): Promise<{ success: boolean; message?: string; error?: string }> {
        console.log("🔧 [OffscreenManager] Processing init request from offscreen");
        
        // Enhanced debugging for device ID state
        console.log("🔍 [OffscreenManager] Current appState reference:", this.appState);
        console.log("🔍 [OffscreenManager] AppState deviceId:", this.appState.deviceId);
        console.log("🔍 [OffscreenManager] AppState type:", typeof this.appState);
        console.log("🔍 [OffscreenManager] AppState keys:", Object.keys(this.appState));

        if (!this.appState.deviceId) {
            console.warn("⚠️ [OffscreenManager] Init request failed: no device ID");
            console.warn("⚠️ [OffscreenManager] Complete state object:", JSON.stringify(this.appState, null, 2));
            return { success: false, error: "No device ID available" };
        }

        console.log("✅ [OffscreenManager] Device ID found, proceeding with initialization");
        const result = await this.sendInitData(this.appState.deviceId);
        return result.success
            ? { success: true, message: "Init data sent" }
            : { success: false, error: result.error };
    }

    /**
     * Get current offscreen status
     */
    async getOffscreenStatus(): Promise<{ hasDocument: boolean; ready: boolean }> {
        const hasDocument = chrome.offscreen ? await chrome.offscreen.hasDocument() : false;
        return { hasDocument, ready: this.offscreenReady };
    }

    // Simple getters
    get isReady(): boolean {
        return this.offscreenReady;
    }
}
