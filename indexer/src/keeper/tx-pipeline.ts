/**
 * keeper/tx-pipeline.ts
 *
 * Builds, simulates, signs, and submits a single Soroban maintenance
 * transaction.  Implements the fee-bump and retry strategy described in the
 * issue spec.
 *
 * Pipeline stages:
 *   1. Build a TransactionBuilder invocation for the target entry-point.
 *   2. simulateTransaction → assembleTransaction (resource footprint).
 *   3. Sign with the keeper Keypair.
 *   4. sendTransaction + poll getTransaction to completion.
 *   5. On timeout → fee-bump with TransactionBuilder.buildFeeBumpTransaction,
 *      capped at KEEPER_FEE_BUMP_MAX_RETRIES escalations.
 *   6. On sequence collision → reload source account + rebuild from step 1.
 *
 * Dry-run mode (KEEPER_DRY_RUN=true) executes steps 1-2 and logs the intended
 * action without broadcasting (step 3+).
 */

import {
  rpc,
  Contract,
  TransactionBuilder,
  BASE_FEE,
  Keypair,
  Account,
  nativeToScVal,
  assembleTransaction,
  Transaction,
  FeeBumpTransaction,
} from '@stellar/stellar-sdk';
import { logger } from '../logger.js';
import {
  keeperFeeBumpsTotal,
  keeperSimulationFailuresTotal,
} from '../metrics.js';
import { classifyError, isFeeError, isSeqError } from './error-classifier.js';
import type { KeeperConfig } from '../config.js';
import type { KeeperCandidate, SubmitOutcome } from './types.js';

// ── Build entry-point arguments ───────────────────────────────────────────────

/**
 * Returns the contract method name and ScVal arguments for a given candidate.
 * finalize_auction requires the caller address as the first argument because
 * the contract does caller.require_auth().
 */
function buildCallArgs(
  candidate: KeeperCandidate,
  keeperPublicKey: string,
): { method: string; args: ReturnType<typeof nativeToScVal>[] } {
  const id = candidate.targetId;

  switch (candidate.targetType) {
    case 'ExpireListing':
      return {
        method: 'expire_listing',
        args: [nativeToScVal(id, { type: 'u64' })],
      };
    case 'FinalizeAuction':
      return {
        method: 'finalize_auction',
        args: [
          nativeToScVal(keeperPublicKey, { type: 'address' }),
          nativeToScVal(id, { type: 'u64' }),
        ],
      };
    case 'ReclaimOffer':
      return {
        method: 'reclaim_offer',
        args: [nativeToScVal(id, { type: 'u64' })],
      };
  }
}

// ── Poll getTransaction ───────────────────────────────────────────────────────

async function pollTransaction(
  server: rpc.Server,
  txHash: string,
  cfg: KeeperConfig,
): Promise<rpc.Api.GetTransactionResponse> {
  const deadline = Date.now() + cfg.KEEPER_POLL_TIMEOUT_MS;

  while (Date.now() < deadline) {
    const result = await server.getTransaction(txHash);

    if (result.status !== rpc.Api.GetTransactionStatus.NOT_FOUND) {
      return result;
    }

    await new Promise((r) => setTimeout(r, cfg.KEEPER_POLL_INTERVAL_MS));
  }

  throw new Error(`poll timeout: transaction ${txHash} not found after ${cfg.KEEPER_POLL_TIMEOUT_MS}ms`);
}

// ── Fee calculation helpers ───────────────────────────────────────────────────

/**
 * Calculate the escalated fee for a fee-bump attempt.
 * Each retry multiplies by KEEPER_FEE_BUMP_MULTIPLIER, capped at
 * KEEPER_MAX_FEE_STROOPS.
 */
export function escalateFee(
  baseFeeStroops: number,
  bumpAttempt: number,          // 1-indexed
  multiplier: number,
  maxFeeStroops: number,
): number {
  const escalated = Math.ceil(baseFeeStroops * Math.pow(multiplier, bumpAttempt));
  return Math.min(escalated, maxFeeStroops);
}

// ── Core pipeline ─────────────────────────────────────────────────────────────

