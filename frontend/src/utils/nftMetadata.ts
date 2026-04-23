import type { NftAttribute, NftMetadata, NftToken } from '../types';

function getMetadataValue(
  metadata: NftMetadata | null | undefined,
  key: string,
): unknown {
  return metadata?.[key];
}

function getMetadataString(
  metadata: NftMetadata | null | undefined,
  key: string,
): string | null {
  const value = getMetadataValue(metadata, key);
  return typeof value === 'string' ? value : null;
}

export function getNftImageUrl(token: NftToken | null | undefined): string | null {
  if (!token) return null;

  return token.image_url
    || getMetadataString(token.metadata, 'image')
    || getMetadataString(token.metadata, 'image_url')
    || getMetadataString(token.metadata, 'imageUrl')
    || getDataImageUrl(getMetadataValue(token.metadata, 'image_data'));
}

export function getNftDescription(token: NftToken | null | undefined): string | null {
  if (!token) return null;
  return getMetadataString(token.metadata, 'description');
}

export function getNftAttributes(token: NftToken | null | undefined): NftAttribute[] {
  const attributes = token?.metadata?.attributes;
  if (!Array.isArray(attributes)) return [];

  return attributes.filter(isNftAttribute);
}

export function isNftMetadataPending(token: NftToken | null | undefined): boolean {
  return token?.metadata_status === 'pending' || token?.metadata_status === 'retryable_error';
}

export function isNftMetadataUnavailable(token: NftToken | null | undefined): boolean {
  return token?.metadata_status === 'permanent_error';
}

function getDataImageUrl(value: unknown): string | null {
  return typeof value === 'string' && value.startsWith('data:image/') ? value : null;
}

function isNftAttribute(value: unknown): value is NftAttribute {
  if (!value || typeof value !== 'object') return false;

  const attribute = value as Record<string, unknown>;
  return typeof attribute.trait_type === 'string'
    && (typeof attribute.value === 'string' || typeof attribute.value === 'number');
}
