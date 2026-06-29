/**
 * Unit tests for lib/ipfs.ts — IPFS gateway fallback utilities.
 */
import axios from 'axios';

jest.mock('@/lib/config', () => ({
  config: {
    pinataGateway: 'https://gateway.pinata.cloud',
  },
}));

jest.mock('axios');
const mockAxios = jest.mocked(axios);

import {
  normalizeIpfsUri,
  getGatewayUrls,
  cidToGatewayUrl,
  fetchMetadata,
  DEFAULT_FALLBACK_GATEWAYS,
} from '@/lib/ipfs';

describe('normalizeIpfsUri', () => {
  it('strips ipfs:// prefix from a CID', () => {
    expect(normalizeIpfsUri('ipfs://QmTest123')).toBe('QmTest123');
  });

  it('passes a raw CID through unchanged', () => {
    expect(normalizeIpfsUri('QmTest123')).toBe('QmTest123');
  });

  it('passes an HTTP URL through unchanged', () => {
    expect(normalizeIpfsUri('https://example.com/img.png')).toBe('https://example.com/img.png');
  });

  it('trims whitespace', () => {
    expect(normalizeIpfsUri('  ipfs://QmTest  ')).toBe('QmTest');
  });
});

describe('getGatewayUrls', () => {
  it('returns primary gateway first, then fallbacks', () => {
    const urls = getGatewayUrls('QmTest', 'https://my-gateway.example.com');
    expect(urls[0]).toBe('https://my-gateway.example.com/ipfs/QmTest');
    expect(urls[1]).toBe('https://ipfs.io/ipfs/QmTest');
    expect(urls.slice(2)).toEqual(
      DEFAULT_FALLBACK_GATEWAYS.slice(1).map((g) => `${g}/ipfs/QmTest`)
    );
  });

  it('deduplicates when primary matches a fallback', () => {
    const urls = getGatewayUrls('QmTest', 'https://ipfs.io');
    const ipfsIoCount = urls.filter((u) => u.startsWith('https://ipfs.io')).length;
    expect(ipfsIoCount).toBe(1);
  });

  it('strips ipfs:// prefix from input', () => {
    const urls = getGatewayUrls('ipfs://QmTest');
    expect(urls[0]).toContain('/ipfs/QmTest');
  });

  it('returns a single-element array for HTTP URLs', () => {
    const urls = getGatewayUrls('https://cdn.example.com/img.png');
    expect(urls).toEqual(['https://cdn.example.com/img.png']);
  });

  it('uses config.pinataGateway when no primary is provided', () => {
    const urls = getGatewayUrls('QmTest');
    expect(urls[0]).toBe('https://gateway.pinata.cloud/ipfs/QmTest');
  });

  it('strips trailing slashes from gateway URLs', () => {
    const urls = getGatewayUrls('QmTest', 'https://gateway.example.com/');
    expect(urls[0]).toBe('https://gateway.example.com/ipfs/QmTest');
  });
});

describe('cidToGatewayUrl', () => {
  it('returns the first gateway URL (primary)', () => {
    const url = cidToGatewayUrl('QmTest');
    expect(url).toBe('https://gateway.pinata.cloud/ipfs/QmTest');
  });

  it('handles HTTP URLs', () => {
    const url = cidToGatewayUrl('https://cdn.example.com/img.png');
    expect(url).toBe('https://cdn.example.com/img.png');
  });
});

describe('fetchMetadata', () => {
  const mockMetadata = { title: 'Test', description: 'Desc', image: 'QmImg', year: '2024', category: 'art' };

  beforeEach(() => {
    mockAxios.get.mockReset();
  });

  it('fetches from the primary gateway on first attempt', async () => {
    mockAxios.get.mockResolvedValueOnce({ data: mockMetadata });

    const result = await fetchMetadata('QmMetaCid');
    expect(result).toEqual(mockMetadata);
    expect(mockAxios.get).toHaveBeenCalledTimes(1);
    expect(mockAxios.get).toHaveBeenCalledWith(
      'https://gateway.pinata.cloud/ipfs/QmMetaCid'
    );
  });

  it('falls back to the next gateway when the primary fails', async () => {
    mockAxios.get
      .mockRejectedValueOnce(new Error('Primary down'))
      .mockResolvedValueOnce({ data: mockMetadata });

    const result = await fetchMetadata('QmMetaCid');
    expect(result).toEqual(mockMetadata);
    expect(mockAxios.get).toHaveBeenCalledTimes(2);
    expect(mockAxios.get).toHaveBeenNthCalledWith(
      2,
      'https://ipfs.io/ipfs/QmMetaCid'
    );
  });

  it('throws when all gateways fail', async () => {
    mockAxios.get.mockRejectedValue(new Error('All gateways down'));

    await expect(fetchMetadata('QmMetaCid')).rejects.toThrow('All gateways down');
    // Should have tried every gateway
    expect(mockAxios.get).toHaveBeenCalledTimes(
      1 + DEFAULT_FALLBACK_GATEWAYS.length
    );
  });

  it('returns a default object for undefined CIDs', async () => {
    const result = await fetchMetadata(undefined);
    expect(result).toEqual({
      title: 'Unknown Artwork',
      description: '',
      artist: 'Unknown',
      image: '',
      year: '',
      category: '',
    });
    expect(mockAxios.get).not.toHaveBeenCalled();
  });
});
