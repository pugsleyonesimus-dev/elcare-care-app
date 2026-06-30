export const SOROBAN_ERROR_MESSAGES: Record<number, string> = {
  1: "Invalid metadata provided for this artwork.",
  2: "The price is invalid. Please enter a positive value.",
  3: "This listing was not found on-chain.",
  4: "This listing is no longer active.",
  5: "You are not authorized to perform this action.",
  6: "You cannot buy your own listing.",
  7: "Revenue split configuration is invalid.",
  8: "Too many recipients were supplied for this listing.",
  9: "This auction was not found on-chain.",
  10: "This auction is no longer active.",
  11: "Your bid is too low for this auction.",
  12: "This auction has already expired.",
  13: "This auction has not expired yet.",
  14: "This auction is already finalized.",
  15: "This artist account is currently revoked.",
  16: "This offer was not found on-chain.",
  17: "You cannot make an offer on your own listing.",
  18: "This offer is no longer pending.",
  19: "Offer amount is too low.",
  20: "This listing has already been sold.",
  21: "This listing has been cancelled.",
  22: "The contract rejected this request for safety reasons.",
  23: "Insufficient token balance to complete this transaction.",
  24: "Token allowance is too low. Please approve the required amount.",
};

/**
 * Phrases that indicate the user cancelled signing in their wallet extension.
 * Checked case-insensitively against the raw error message string.
 */
const USER_REJECTION_PHRASES: string[] = [
  "user rejected",
  "user denied",
  "user cancelled",
  "user canceled",
  "rejected by user",
  "transaction was rejected",
  "sign request was rejected",
  "request rejected",
];

/**
 * Returns true when the error was caused by the user explicitly declining
 * the signing request in their wallet (Freighter, LOBSTR, etc.).
 */
export function isUserRejectionError(error: unknown): boolean {
  const msg =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  const lower = msg.toLowerCase();
  return USER_REJECTION_PHRASES.some((phrase) => lower.includes(phrase));
}

const CONTRACT_CODE_PATTERNS: RegExp[] = [
  /Error\(Contract,\s*#(\d+)\)/i,
  /Contract(?:Error)?[^\d#]*(?:#|code[:=\s])\s*(\d+)/i,
  /"contractCode"\s*:\s*(\d+)/i,
];

export function extractSorobanContractCode(raw: string): number | null {
  for (const pattern of CONTRACT_CODE_PATTERNS) {
    const match = raw.match(pattern);
    if (match?.[1]) {
      const parsed = Number.parseInt(match[1], 10);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
}

export function mapSorobanErrorMessage(raw: string): string | null {
  const code = extractSorobanContractCode(raw);
  if (code === null) return null;
  const mapped = SOROBAN_ERROR_MESSAGES[code];
  return mapped ? `${mapped} (code ${code})` : null;
}

export function getReadableErrorMessage(
  error: unknown,
  fallback = "Something went wrong. Please try again."
): string {
  if (error instanceof Error) {
    const mapped = mapSorobanErrorMessage(error.message);
    return mapped ?? error.message ?? fallback;
  }
  if (typeof error === "string") {
    const mapped = mapSorobanErrorMessage(error);
    return mapped ?? error;
  }
  return fallback;
}
