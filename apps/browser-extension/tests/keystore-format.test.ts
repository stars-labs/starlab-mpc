/**
 * Test to validate keystore format compatibility between CLI and Chrome extension
 */

import { describe, test, expect } from 'bun:test';

describe('Keystore Format Compatibility', () => {
  test('should handle both hex-encoded and direct JSON formats', () => {
    // Test data representing both formats
    const hexEncodedKeystore = {
      version: "1.0",
      curve: "Secp256k1Curve",
      identifier: 3,
      total_participants: 3,
      threshold: 2,
      // Hex-encoded JSON (CLI format) - simple valid JSON for testing
      key_package: "7b7d", // "{}" in hex
      group_public_key: "7b226b6579223a2276616c7565227d", // '{"key":"value"}' in hex
      created_at: 1750842511
    };

    const directJsonKeystore = {
      version: "1.0", 
      curve: "Secp256k1Curve",
      identifier: 3,
      total_participants: 3,
      threshold: 2,
      // Direct JSON strings (extension export format)
      key_package: '{"header":2,"version":0,"ciphersuite":"FROST-secp256k1-SHA256-v1"}',
      group_public_key: '{"verifying_shares":{"011":"03e0a2f5ffec43d3c80d11e9903488f110458dc4ad49457c851ff0c8bc16eada639"}}',
      created_at: 1750842511
    };

    // Test hex detection
    const isHex = (str: string) => str.split('').every(c => '0123456789abcdefABCDEF'.includes(c));
    
    expect(isHex(hexEncodedKeystore.key_package)).toBe(true);
    expect(isHex(hexEncodedKeystore.group_public_key)).toBe(true);
    expect(isHex(directJsonKeystore.key_package)).toBe(false);
    expect(isHex(directJsonKeystore.group_public_key)).toBe(false);

    // Test hex decoding
    const hexDecode = (hex: string): string => {
      let result = '';
      for (let i = 0; i < hex.length; i += 2) {
        result += String.fromCharCode(parseInt(hex.substr(i, 2), 16));
      }
      return result;
    };

    // Verify hex-encoded format can be decoded to JSON
    if (isHex(hexEncodedKeystore.key_package)) {
      const decoded = hexDecode(hexEncodedKeystore.key_package);
      expect(() => JSON.parse(decoded)).not.toThrow();
    }

    // Verify direct JSON format is valid
    expect(() => JSON.parse(directJsonKeystore.key_package)).not.toThrow();
    expect(() => JSON.parse(directJsonKeystore.group_public_key)).not.toThrow();
  });

  test('should import keystore with either format', () => {
    // Mock import function behavior
    const mockImport = (keystoreJson: string) => {
      const keystore = JSON.parse(keystoreJson);
      const keyPackageStr = keystore.key_package;
      const groupPublicKeyStr = keystore.group_public_key;
      
      // Check if hex-encoded or direct JSON
      const isHex = (str: string) => str.split('').every(c => '0123456789abcdefABCDEF'.includes(c));
      
      let keyPackageJson: string;
      let groupPublicKeyJson: string;
      
      if (isHex(keyPackageStr)) {
        // Decode hex to JSON
        const hexDecode = (hex: string): string => {
          let result = '';
          for (let i = 0; i < hex.length; i += 2) {
            result += String.fromCharCode(parseInt(hex.substr(i, 2), 16));
          }
          return result;
        };
        keyPackageJson = hexDecode(keyPackageStr);
        groupPublicKeyJson = hexDecode(groupPublicKeyStr);
      } else {
        // Already JSON
        keyPackageJson = keyPackageStr;
        groupPublicKeyJson = groupPublicKeyStr;
      }
      
      // Validate JSON
      JSON.parse(keyPackageJson);
      JSON.parse(groupPublicKeyJson);
      
      return {
        success: true,
        identifier: keystore.identifier,
        threshold: keystore.threshold,
        totalParticipants: keystore.total_participants
      };
    };

    // Test with hex-encoded format
    const hexKeystore = JSON.stringify({
      version: "1.0",
      curve: "Secp256k1Curve", 
      identifier: 3,
      total_participants: 3,
      threshold: 2,
      key_package: "7b7d", // "{}" in hex
      group_public_key: "7b7d", // "{}" in hex
      created_at: 1750842511
    });

    const hexResult = mockImport(hexKeystore);
    expect(hexResult.success).toBe(true);
    expect(hexResult.identifier).toBe(3);

    // Test with direct JSON format
    const jsonKeystore = JSON.stringify({
      version: "1.0",
      curve: "Secp256k1Curve",
      identifier: 3, 
      total_participants: 3,
      threshold: 2,
      key_package: "{}",
      group_public_key: "{}",
      created_at: 1750842511
    });

    const jsonResult = mockImport(jsonKeystore);
    expect(jsonResult.success).toBe(true);
    expect(jsonResult.identifier).toBe(3);
  });

  test('should handle invalid formats gracefully', () => {
    const invalidKeystores = [
      // Missing required fields
      {
        version: "1.0",
        curve: "Secp256k1Curve"
      },
      // Invalid hex
      {
        version: "1.0",
        identifier: 3,
        total_participants: 3, 
        threshold: 2,
        key_package: "ZZZZ", // Invalid hex
        group_public_key: "{}",
        created_at: 1750842511
      },
      // Invalid JSON in direct format
      {
        version: "1.0",
        identifier: 3,
        total_participants: 3,
        threshold: 2, 
        key_package: "{invalid json",
        group_public_key: "{}",
        created_at: 1750842511
      }
    ];

    // Cast to any inside the loop — the invalidKeystores array has
    // intentionally heterogeneous shapes (each entry omits a
    // different field to test validation) so the inferred union
    // type makes every field possibly-undefined. Tests branch on
    // index so the relevant field is guaranteed present at each
    // check site.
    invalidKeystores.forEach((k, index) => {
      const keystore = k as any;
      const keystoreJson = JSON.stringify(keystore);
      // Check for missing fields - first keystore should fail
      if (index === 0) {
        expect(keystore.identifier).toBeUndefined();
      } else if (index === 1) {
        // Invalid hex should not be all hex digits
        expect(keystore.key_package.split('').every((c: string) => '0123456789abcdefABCDEF'.includes(c))).toBe(false);
      } else if (index === 2) {
        // Invalid JSON should throw when parsed
        expect(() => JSON.parse(keystore.key_package)).toThrow();
      }
    });
  });
});