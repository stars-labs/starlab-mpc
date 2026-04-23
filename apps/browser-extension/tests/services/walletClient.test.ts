import { describe, it, expect, beforeEach, afterEach, mock } from 'bun:test';
import { jest } from 'bun:test';
import WalletClientService from '../../src/services/walletClient';
import AccountService from '../../src/services/accountService';
import NetworkService from '../../src/services/networkService';

// Mock the viem module
mock.module('viem', () => ({
  createWalletClient: jest.fn(() => ({
    account: { address: '0x123' },
    signMessage: jest.fn().mockResolvedValue('0xsignature'),
    sendTransaction: jest.fn().mockResolvedValue('0xtxhash'),
    getBalance: jest.fn().mockResolvedValue(BigInt('1000000000000000000')),
    getTransactionCount: jest.fn().mockResolvedValue(5)
  })),
  createPublicClient: jest.fn(() => ({
    getBalance: jest.fn().mockResolvedValue(BigInt('1000000000000000000')),
    getTransactionCount: jest.fn().mockResolvedValue(5)
  })),
  http: jest.fn(() => ({})),
  custom: jest.fn(() => ({}))
}));

// Intentionally not mocking viem/chains — it would bleed into other
// test files (bun mock.module is process-global) and starve
// networkService.test.ts of sepolia/polygon/arbitrum + nativeCurrency
// fields. The real viem/chains is cheap to import and has no side
// effects.

describe('WalletClientService', () => {
  let walletClient: WalletClientService;
  let mockAccountService: any;
  let mockNetworkService: any;

  beforeEach(() => {
    // Reset singleton
    (WalletClientService as any).instance = null;

    // Create mock services
    mockAccountService = {
      getCurrentAccount: jest.fn(),
      getAccountById: jest.fn(),
      onAccountChange: jest.fn()
    };

    mockNetworkService = {
      getCurrentNetwork: jest.fn(),
      getNetworkById: jest.fn(),
      onNetworkChange: jest.fn()
    };

    // Mock the getInstance methods
    jest.spyOn(AccountService, 'getInstance').mockReturnValue(mockAccountService);
    jest.spyOn(NetworkService, 'getInstance').mockReturnValue(mockNetworkService);

    walletClient = WalletClientService.getInstance();
  });

  afterEach(() => {
    // Restore AccountService/NetworkService.getInstance — spies installed
    // on class statics leak across test files (clearAllMocks resets call
    // history but not implementations). Without this, subsequent files
    // that call AccountService.getInstance() get our test's mock object
    // instead of the real singleton.
    jest.restoreAllMocks();
    (WalletClientService as any).instance = null;
  });

  describe('Initialization', () => {
    it('should create singleton instance', () => {
      const instance1 = WalletClientService.getInstance();
      const instance2 = WalletClientService.getInstance();
      expect(instance1).toBe(instance2);
    });
  });

  describe('Connection Management', () => {
    it('should connect with valid account and network', async () => {
      const mockAccount = {
        id: '1',
        address: '0x123',
        name: 'Test Account',
        blockchain: 'ethereum'
      };

      const mockNetwork = {
        id: 1,
        name: 'Ethereum',
        rpcUrls: {
          default: {
            http: ['https://eth.merkle.io']
          }
        }
      };

      mockAccountService.getCurrentAccount.mockReturnValue(mockAccount);
      mockNetworkService.getCurrentNetwork.mockReturnValue(mockNetwork);

      const result = await walletClient.connect();

      expect(result).toEqual({ connected: true });
      expect(await walletClient.isConnected()).toBe(true);
    });

    it('should still connect even when no account available', async () => {
      mockAccountService.getCurrentAccount.mockReturnValue(null);

      const result = await walletClient.connect();
      expect(result).toEqual({ connected: true });
    });

    it('should still connect even when no network available', async () => {
      const mockAccount = {
        id: '1',
        address: '0x123',
        name: 'Test Account',
        blockchain: 'ethereum'
      };

      mockAccountService.getCurrentAccount.mockReturnValue(mockAccount);
      mockNetworkService.getCurrentNetwork.mockReturnValue(null);

      const result = await walletClient.connect();
      expect(result).toEqual({ connected: true });
    });
  });

  describe('Wallet Operations', () => {
    beforeEach(async () => {
      const mockAccount = {
        id: '1',
        address: '0x123',
        name: 'Test Account',
        blockchain: 'ethereum'
      };

      const mockNetwork = {
        id: 1,
        name: 'Ethereum',
        rpcUrls: {
          default: {
            http: ['https://eth.merkle.io']
          }
        }
      };

      mockAccountService.getCurrentAccount.mockReturnValue(mockAccount);
      mockNetworkService.getCurrentNetwork.mockReturnValue(mockNetwork);

      await walletClient.connect();
    });

    it('should get balance', async () => {
      const balance = await walletClient.getBalance('0x123');
      expect(balance).toBe('1000000000000000000');
    });

    it('should get transaction count', async () => {
      const count = await walletClient.getTransactionCount('0x123');
      expect(count).toBe(5);
    });

    it('should throw error for sign message (MPC required)', async () => {
      await expect(walletClient.signMessage('Hello')).rejects.toThrow('Message signing must use MPC protocol');
    });

    it('should throw error for send transaction (MPC required)', async () => {
      await expect(walletClient.sendTransaction({
        to: '0x456',
        value: BigInt('1000000000000000000')
      })).rejects.toThrow('Transaction signing must use MPC protocol');
    });
  });

  describe('Error Handling', () => {
    it('should throw error when no account for balance check', async () => {
      mockAccountService.getCurrentAccount.mockReturnValue(null);
      await expect(walletClient.getBalance()).rejects.toThrow('No account selected');
    });
  });
});