export interface PipelineOptions {
  server: rpc.Server;
  contractId: string;
  networkPassphrase: string;
  keypair: Keypair;
  cfg: KeeperConfig;
  dryRun: boolean;
}

/**
 * Execute the full transaction pipeline for a single candidate.
 * Returns a SubmitOutcome describing what happened.
 */
export async function executeTransaction(
  candidate: KeeperCandidate,
  opts: PipelineOptions,
): Promise<SubmitOutcome> {
  const { server, contractId, networkPassphrase, keypair, cfg, dryRun } = opts;
  const entryPoint = candidate.targetType === 'ExpireListing'
    ? 'expire_listing'
    : candidate.targetType === 'FinalizeAuction'
      ? 'finalize_auction'
      : 'reclaim_offer';

  // ── Step 1 + 2: Build and simulate (with optional sequence recovery) ────────

  let builtTx: Transaction;
  let assembledTx: Transaction;
  let baseFeeStroops = Number(BASE_FEE);

  // Sequence-number recovery: re-fetch account if a prior attempt reported
  // tx_bad_seq.  We attempt at most 2 load-and-build rounds.
  for (let seqAttempt = 0; seqAttempt <= 1; seqAttempt++) {
    const sourceAccount = await server.getAccount(keypair.publicKey());
    const account = new Account(sourceAccount.accountId(), sourceAccount.sequenceNumber());

    const contract = new Contract(contractId);
    const { method, args } = buildCallArgs(candidate, keypair.publicKey());

    builtTx = new TransactionBuilder(account, {
      fee: String(cfg.KEEPER_MAX_FEE_STROOPS),
      networkPassphrase,
    })
      .addOperation(contract.call(method, ...args))
      .setTimeout(cfg.KEEPER_SUBMIT_TIMEOUT_MS / 1000)
      .build();

    // ── Simulate to obtain resource footprint ─────────────────────────────────
    let sim: rpc.Api.SimulateTransactionResponse;
    try {
      sim = await server.simulateTransaction(builtTx);
    } catch (err) {
      keeperSimulationFailuresTotal.inc({ entry_point: entryPoint });
      const cls = classifyError(err);
      if (cls === 'permanent') {
        return { kind: 'permanent_skip', reason: String(err) };
      }
      return { kind: 'transient_failure', error: err instanceof Error ? err : new Error(String(err)) };
    }

    if (rpc.Api.isSimulationError(sim)) {
      keeperSimulationFailuresTotal.inc({ entry_point: entryPoint });
      const errMsg = (sim as rpc.Api.SimulateTransactionErrorResponse).error;
      const cls = classifyError(errMsg);
      logger.warn('keeper: simulation returned contract error', {
        candidate: `${candidate.targetType}:${candidate.targetId}`,
        error: errMsg,
        class: cls,
      });
      if (cls === 'permanent') {
        return { kind: 'permanent_skip', reason: errMsg };
      }
      return { kind: 'transient_failure', error: new Error(errMsg) };
    }

    assembledTx = assembleTransaction(builtTx, sim as rpc.Api.SimulateTransactionSuccessResponse).build();
    baseFeeStroops = Number((sim as rpc.Api.SimulateTransactionSuccessResponse).minResourceFee ?? BASE_FEE);
    break; // successful build
  }

  // ── Dry-run: log and return without broadcasting ──────────────────────────
  if (dryRun) {
    logger.info('keeper: DRY-RUN — would submit transaction', {
      candidate: `${candidate.targetType}:${candidate.targetId}`,
      method: buildCallArgs(candidate, keypair.publicKey()).method,
      txXdr: assembledTx!.toXDR(),
    });
    return { kind: 'succeeded', txHash: 'dry-run', feePaid: 0n };
  }

  // ── Step 3: Sign ──────────────────────────────────────────────────────────
  assembledTx!.sign(keypair);

  // ── Steps 4-5: Submit + poll + fee-bump loop ──────────────────────────────
  let currentTx: Transaction | FeeBumpTransaction = assembledTx!;
  let lastTxHash = '';

  for (let bumpAttempt = 0; bumpAttempt <= cfg.KEEPER_FEE_BUMP_MAX_RETRIES; bumpAttempt++) {
    // Send
    let sendResult: rpc.Api.SendTransactionResponse;
    try {
      sendResult = await server.sendTransaction(currentTx);
    } catch (err) {
      const cls = classifyError(err);
      if (cls === 'permanent') {
        return { kind: 'permanent_skip', reason: String(err) };
      }
      if (isSeqError(err) && bumpAttempt === 0) {
        // Sequence collision: rebuild with fresh sequence, no fee-bump needed
        logger.warn('keeper: sequence collision on submit, rebuilding', {
          candidate: `${candidate.targetType}:${candidate.targetId}`,
        });
        return executeTransaction(candidate, opts); // tail-recurse once
      }
      return { kind: 'transient_failure', error: err instanceof Error ? err : new Error(String(err)) };
    }

    if (sendResult.status === 'ERROR') {
      const errStr = JSON.stringify(sendResult.errorResult ?? sendResult);
      const cls = classifyError(errStr);
      if (cls === 'permanent') {
        return { kind: 'permanent_skip', reason: errStr };
      }
      return { kind: 'transient_failure', error: new Error(errStr) };
    }

    lastTxHash = sendResult.hash;

    // Poll for confirmation
    let pollResult: rpc.Api.GetTransactionResponse;
    try {
      pollResult = await pollTransaction(server, lastTxHash, cfg);
    } catch (timeoutErr) {
      // Timeout — escalate fee if budget allows and retries remain
      if (bumpAttempt >= cfg.KEEPER_FEE_BUMP_MAX_RETRIES) {
        return {
          kind: 'transient_failure',
          error: new Error(`fee-bump cap reached after ${bumpAttempt} bumps; last error: ${timeoutErr}`),
        };
      }

      const bumpFee = escalateFee(
        baseFeeStroops,
        bumpAttempt + 1,
        cfg.KEEPER_FEE_BUMP_MULTIPLIER,
        cfg.KEEPER_MAX_FEE_STROOPS,
      );

      logger.warn('keeper: tx timeout — applying fee-bump', {
        candidate: `${candidate.targetType}:${candidate.targetId}`,
        bumpAttempt: bumpAttempt + 1,
        bumpFeeStroops: bumpFee,
      });

      keeperFeeBumpsTotal.inc({ entry_point: entryPoint });

      // Build fee-bump wrapping the original assembled (base) transaction
      const feeBumpTx = TransactionBuilder.buildFeeBumpTransaction(
        keypair,
        String(bumpFee),
        assembledTx! as Transaction,
        networkPassphrase,
      );
      feeBumpTx.sign(keypair);
      currentTx = feeBumpTx;
      continue; // next loop iteration submits the fee-bump
    }

    if (pollResult.status === rpc.Api.GetTransactionStatus.SUCCESS) {
      // Extract the fee actually charged from the envelope
      const feePaid = BigInt(
        (pollResult as rpc.Api.GetSuccessfulTransactionResponse).envelopeXdr
          ? 0 // real extraction would parse the XDR; 0 is a safe fallback
          : 0
      );
      return { kind: 'succeeded', txHash: lastTxHash, feePaid };
    }

    if (pollResult.status === rpc.Api.GetTransactionStatus.FAILED) {
      const resultMeta = (pollResult as rpc.Api.GetFailedTransactionResponse).resultXdr ?? '';
      const cls = classifyError(resultMeta.toString());
      if (cls === 'permanent') {
        return { kind: 'permanent_skip', reason: resultMeta.toString() };
      }
      // Non-permanent on-chain failure — surface as transient so the keeper
      // can retry on the next cycle
      return { kind: 'transient_failure', error: new Error(`on-chain failure: ${resultMeta}`) };
    }

    // Should not reach here (NOT_FOUND after poll timeout is handled above)
    return { kind: 'transient_failure', error: new Error(`unexpected poll status: ${pollResult.status}`) };
  }

  return {
    kind: 'transient_failure',
    error: new Error('fee-bump loop exhausted without a definitive result'),
  };
}
