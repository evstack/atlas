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
        args: inputs.map((inp, i) => {
          const chunk = calldata.slice(i * 64, (i + 1) * 64).padEnd(64, '0');
          return {
            name: inp.name || `arg${i}`,
            type: inp.type,
            value: `0x${chunk}`,
          };
        }),
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
  const intMatch = type.match(/^(u)?int(\d*)$/);
  if (intMatch) {
    const isUnsigned = intMatch[1] === 'u';
    const bits = intMatch[2] ? Number.parseInt(intMatch[2], 10) : 256;
    let value = BigInt('0x' + wordHex);

    if (!isUnsigned) {
      const mask = (1n << BigInt(bits)) - 1n;
      value &= mask;
      const signBit = 1n << BigInt(bits - 1);
      if ((value & signBit) !== 0n) {
        value -= 1n << BigInt(bits);
      }
    }

    return { value: value.toString(10), size: 32 };
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

// Keccak-256 implementation (Ethereum's variant of SHA-3).
function keccak256(data: Uint8Array): Uint8Array {
  const state = Array<bigint>(25).fill(0n);
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
      state[j / 8] = mask64(state[j / 8] ^ lane);
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
  return mask64(val);
}

function writeLane(buf: Uint8Array, offset: number, val: bigint) {
  const u = mask64(val);
  for (let i = 0; i < 8; i++) {
    buf[offset + i] = Number((u >> BigInt(8 * i)) & 0xffn);
  }
}

const ROTATION_OFFSETS = [
  [0, 36, 3, 41, 18],
  [1, 44, 10, 45, 2],
  [62, 6, 43, 15, 61],
  [28, 55, 25, 21, 56],
  [27, 20, 39, 8, 14],
] as const;

const ROUND_CONSTANTS: bigint[] = [
  0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an, 0x8000000080008000n,
  0x000000000000808bn, 0x0000000080000001n, 0x8000000080008081n, 0x8000000000008009n,
  0x000000000000008an, 0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
  0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n, 0x8000000000008003n,
  0x8000000000008002n, 0x8000000000000080n, 0x000000000000800an, 0x800000008000000an,
  0x8000000080008081n, 0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
];

function mask64(x: bigint): bigint {
  return BigInt.asUintN(64, x);
}

function rol64(x: bigint, n: number): bigint {
  n = ((n % 64) + 64) % 64;
  if (n === 0) return x;
  const u = mask64(x);
  return mask64((u << BigInt(n)) | (u >> BigInt(64 - n)));
}

function keccakF(state: bigint[]) {
  for (let round = 0; round < 24; round++) {
    const c = Array<bigint>(5).fill(0n);
    for (let x = 0; x < 5; x++) {
      c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
    }

    const d = Array<bigint>(5).fill(0n);
    for (let x = 0; x < 5; x++) {
      d[x] = c[(x + 4) % 5] ^ rol64(c[(x + 1) % 5], 1);
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        const index = x + 5 * y;
        state[index] = mask64(state[index] ^ d[x]);
      }
    }

    const b = Array<bigint>(25).fill(0n);
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        const index = x + 5 * y;
        const newX = y;
        const newY = (2 * x + 3 * y) % 5;
        b[newX + 5 * newY] = rol64(state[index], ROTATION_OFFSETS[x][y]);
      }
    }

    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        const index = x + 5 * y;
        const current = b[index];
        const next = b[(x + 1) % 5 + 5 * y];
        const nextNext = b[(x + 2) % 5 + 5 * y];
        state[index] = mask64(current ^ (mask64(~next) & nextNext));
      }
    }

    state[0] = mask64(state[0] ^ ROUND_CONSTANTS[round]);
  }
}

// ── Hex utils ─────────────────────────────────────────────────────────────────

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const padded = clean.length % 2 === 0 ? clean : '0' + clean;
  const bytes = new Uint8Array(padded.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    const value = parseInt(padded.slice(i * 2, i * 2 + 2), 16);
    if (Number.isNaN(value)) {
      throw new Error(`Invalid hex byte at index ${i}`);
    }
    bytes[i] = value;
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}
