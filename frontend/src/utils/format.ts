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

/**
 * Formats a Wei value to Ether with specified decimal places
 */
export function formatEther(wei: string, decimals = 6): string {
  if (!wei) return '0';

  const weiNum = BigInt(wei);
  const etherDivisor = BigInt(10 ** 18);
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

  const wei = BigInt(weiPrice);
  const gweiDivisor = BigInt(10 ** 9);
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

  const amountNum = BigInt(amount);
  if (amountNum === BigInt(0)) return '0';

  const divisor = BigInt(10 ** decimals);
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

/**
 * Formats a percentage value
 * @param percentage The percentage value (0-100)
 * @param decimals Number of decimal places to display
 */
export function formatPercentage(percentage: number, decimals = 2): string {
  return `${percentage.toFixed(decimals)}%`;
}
