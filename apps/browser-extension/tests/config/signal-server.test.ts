/**
 * Regression tests for Ext-0: centralized signal-server URL config.
 * Guards against hardcoded signal-server URLs silently reappearing, and
 * the default drifting away from the TUI's URL (which would re-break
 * interop).
 */
import { describe, it, expect, beforeEach } from 'bun:test';
import {
    DEFAULT_SIGNAL_SERVER_URL,
    SIGNAL_SERVER_STORAGE_KEY,
    getSignalServerUrl,
    setSignalServerUrl,
    mergeRoom,
    isValidRoom,
    newRoom,
    getRoom,
    setRoom,
} from '../../src/config/signal-server';

describe('Signal server config', () => {
    beforeEach(async () => {
        await chrome.storage.local.clear();
    });

    it('default matches the TUI URL', () => {
        // If this assertion fails, check `apps/tui-node/src/elm/model.rs`
        // — if the TUI moved its default, we probably need to move too.
        expect(DEFAULT_SIGNAL_SERVER_URL).toBe('wss://panda.qzz.io');
    });

    it('returns default when storage is unset', async () => {
        const url = await getSignalServerUrl();
        expect(url).toBe(DEFAULT_SIGNAL_SERVER_URL);
    });

    it('returns override when set', async () => {
        await chrome.storage.local.set({
            [SIGNAL_SERVER_STORAGE_KEY]: 'wss://custom.example.org',
        });
        const url = await getSignalServerUrl();
        expect(url).toBe('wss://custom.example.org');
    });

    it('falls back to default when override is empty string', async () => {
        await chrome.storage.local.set({ [SIGNAL_SERVER_STORAGE_KEY]: '' });
        const url = await getSignalServerUrl();
        expect(url).toBe(DEFAULT_SIGNAL_SERVER_URL);
    });

    it('setSignalServerUrl writes a valid wss URL', async () => {
        const ok = await setSignalServerUrl('wss://new.example.org');
        expect(ok).toBe(true);
        const stored = await chrome.storage.local.get(SIGNAL_SERVER_STORAGE_KEY);
        expect(stored[SIGNAL_SERVER_STORAGE_KEY]).toBe('wss://new.example.org');
    });

    it('setSignalServerUrl accepts plain ws:// for localhost dev', async () => {
        const ok = await setSignalServerUrl('ws://localhost:8080');
        expect(ok).toBe(true);
    });

    it('setSignalServerUrl rejects http(s):// schemes', async () => {
        const ok = await setSignalServerUrl('https://example.org');
        expect(ok).toBe(false);
        const stored = await chrome.storage.local.get(SIGNAL_SERVER_STORAGE_KEY);
        expect(stored[SIGNAL_SERVER_STORAGE_KEY]).toBeUndefined();
    });

    it('setSignalServerUrl rejects arbitrary strings', async () => {
        const ok = await setSignalServerUrl('not a url');
        expect(ok).toBe(false);
    });
});

describe('Tenant room (multi-tenant #31)', () => {
    beforeEach(async () => {
        await chrome.storage.local.clear();
    });

    it('mergeRoom appends room as a query param', () => {
        expect(mergeRoom('wss://h', 'abcdefghij012345')).toBe('wss://h/?room=abcdefghij012345');
        expect(mergeRoom('wss://h/p', 'r')).toBe('wss://h/p?room=r');
        expect(mergeRoom('wss://h/?x=1', 'r')).toBe('wss://h/?x=1&room=r');
        expect(mergeRoom('wss://h/?room=keep', 'r')).toBe('wss://h/?room=keep'); // already present
        expect(mergeRoom('wss://h', null)).toBe('wss://h'); // no room → unchanged
    });

    it('isValidRoom requires >=16 safe chars', () => {
        expect(isValidRoom('acme')).toBe(false);
        expect(isValidRoom('')).toBe(false);
        expect(isValidRoom('bad room!!!!!!!!!')).toBe(false); // space/!
        expect(isValidRoom('a'.repeat(16))).toBe(true);
        expect(isValidRoom('7f3a9c2e-4b1d-4e8a-9c2f-001122334455')).toBe(true);
    });

    it('newRoom produces an accepted room', () => {
        expect(isValidRoom(newRoom())).toBe(true);
    });

    it('getSignalServerUrl merges a stored room', async () => {
        await setRoom('7f3a9c2e-4b1d-4e8a-9c2f-001122334455');
        expect(await getSignalServerUrl()).toBe(
            'wss://panda.qzz.io/?room=7f3a9c2e-4b1d-4e8a-9c2f-001122334455',
        );
    });

    it('setRoom rejects weak rooms', async () => {
        expect(await setRoom('acme')).toBe(false);
        expect(await getRoom()).toBeNull();
    });

    it('no room → URL has none (server will reject)', async () => {
        expect(await getSignalServerUrl()).toBe('wss://panda.qzz.io');
    });
});
