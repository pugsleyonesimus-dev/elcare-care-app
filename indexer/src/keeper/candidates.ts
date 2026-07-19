/**
 * keeper/candidates.ts
 *
 * Discovers on-chain objects that are ready for keeper maintenance:
 *
 *   ExpireListing   — Active listings whose expires_at has passed.
 *                     The DB does not store expires_at, so we query all Active
 *                     listings and call get_listing via simulateTransaction to
 *                     check each one.  A dedicated view call avoids any changes
 *                     to the ingestion pipeline.
 *
 *   FinalizeAuction — Active auctions where endTime (BigInt Unix seconds) <= now.
 *                     endTime IS stored in the DB, so no RPC view call needed.
 *
 *   ReclaimOffer    — Pending offers whose expires_at has passed.
 *                     The DB does not store expires_at, so we call get_offer via
 *                     simulateTransaction for each Pending offer.
 *
 * Discovery never writes to the DB or to the chain — it is purely read-only.
 */

import {
  rpc,
  Contract,
  TransactionBuilder,
  BASE_FEE,
  Account,
  nativeToScVal,
  xdr,
} from '@stellar/stellar-sdk';
import prisma from '../db.js';
import { logger } from '../logger.js';
import { keeperCandidatesDiscovered, keeperSimulationFailuresTotal } from '../metrics.js';
import type { KeeperCandidate } from './types.js';

// How many Active listings / offers to check per discovery sweep.
// Keeps each cycle bounded regardless of dataset size.
const MAX_LISTINGS_TO_CHECK = parseInt(process.env.KEEPER_DISCOVERY_LIMIT || '200');
const MAX_OFFERS_TO_CHECK   = parseInt(process.env.KEEPER_DISCOVERY_LIMIT || '200');

// ── Low-level: single view call via simulateTransaction ──────────────────────

/**
 * Call a read-only contract function via simulateTransaction and return the
 * decoded JS value.  Returns null if the simulate call fails (e.g. ledger
 * entry evicted) — the candidate is silently skipped.
 */
