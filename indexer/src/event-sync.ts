import { rpc } from '@stellar/stellar-sdk';
import { parseMarketplaceEvent, type DecodedEvent } from './parser.js';
import { decodeErrorsCounter } from './metrics.js';
import { withRetry } from './retry.js';

export const MAX_LEDGER_WINDOW = 17_000;
export const EVENT_PAGE_LIMIT = 100;

type RpcEvent = {
  topic: unknown[];
  value: unknown;
  ledger: number;
  contractId?: string;
  txHash?: string;
  id?: string; // Stellar event ID encodes position info
};

function toBase64(value: unknown): string {
  if (typeof value === 'string') return value;
  if (
    value &&
    typeof value === 'object' &&
    'toXDR' in value &&
    typeof (value as { toXDR: (format: string) => string }).toXDR === 'function'
  ) {
    return (value as { toXDR: (format: string) => string }).toXDR('base64');
  }
  return String(value);
}

/**
 * Extracts a stable event index from the Stellar event ID.
 * Stellar event IDs are formatted as "<ledger>-<txIndex>-<eventIndex>" or similar.
 * We use the last numeric segment as the index within the ledger.
 */
function extractEventIndex(event: RpcEvent, fallback: number): number {
  if (typeof event.id === 'string') {
    const parts = event.id.split('-');
    const last = parseInt(parts[parts.length - 1], 10);
    if (!isNaN(last)) return last;
  }
  return fallback;
}

function decodeRpcEvent(event: RpcEvent, eventIndex: number): DecodedEvent | null {
  const topicStrings = event.topic.map((topic) => toBase64(topic));
  const contractId = event.contractId ?? '';
  const txHash = event.txHash ?? '';
  return parseMarketplaceEvent(
    topicStrings,
    toBase64(event.value),
    event.ledger,
    contractId,
    txHash,
    extractEventIndex(event, eventIndex)
  );
}

export async function collectMarketplaceEvents(
  server: rpc.Server,
  contractIds: string[],
  startLedger: number,
  endLedger: number
): Promise<DecodedEvent[]> {
  if (contractIds.length === 0 || startLedger > endLedger) {
    return [];
  }

  const decodedEvents: DecodedEvent[] = [];

  for (let windowStart = startLedger; windowStart <= endLedger; windowStart += MAX_LEDGER_WINDOW) {
    const windowEnd = Math.min(windowStart + MAX_LEDGER_WINDOW - 1, endLedger);
    let paginationToken: string | null = null;

    do {
      const response: any = await withRetry(
        () => server.getEvents({
          startLedger: windowStart,
          endLedger: windowEnd,
          filters: [{ type: 'contract', contractIds }],
          limit: EVENT_PAGE_LIMIT,
          ...(paginationToken ? { cursor: paginationToken } : {}),
        } as any),
        { operation: 'getEvents', maxAttempts: 5, baseDelayMs: 500 }
      );

      for (const [idx, event] of (response.events ?? []).entries()) {
        try {
          const decoded = decodeRpcEvent(event, idx);
          if (decoded) decodedEvents.push(decoded);
        } catch (err) {
          decodeErrorsCounter.inc();
          console.error({
            msg: '[EventSync] Failed to decode event — skipping',
            ledger: (event as RpcEvent).ledger,
            eventIndex: idx,
            error: err instanceof Error ? err.message : String(err),
          });
        }
      }

      paginationToken = response.paginationToken ?? null;
    } while (paginationToken);
  }

  return decodedEvents;
}