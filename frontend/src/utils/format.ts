/**
 * Truncates a hash or address for display
 * @param hash The full hash/address string
 * @param startChars Number of characters to show at start (default: 6)
 * @param endChars Number of characters to show at end (default: 4)
 */
export function truncateHash(hash: string, startChars = 6, endChars = 4): string {
  if (!hash) return '';
  if (hash.length <= startChars + endChars) return hash;
  return `${hash.slice(0, startChars)}...${hash.slice(-endChars)}`;
}

function expandScientificIntString(input: string): string {
  const s = input.trim();
  if (/^\d+$/.test(s)) return s;
  const m = s.match(/^([\d.]+)[eE]([+-]?\d+)$/);
  if (!m) return s;
  const base = m[1];
  const exp = parseInt(m[2], 10) || 0;
  const dot = base.indexOf('.');
  const digits = base.replace('.', '');
  const decimals = dot >= 0 ? (base.length - dot - 1) : 0;
  if (exp >= decimals) {
    return digits + '0'.repeat(exp - decimals);
  }
  // If result would be fractional, fall back to removing the decimal point (floor toward zero)
  const intLen = digits.length - (decimals - exp);
  const intPart = intLen > 0 ? digits.slice(0, intLen) : '0';
  return intPart.replace(/^0+(?=\d)/, '');
}

function pow10BigInt(n: number): bigint {
  if (n <= 0) return 1n;
  return BigInt('1' + '0'.repeat(n));
}

/**
 * Formats a Wei value to Ether with specified decimal places
 */
export function formatEther(wei: string, decimals = 6): string {
  if (!wei) return '0';

  const normalized = expandScientificIntString(wei);
  const weiNum = BigInt(normalized);
  const etherDivisor = pow10BigInt(18);
  const wholePart = weiNum / etherDivisor;
  const fractionalPart = weiNum % etherDivisor;

  if (fractionalPart === BigInt(0)) {
    return wholePart.toString();
  }

  const fractionalStr = fractionalPart.toString().padStart(18, '0');
  const truncatedFractional = fractionalStr.slice(0, decimals).replace(/0+$/, '');

  if (truncatedFractional === '') {
    return wholePart.toString();
  }

  return `${wholePart}.${truncatedFractional}`;
}

// Exact ETH (no rounding): use all 18 fractional digits, then trim trailing zeros
export function formatEtherExact(wei: string): string {
  if (!wei) return '0';
  const normalized = expandScientificIntString(wei);
  const weiNum = BigInt(normalized);
  const etherDivisor = pow10BigInt(18);
  const wholePart = weiNum / etherDivisor;
  const fractionalPart = weiNum % etherDivisor;
  if (fractionalPart === 0n) return wholePart.toString();
  const fractionalStr = fractionalPart.toString().padStart(18, '0').replace(/0+$/, '');
  if (fractionalStr === '') return wholePart.toString();
  return `${wholePart}.${fractionalStr}`;
}

export function formatUsd(amount: number, opts: { minCents?: number } = {}): string {
  const { minCents = 1 } = opts;
  const absCents = Math.round(Math.abs(amount) * 100);
  const sign = amount < 0 ? '-' : '';
  if (absCents > 0 && absCents < minCents) {
    return `${sign}< $0.01`;
  }
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 2 }).format(amount);
}

/**
 * Formats a number with thousand separators
 */
export function formatNumber(num: number | string): string {
  const n = typeof num === 'string' ? parseFloat(num) : num;
  return new Intl.NumberFormat('en-US').format(n);
}

/**
 * Formats a Unix timestamp to a human-readable date string
 */
export function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}

/**
 * Formats a Unix timestamp to relative time (e.g., "5 mins ago")
 */
export function formatTimeAgo(timestamp: number): string {
  const now = Date.now() / 1000;
  const diff = now - timestamp;

  if (diff < 60) {
    return `${Math.floor(diff)} secs ago`;
  }
  if (diff < 3600) {
    const mins = Math.floor(diff / 60);
    return `${mins} min${mins > 1 ? 's' : ''} ago`;
  }
  if (diff < 86400) {
    const hours = Math.floor(diff / 3600);
    return `${hours} hour${hours > 1 ? 's' : ''} ago`;
  }
  const days = Math.floor(diff / 86400);
  return `${days} day${days > 1 ? 's' : ''} ago`;
}

/**
 * Formats gas value with Gwei suffix
 */
export function formatGas(gas: string): string {
  if (!gas) return '0';
  return formatNumber(gas);
}

/**
 * Formats gas price to Gwei
 */
export function formatGasPrice(weiPrice: string): string {
  if (!weiPrice) return '0';

  const wei = BigInt(expandScientificIntString(weiPrice));
  const gweiDivisor = pow10BigInt(9);
  const gwei = wei / gweiDivisor;
  const remainder = wei % gweiDivisor;

  if (remainder === BigInt(0)) {
    return `${gwei} Gwei`;
  }

  const decimalPart = (Number(remainder) / 10 ** 9).toFixed(2).slice(2).replace(/0+$/, '');
  if (decimalPart === '') {
    return `${gwei} Gwei`;
  }

  return `${gwei}.${decimalPart} Gwei`;
}

/**
 * Formats bytes to human readable size
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${units[i]}`;
}

/**
 * Formats a token amount with the correct number of decimals
 * @param amount The raw token amount as a string
 * @param decimals The number of decimals for the token
 * @param displayDecimals Maximum number of decimals to display (default: 6)
 */
export function formatTokenAmount(amount: string, decimals: number, displayDecimals = 6): string {
  if (!amount) return '0';

  const amountNum = BigInt(expandScientificIntString(amount));
  if (amountNum === BigInt(0)) return '0';

  const divisor = pow10BigInt(decimals);
  const wholePart = amountNum / divisor;
  const fractionalPart = amountNum % divisor;

  if (fractionalPart === BigInt(0)) {
    return formatNumber(wholePart.toString());
  }

  const fractionalStr = fractionalPart.toString().padStart(decimals, '0');
  const truncatedFractional = fractionalStr.slice(0, displayDecimals).replace(/0+$/, '');

  if (truncatedFractional === '') {
    return formatNumber(wholePart.toString());
  }

  return `${formatNumber(wholePart.toString())}.${truncatedFractional}`;
}

// Exact token amount (no rounding): use full fractional precision and trim trailing zeros
export function formatTokenAmountExact(amount: string, decimals: number): string {
  if (!amount) return '0';
  const amountNum = BigInt(expandScientificIntString(amount));
  if (amountNum === 0n) return '0';
  if (decimals <= 0) return formatNumber(amountNum.toString());
  const divisor = pow10BigInt(decimals);
  const wholePart = amountNum / divisor;
  const fractionalPart = amountNum % divisor;
  if (fractionalPart === 0n) return formatNumber(wholePart.toString());
  const fractionalStr = fractionalPart.toString().padStart(decimals, '0').replace(/0+$/, '');
  if (fractionalStr === '') return formatNumber(wholePart.toString());
  return `${formatNumber(wholePart.toString())}.${fractionalStr}`;
}

/**
 * Formats a percentage value
 * @param percentage The percentage value (0-100)
 * @param decimals Number of decimal places to display
 */
export function formatPercentage(percentage: number | null | undefined, decimals = 2): string {
  if (percentage === null || percentage === undefined || Number.isNaN(percentage)) return '0%';
  return `${percentage.toFixed(decimals)}%`;
}