async function viewCall(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
  method: string,
  args: xdr.ScVal[],
): Promise<xdr.ScVal | null> {
  // We need any valid source account to build the transaction; use a well-known
  // testnet/mainnet account that always exists.  The sequence number is 0
  // because simulateTransaction does not enforce sequence ordering.
  const sourceAccount = new Account('GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN', '0');
  const contract = new Contract(contractId);

  const tx = new TransactionBuilder(sourceAccount, {
    fee: BASE_FEE,
    networkPassphrase,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  try {
    const sim = await server.simulateTransaction(tx);
    if (rpc.Api.isSimulationError(sim)) {
      return null;
    }
    // The first result entry holds the return value.
    const result = (sim as rpc.Api.SimulateTransactionSuccessResponse).result;
    if (!result) return null;
    return result.retval;
  } catch {
    return null;
  }
}

// ── Expired listing discovery ─────────────────────────────────────────────────

/**
 * Returns the listing's expires_at Unix timestamp from an on-chain view call,
 * or null if the listing has no expiry or the call failed.
 */
async function getListingExpiresAt(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
  listingId: bigint,
): Promise<bigint | null> {
  const retval = await viewCall(
    server, contractId, networkPassphrase,
    'get_listing',
    [nativeToScVal(listingId, { type: 'u64' })],
  );
  if (!retval) return null;

  try {
    // The Listing struct is returned as an ScMap.  We decode it as a native JS
    // object and extract expires_at.
    const { scValToNative } = await import('@stellar/stellar-sdk');
    const native = scValToNative(retval) as Record<string, unknown>;
    const expiresAt = native['expires_at'];
    // Soroban Option<u64> is either null/undefined (None) or a bigint (Some).
    if (expiresAt === null || expiresAt === undefined) return null;
    return BigInt(expiresAt as string | number | bigint);
  } catch {
    return null;
  }
}

export async function discoverExpiredListings(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
): Promise<KeeperCandidate[]> {
  const nowSec = BigInt(Math.floor(Date.now() / 1000));

  // Fetch a bounded batch of Active listings from the DB.
  const listings = await prisma.listing.findMany({
    where: { status: 'Active' },
    select: { listingId: true },
    orderBy: { updatedAtLedger: 'asc' },
    take: MAX_LISTINGS_TO_CHECK,
  });

  const candidates: KeeperCandidate[] = [];

  for (const { listingId } of listings) {
    try {
      const expiresAt = await getListingExpiresAt(server, contractId, networkPassphrase, listingId);
      if (expiresAt !== null && nowSec >= expiresAt) {
        candidates.push({ targetType: 'ExpireListing', targetId: listingId });
      }
    } catch (err) {
      keeperSimulationFailuresTotal.inc({ entry_point: 'expire_listing' });
      logger.warn('keeper: failed to check listing expiry', {
        listingId: listingId.toString(),
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }

  keeperCandidatesDiscovered.set({ target_type: 'ExpireListing' }, candidates.length);
  return candidates;
}

// ── Ended auction discovery ───────────────────────────────────────────────────

export async function discoverEndedAuctions(): Promise<KeeperCandidate[]> {
  const nowSec = BigInt(Math.floor(Date.now() / 1000));

  const auctions = await prisma.auction.findMany({
    where: {
      status: 'Active',
      endTime: { lte: nowSec },
    },
    select: { auctionId: true },
  });

  const candidates = auctions.map(({ auctionId }) => ({
    targetType: 'FinalizeAuction' as const,
    targetId: auctionId,
  }));

  keeperCandidatesDiscovered.set({ target_type: 'FinalizeAuction' }, candidates.length);
  return candidates;
}

// ── Expired offer discovery ───────────────────────────────────────────────────

/**
 * Returns the offer's expires_at from an on-chain view call, or null if the
 * offer has no expiry or the call failed.
 */
async function getOfferExpiresAt(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
  offerId: bigint,
): Promise<bigint | null> {
  const retval = await viewCall(
    server, contractId, networkPassphrase,
    'get_offer',
    [nativeToScVal(offerId, { type: 'u64' })],
  );
  if (!retval) return null;

  try {
    const { scValToNative } = await import('@stellar/stellar-sdk');
    const native = scValToNative(retval) as Record<string, unknown>;
    const expiresAt = native['expires_at'];
    if (expiresAt === null || expiresAt === undefined) return null;
    return BigInt(expiresAt as string | number | bigint);
  } catch {
    return null;
  }
}

export async function discoverExpiredOffers(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
): Promise<KeeperCandidate[]> {
  const nowSec = BigInt(Math.floor(Date.now() / 1000));

  const offers = await prisma.offer.findMany({
    where: { status: 'Pending' },
    select: { offerId: true },
    orderBy: { updatedAtLedger: 'asc' },
    take: MAX_OFFERS_TO_CHECK,
  });

  const candidates: KeeperCandidate[] = [];

  for (const { offerId } of offers) {
    try {
      const expiresAt = await getOfferExpiresAt(server, contractId, networkPassphrase, offerId);
      if (expiresAt !== null && nowSec >= expiresAt) {
        candidates.push({ targetType: 'ReclaimOffer', targetId: offerId });
      }
    } catch (err) {
      keeperSimulationFailuresTotal.inc({ entry_point: 'reclaim_offer' });
      logger.warn('keeper: failed to check offer expiry', {
        offerId: offerId.toString(),
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }

  keeperCandidatesDiscovered.set({ target_type: 'ReclaimOffer' }, candidates.length);
  return candidates;
}

// ── Combined discovery ────────────────────────────────────────────────────────

export async function discoverAllCandidates(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
): Promise<KeeperCandidate[]> {
  const [listings, auctions, offers] = await Promise.all([
    discoverExpiredListings(server, contractId, networkPassphrase),
    discoverEndedAuctions(),
    discoverExpiredOffers(server, contractId, networkPassphrase),
  ]);

  return [...listings, ...auctions, ...offers];
}
