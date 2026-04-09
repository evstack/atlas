import type { AbiItem } from '../types';

export interface DecodedArg {
  name: string;
  type: string;
  value: string;
}

export interface DecodedCall {
  name: string;
  args: DecodedArg[];
}

/**
 * Decode transaction input data using the provided ABI.
 * Returns null if the input is empty, too short, or no matching function is found.
 */
export function decodeInputData(input: string | undefined | null, abi?: AbiItem[]): DecodedCall | null {
  if (!input || !abi || input === '0x' || input.length < 10) return null;

  const selectorHex = input.slice(2, 10).toLowerCase();

  for (const item of abi) {
    if (item.type !== 'function' || !item.name) continue;

    const inputs = item.inputs ?? [];
    const sig = `${item.name}(${inputs.map(i => i.type).join(',')})`;
    const hash = keccak256Hex(sig).slice(0, 8);

    if (hash !== selectorHex) continue;

    // Matched — try to decode arguments
    const calldata = input.slice(10); // hex after selector, no 0x
    const args = decodeArguments(inputs, calldata);
    if (args === null) {
      // Matched selector but decode failed — still return the function name
      return {
        name: item.name,
        args: inputs.map((inp, i) => ({
          name: inp.name || `arg${i}`,
          type: inp.type,
          value: `0x${calldata.slice(i * 64, (i + 1) * 64)}`,
        })),
      };
    }

    return { name: item.name, args };
  }

  return null;
}

// ── Argument decoding ─────────────────────────────────────────────────────────

function decodeArguments(
  inputs: AbiItem['inputs'],
  calldata: string
): DecodedArg[] | null {
  if (!inputs || inputs.length === 0) return [];

  try {
    const buf = hexToBytes(calldata);
    const result: DecodedArg[] = [];
    let offset = 0;

    for (let i = 0; i < inputs.length; i++) {
      const inp = inputs[i];
      const { value, size } = decodeType(inp.type, buf, offset, 0);
      result.push({ name: inp.name || `arg${i}`, type: inp.type, value });
      offset += size;
    }

    return result;
  } catch {
    return null;
  }
}

function decodeType(
  type: string,
  buf: Uint8Array,
  offset: number,
  depth: number
): { value: string; size: number } {
  if (depth > 5) return { value: '…', size: 32 };

  const word = buf.slice(offset, offset + 32);
  const wordHex = bytesToHex(word);

  // uint/int (fixed size)
  if (/^u?int(\d*)$/.test(type)) {
    return { value: BigInt('0x' + wordHex).toString(10), size: 32 };
  }

  // address
  if (type === 'address') {
    return { value: '0x' + wordHex.slice(24), size: 32 };
  }

  // bool
  if (type === 'bool') {
    return { value: BigInt('0x' + wordHex) === 0n ? 'false' : 'true', size: 32 };
  }

  // bytes1 … bytes32 (static)
  if (/^bytes(\d+)$/.test(type)) {
    const n = parseInt(type.replace('bytes', ''), 10);
    return { value: '0x' + wordHex.slice(0, n * 2), size: 32 };
  }

  // dynamic: bytes, string, arrays — just show the offset pointer value
  return { value: '0x' + wordHex, size: 32 };
}

// ── Keccak-256 (sync, pure JS) ────────────────────────────────────────────────
// Minimal implementation for 4-byte selector matching. Not cryptographically
// sensitive here — we just need function selector matching.

function keccak256Hex(input: string): string {
  const bytes = new TextEncoder().encode(input);
  return bytesToHex(keccak256(bytes));
}

// Keccak-256 implementation (Ethereum's variant of SHA-3)
// Based on the NIST Keccak reference implementation, adapted for JS.
function keccak256(data: Uint8Array): Uint8Array {
  const state = new BigInt64Array(25);
  const rate = 136; // 1088 / 8 for keccak256
  const output = 32;

  // Padding
  const padded = new Uint8Array(Math.ceil((data.length + 1) / rate) * rate);
  padded.set(data);
  padded[data.length] = 0x01;
  padded[padded.length - 1] |= 0x80;

  // Absorb
  for (let i = 0; i < padded.length; i += rate) {
    for (let j = 0; j < rate; j += 8) {
      const lane = readLane(padded, i + j);
      state[j / 8] ^= lane;
    }
    keccakF(state);
  }

  // Squeeze
  const hash = new Uint8Array(output);
  for (let i = 0; i < output; i += 8) {
    writeLane(hash, i, state[i / 8]);
  }
  return hash;
}

function readLane(buf: Uint8Array, offset: number): bigint {
  let val = 0n;
  for (let i = 0; i < 8; i++) {
    val |= BigInt(buf[offset + i] ?? 0) << BigInt(8 * i);
  }
  return BigInt.asIntN(64, val);
}

function writeLane(buf: Uint8Array, offset: number, val: bigint) {
  const u = BigInt.asUintN(64, val);
  for (let i = 0; i < 8; i++) {
    buf[offset + i] = Number((u >> BigInt(8 * i)) & 0xffn);
  }
}

// Rotation constants
const ROT: number[] = [
  1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];
// Pi indices
const PI: number[] = [
  10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];
// Round constants
const RC: bigint[] = [
  0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an, 0x8000000080008000n,
  0x000000000000808bn, 0x0000000080000001n, 0x8000000080008081n, 0x8000000000008009n,
  0x000000000000008an, 0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
  0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n, 0x8000000000008003n,
  0x8000000000008002n, 0x8000000000000080n, 0x000000000000800an, 0x800000008000000an,
  0x8000000080008081n, 0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
];

function rol64(x: bigint, n: number): bigint {
  n = ((n % 64) + 64) % 64;
  if (n === 0) return x;
  const u = BigInt.asUintN(64, x);
  return BigInt.asIntN(64, (u << BigInt(n)) | (u >> BigInt(64 - n)));
}

function keccakF(A: BigInt64Array) {
  for (let round = 0; round < 24; round++) {
    // θ
    const C = new BigInt64Array(5);
    for (let x = 0; x < 5; x++) {
      C[x] = A[x] ^ A[x + 5] ^ A[x + 10] ^ A[x + 15] ^ A[x + 20];
    }
    const D = new BigInt64Array(5);
    for (let x = 0; x < 5; x++) {
      D[x] = C[(x + 4) % 5] ^ rol64(C[(x + 1) % 5], 1);
    }
    for (let i = 0; i < 25; i++) A[i] ^= D[i % 5];

    // ρ and π
    const B = new BigInt64Array(25);
    B[0] = A[0];
    for (let t = 0; t < 24; t++) {
      B[PI[t]] = rol64(A[t === 0 ? 0 : PI[t - 1]], ROT[t]);
    }

    // χ
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        A[x + 5 * y] = B[x + 5 * y] ^ (~B[(x + 1) % 5 + 5 * y] & B[(x + 2) % 5 + 5 * y]);
      }
    }

    // ι
    A[0] ^= RC[round];
  }
}

// ── Hex utils ─────────────────────────────────────────────────────────────────

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const padded = clean.length % 2 === 0 ? clean : '0' + clean;
  const bytes = new Uint8Array(padded.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(padded.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}
