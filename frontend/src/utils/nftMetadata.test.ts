import { describe, expect, test } from 'bun:test';

import type { NftToken } from '../types';
import {
  getNftImageUrl,
  isNftMetadataPending,
  isNftMetadataUnavailable,
} from './nftMetadata';

function createToken(overrides: Partial<NftToken> = {}): NftToken {
  return {
    contract_address: '0x0000000000000000000000000000000000000000',
    token_id: '1',
    owner: '0x0000000000000000000000000000000000000001',
    token_uri: 'https://example.com/1.json',
    metadata_status: 'pending',
    metadata_retry_count: 0,
    next_retry_at: null,
    last_metadata_error: null,
    last_metadata_attempted_at: null,
    metadata_updated_at: null,
    metadata: null,
    image_url: null,
    name: null,
    last_transfer_block: 1,
    ...overrides,
  };
}

describe('getNftImageUrl', () => {
  test('falls back to raw metadata image aliases', () => {
    const token = createToken({
      metadata_status: 'fetched',
      metadata: {
        imageUrl: 'https://cdn.example.com/token.png',
      },
    });

    expect(getNftImageUrl(token)).toBe('https://cdn.example.com/token.png');
  });
});

describe('metadata state helpers', () => {
  test('exposes pending and unavailable token states', () => {
    expect(isNftMetadataPending(createToken({ metadata_status: 'retryable_error' }))).toBe(true);
    expect(isNftMetadataUnavailable(createToken({ metadata_status: 'permanent_error' }))).toBe(
      true,
    );
  });
});
