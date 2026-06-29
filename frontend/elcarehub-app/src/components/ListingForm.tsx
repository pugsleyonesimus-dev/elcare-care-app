// ─────────────────────────────────────────────────────────────
// components/ListingForm.tsx — create and edit listing form
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useEffect, useMemo } from "react";
import { useCreateListing, useUpdateListing } from "@/hooks/useMarketplace";
import { useWalletContext } from "@/context/WalletContext";
import { Upload, CheckCircle, Loader2, Save, Plus, Trash2 } from "lucide-react";
import { GuardButton } from "./WalletGuard";
import { ArtworkMetadata, fetchMetadata } from "@/lib/ipfs";
import { Listing, stroopsToXlm } from "@/lib/contract";
import { DEFAULT_TOKEN } from "@/config/tokens";
import { useSupportedTokens } from "@/hooks/useSupportedTokens";
import { ensureTokenOption, getDefaultSupportedToken } from "@/lib/token-support";
import posthog from "posthog-js";
import { isValidStellarAddress } from "@/lib/validation";

export const ART_CATEGORIES = [
  "Painting",
  "Sculpture",
  "Photography",
  "Digital Art",
  "Textile",
  "Jewelry",
  "Other",
];

// ── Contract constraint constants (mirrors soroban-marketplace/src/types.rs) ──
/** Minimum price in XLM (1 stroop = 0.0000001 XLM). */
export const MIN_PRICE_XLM = 0.0000001;
/** Maximum price in XLM (i128 max / 10^7, practical upper-bound). */
export const MAX_PRICE_XLM = 9_223_372_036_854.7758;
/** Maximum number of royalty recipients (TooManyRecipients = 8). */
export const MAX_RECIPIENTS = 4;
/** Royalty percentages across all recipients must sum to exactly 100. */
export const REQUIRED_SPLIT_SUM = 100;

export interface RecipientInput {
  address: string;
  percentage: number;
}

interface FormState {
  collectionAddress: string;
  nftTokenId: number;
  price: number;
  tokenAddress: string;
  recipients: RecipientInput[];
}

interface FieldErrors {
  collectionAddress?: string;
  nftTokenId?: string;
  price?: string;
  tokenAddress?: string;
  recipients?: string;
  recipientRows?: Array<{ address?: string; percentage?: string }>;
}

interface ListingFormProps {
  listing?: Listing; // If provided, we are in EDIT mode
  onSuccess?: (listingId: number) => void;
  onCancel?: () => void;
}

// ── Validation ────────────────────────────────────────────────

/**
 * Validates all form fields against the contract constraints.
 * Returns an error map; an empty object means the form is valid.
 */
export function validateListingForm(form: FormState): FieldErrors {
  const errors: FieldErrors = {};

  // Collection address — must be a non-empty, valid Stellar address
  if (!form.collectionAddress.trim()) {
    errors.collectionAddress = "Collection address is required.";
  } else if (!isValidStellarAddress(form.collectionAddress.trim())) {
    errors.collectionAddress = "Must be a valid Stellar contract address (starts with C).";
  }

  // NFT Token ID — must be a non-negative integer
  if (!Number.isInteger(form.nftTokenId) || form.nftTokenId < 0) {
    errors.nftTokenId = "Token ID must be a non-negative integer.";
  }

  // Price — must be within contract bounds (price > 0, price ≤ MAX)
  if (!Number.isFinite(form.price) || form.price <= 0) {
    errors.price = `Price must be greater than 0.`;
  } else if (form.price < MIN_PRICE_XLM) {
    errors.price = `Price must be at least ${MIN_PRICE_XLM} (1 stroop).`;
  } else if (form.price > MAX_PRICE_XLM) {
    errors.price = `Price exceeds the maximum allowed value.`;
  }

  // Token address — must be selected
  if (!form.tokenAddress) {
    errors.tokenAddress = "A payment token must be selected.";
  }

  // Recipients — must have 1–4 rows, each a valid address, and sum to exactly 100%
  if (form.recipients.length === 0) {
    errors.recipients = "At least one recipient is required.";
  } else if (form.recipients.length > MAX_RECIPIENTS) {
    errors.recipients = `A maximum of ${MAX_RECIPIENTS} recipients is allowed.`;
  } else {
    const rowErrors: Array<{ address?: string; percentage?: string }> =
      form.recipients.map((r) => {
        const rowErr: { address?: string; percentage?: string } = {};
        if (!r.address.trim()) {
          rowErr.address = "Address is required.";
        } else if (!isValidStellarAddress(r.address.trim())) {
          rowErr.address = "Must be a valid Stellar address.";
        }
        if (!Number.isFinite(r.percentage) || r.percentage <= 0) {
          rowErr.percentage = "Must be greater than 0.";
        } else if (r.percentage > REQUIRED_SPLIT_SUM) {
          rowErr.percentage = `Cannot exceed ${REQUIRED_SPLIT_SUM}%.`;
        }
        return rowErr;
      });

    const hasRowErrors = rowErrors.some(
      (e) => e.address !== undefined || e.percentage !== undefined
    );
    if (hasRowErrors) {
      errors.recipientRows = rowErrors;
    }

    const total = form.recipients.reduce((sum, r) => sum + (r.percentage || 0), 0);
    if (Math.round(total) !== REQUIRED_SPLIT_SUM) {
      errors.recipients = `Recipient percentages must sum to exactly ${REQUIRED_SPLIT_SUM}% (currently ${total.toFixed(2)}%).`;
    }
  }

  return errors;
}

export function isFormValid(errors: FieldErrors): boolean {
  const hasTopLevelError =
    errors.collectionAddress !== undefined ||
    errors.nftTokenId !== undefined ||
    errors.price !== undefined ||
    errors.tokenAddress !== undefined ||
    errors.recipients !== undefined;

  const hasRowError =
    errors.recipientRows !== undefined &&
    errors.recipientRows.some(
      (r) => r.address !== undefined || r.percentage !== undefined
    );

  return !hasTopLevelError && !hasRowError;
}

export function ListingForm({ listing, onSuccess, onCancel }: ListingFormProps) {
  const isEdit = !!listing;
  const { publicKey } = useWalletContext();
  const { tokens: availableTokens } = useSupportedTokens();

  const { create, isCreating, progress: createProgress, error: createError } =
    useCreateListing(publicKey);
  const { update, isUpdating, progress: updateProgress, error: updateError } =
    useUpdateListing(publicKey);

  const [form, setForm] = useState<FormState>({
    collectionAddress: "",
    nftTokenId: 0,
    price: 10,
    tokenAddress: DEFAULT_TOKEN.address,
    recipients: [{ address: publicKey ?? "", percentage: 100 }],
  });
  const [touched, setTouched] = useState<Set<string>>(new Set());
  const [submitAttempted, setSubmitAttempted] = useState(false);
  const [successId, setSuccessId] = useState<number | null>(null);
  const [currentMetadata, setCurrentMetadata] = useState<ArtworkMetadata | null>(null);
  const [isFetchingMetadata, setIsFetchingMetadata] = useState(false);

  const tokenOptions = listing
    ? ensureTokenOption(availableTokens, form.tokenAddress)
    : availableTokens;
  const hasTokenOptions = tokenOptions.length > 0;
  const defaultToken = getDefaultSupportedToken(tokenOptions);
  const selectedToken =
    tokenOptions.find((token) => token.address === form.tokenAddress) || defaultToken;

  const errors = useMemo(() => validateListingForm(form), [form]);
  const formIsValid = useMemo(() => isFormValid(errors), [errors]);

  // Load existing data if in edit mode
  useEffect(() => {
    if (listing) {
      setIsFetchingMetadata(true);
      fetchMetadata(listing.metadata_cid ?? "")
        .then((meta) => {
          setCurrentMetadata(meta);
          const existingRecipients =
            listing.recipients && listing.recipients.length > 0
              ? listing.recipients.map((r) => ({
                  address: r.address,
                  percentage: r.percentage,
                }))
              : [{ address: listing.artist, percentage: 100 }];
          setForm({
            collectionAddress: listing.collection,
            nftTokenId: Number(listing.token_id),
            price: parseFloat(stroopsToXlm(listing.price)),
            tokenAddress: listing.token,
            recipients: existingRecipients,
          });
        })
        .finally(() => setIsFetchingMetadata(false));
    }
  }, [listing]);

  // Sync default publicKey into recipient[0] on create mode when wallet connects
  useEffect(() => {
    if (!isEdit && publicKey && form.recipients[0]?.address === "") {
      setForm((cur) => ({
        ...cur,
        recipients: [{ address: publicKey, percentage: 100 }, ...cur.recipients.slice(1)],
      }));
    }
  }, [publicKey, isEdit]);

  // Snap to valid token when token list loads (create mode only)
  useEffect(() => {
    if (isEdit || tokenOptions.length === 0) return;
    if (!tokenOptions.some((token) => token.address === form.tokenAddress)) {
      setForm((current) => ({
        ...current,
        tokenAddress: getDefaultSupportedToken(tokenOptions).address,
      }));
    }
  }, [form.tokenAddress, isEdit, tokenOptions]);

  // ── Field helpers ─────────────────────────────────────────

  function markTouched(field: string) {
    setTouched((prev) => new Set(prev).add(field));
  }

  function shouldShowError(field: string): boolean {
    return submitAttempted || touched.has(field);
  }

  // ── Recipients helpers ────────────────────────────────────

  function addRecipient() {
    if (form.recipients.length >= MAX_RECIPIENTS) return;
    setForm((cur) => ({
      ...cur,
      recipients: [...cur.recipients, { address: "", percentage: 0 }],
    }));
  }

  function removeRecipient(index: number) {
    if (form.recipients.length <= 1) return;
    setForm((cur) => ({
      ...cur,
      recipients: cur.recipients.filter((_, i) => i !== index),
    }));
  }

  function updateRecipient(index: number, field: "address" | "percentage", value: string | number) {
    setForm((cur) => ({
      ...cur,
      recipients: cur.recipients.map((r, i) =>
        i === index ? { ...r, [field]: value } : r
      ),
    }));
    markTouched(`recipient_${index}_${field}`);
  }

  // ── Submit ────────────────────────────────────────────────

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitAttempted(true);

    if (!isFormValid(errors)) return;

    if (isEdit && listing && currentMetadata) {
      const success = await update({
        listingId: listing.listing_id,
        originalTokenAddress: listing.token,
        collectionAddress: form.collectionAddress,
        nftTokenId: form.nftTokenId,
        price: form.price,
        tokenAddress: form.tokenAddress,
        title: currentMetadata.title ?? "",
        description: currentMetadata.description ?? "",
        artistName: currentMetadata.artist ?? "",
        year: currentMetadata.year ?? "",
        category: currentMetadata.category ?? "",
        currentMetadata,
      });
      if (success) {
        setSuccessId(listing.listing_id);
        onSuccess?.(listing.listing_id);
      }
    } else if (!isEdit) {
      const id = await create({
        collectionAddress: form.collectionAddress,
        nftTokenId: form.nftTokenId,
        price: form.price,
        tokenAddress: form.tokenAddress,
        recipients: form.recipients,
      });
      if (id !== null) {
        setSuccessId(id);
        posthog.capture("Listing Created", { listing_id: id, price_xlm: form.price });
        onSuccess?.(id);
      }
    }
  };

  const isLoading = isCreating || isUpdating || isFetchingMetadata;
  const progress = isEdit ? updateProgress : createProgress;
  const error = isEdit ? updateError : createError;

  // ── Success screen ────────────────────────────────────────

  if (successId !== null) {
    return (
      <div className="max-w-xl mx-auto flex flex-col items-center gap-6 rounded-3xl border border-green-100 bg-white p-12 text-center shadow-2xl shadow-green-900/5">
        <div className="rounded-full bg-green-50 p-4">
          <CheckCircle size={56} className="text-green-500" />
        </div>
        <div className="space-y-2">
          <h3 className="text-3xl font-display font-bold text-gray-900">
            Listing #{successId} {isEdit ? "Updated" : "Created"}!
          </h3>
          <p className="text-gray-500 font-inter">
            Your artwork is now live and available for purchase on the ELCARE-HUB marketplace.
          </p>
        </div>
        <div className="flex flex-col sm:flex-row gap-4 w-full mt-4">
          {!isEdit && (
            <button
              onClick={() => {
                setSuccessId(null);
                setSubmitAttempted(false);
                setTouched(new Set());
                setForm({
                  collectionAddress: "",
                  nftTokenId: 0,
                  price: 10,
                  tokenAddress: defaultToken.address,
                  recipients: [{ address: publicKey ?? "", percentage: 100 }],
                });
              }}
              className="flex-1 rounded-2xl bg-brand-500 px-6 py-4 text-lg font-bold text-white hover:bg-brand-600 shadow-lg shadow-brand-500/20 transition-all hover:scale-[1.02] active:scale-[0.98]"
            >
              List Another
            </button>
          )}
          <button
            onClick={onCancel}
            className="flex-1 rounded-2xl border border-gray-200 bg-white px-6 py-4 text-lg font-semibold text-gray-700 hover:bg-gray-50 transition-all"
          >
            Back to Dashboard
          </button>
        </div>
      </div>
    );
  }

  // ── Main render ───────────────────────────────────────────

  const recipientSum = form.recipients.reduce((s, r) => s + (r.percentage || 0), 0);

  return (
    <div className="max-w-3xl mx-auto px-4 py-8">
      <div className="bg-white rounded-3xl shadow-2xl shadow-brand-900/5 border border-brand-100/50 p-6 md:p-10">
        <header className="mb-10 text-center">
          <h2 className="text-4xl font-display font-bold text-gray-900 mb-2">
            {isEdit ? "Refine Your Masterpiece" : "List Your Artwork"}
          </h2>
          <p className="text-gray-500 font-inter">
            {isEdit
              ? "Update your listing details to attract more buyers."
              : "Share your creative vision with collectors across the globe."}
          </p>
        </header>

        <form onSubmit={handleSubmit} noValidate className="space-y-8">
          <div className="grid gap-6 sm:grid-cols-2">

            {/* Collection Address */}
            <div className="sm:col-span-2 space-y-2">
              <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                Collection Address *
              </label>
              <input
                value={form.collectionAddress}
                onChange={(e) => setForm({ ...form, collectionAddress: e.target.value })}
                onBlur={() => markTouched("collectionAddress")}
                aria-invalid={shouldShowError("collectionAddress") && !!errors.collectionAddress}
                aria-describedby={errors.collectionAddress ? "err-collection" : undefined}
                className={`w-full rounded-2xl border px-5 py-4 text-base focus:outline-none transition-all shadow-sm font-inter ${
                  shouldShowError("collectionAddress") && errors.collectionAddress
                    ? "border-red-400 bg-red-50/40 focus:border-red-500"
                    : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                }`}
                placeholder="e.g. C..."
              />
              {shouldShowError("collectionAddress") && errors.collectionAddress && (
                <p id="err-collection" className="text-sm text-red-600 mt-1" role="alert">
                  {errors.collectionAddress}
                </p>
              )}
            </div>

            {/* NFT Token ID */}
            <div className="sm:col-span-2 space-y-2">
              <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                NFT Token ID *
              </label>
              <input
                type="number"
                min={0}
                step={1}
                value={form.nftTokenId}
                onChange={(e) =>
                  setForm({ ...form, nftTokenId: parseInt(e.target.value, 10) || 0 })
                }
                onBlur={() => markTouched("nftTokenId")}
                aria-invalid={shouldShowError("nftTokenId") && !!errors.nftTokenId}
                aria-describedby={errors.nftTokenId ? "err-tokenid" : undefined}
                className={`w-full rounded-2xl border px-5 py-4 text-base focus:outline-none transition-all shadow-sm font-inter ${
                  shouldShowError("nftTokenId") && errors.nftTokenId
                    ? "border-red-400 bg-red-50/40 focus:border-red-500"
                    : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                }`}
              />
              {shouldShowError("nftTokenId") && errors.nftTokenId && (
                <p id="err-tokenid" className="text-sm text-red-600 mt-1" role="alert">
                  {errors.nftTokenId}
                </p>
              )}
            </div>

            {/* Price */}
            <div className="space-y-2">
              <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                Price ({selectedToken.symbol}) *
              </label>
              <div className="relative">
                <input
                  type="number"
                  min={MIN_PRICE_XLM}
                  max={MAX_PRICE_XLM}
                  step="any"
                  value={form.price}
                  onChange={(e) =>
                    setForm({ ...form, price: parseFloat(e.target.value) })
                  }
                  onBlur={() => markTouched("price")}
                  aria-invalid={shouldShowError("price") && !!errors.price}
                  aria-describedby={errors.price ? "err-price" : undefined}
                  className={`w-full rounded-2xl border px-5 py-4 pr-16 text-base focus:outline-none transition-all shadow-sm font-inter ${
                    shouldShowError("price") && errors.price
                      ? "border-red-400 bg-red-50/40 focus:border-red-500"
                      : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                  }`}
                />
                <span className="absolute right-5 top-1/2 -translate-y-1/2 text-sm font-bold text-brand-600">
                  {selectedToken.symbol}
                </span>
              </div>
              {shouldShowError("price") && errors.price && (
                <p id="err-price" className="text-sm text-red-600 mt-1" role="alert">
                  {errors.price}
                </p>
              )}
            </div>

            {/* Payment Token */}
            <div className="space-y-2">
              <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                Payment Token *
              </label>
              <select
                disabled={!hasTokenOptions || isEdit}
                value={form.tokenAddress}
                onChange={(e) => {
                  setForm({ ...form, tokenAddress: e.target.value });
                  markTouched("tokenAddress");
                }}
                onBlur={() => markTouched("tokenAddress")}
                aria-invalid={shouldShowError("tokenAddress") && !!errors.tokenAddress}
                className={`w-full appearance-none rounded-2xl border px-5 py-4 text-base focus:outline-none transition-all shadow-sm font-inter ${
                  shouldShowError("tokenAddress") && errors.tokenAddress
                    ? "border-red-400 bg-red-50/40 focus:border-red-500"
                    : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                }`}
              >
                {hasTokenOptions ? (
                  tokenOptions.map((token) => (
                    <option key={token.address} value={token.address}>
                      {token.name} ({token.symbol})
                    </option>
                  ))
                ) : (
                  <option value="">No supported tokens available</option>
                )}
              </select>
              {shouldShowError("tokenAddress") && errors.tokenAddress && (
                <p className="text-sm text-red-600 mt-1" role="alert">
                  {errors.tokenAddress}
                </p>
              )}
            </div>
          </div>

          {/* ── Royalty Recipients ── */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                  Revenue Split *
                </label>
                <p className="text-xs text-gray-500 mt-0.5 font-inter">
                  Percentages must sum to exactly 100%. Max {MAX_RECIPIENTS} recipients.
                </p>
              </div>
              {form.recipients.length < MAX_RECIPIENTS && (
                <button
                  type="button"
                  onClick={addRecipient}
                  className="flex items-center gap-1.5 rounded-xl border border-brand-200 bg-brand-50 px-3 py-2 text-sm font-semibold text-brand-700 hover:bg-brand-100 transition-all"
                >
                  <Plus size={14} />
                  Add Recipient
                </button>
              )}
            </div>

            <div className="space-y-3">
              {form.recipients.map((recipient, idx) => {
                const rowErrors = errors.recipientRows?.[idx];
                const addressTouched = shouldShowError(`recipient_${idx}_address`);
                const pctTouched = shouldShowError(`recipient_${idx}_percentage`);
                return (
                  <div key={idx} className="flex flex-col sm:flex-row gap-3 items-start">
                    <div className="w-full sm:flex-1 space-y-1">
                      <input
                        value={recipient.address}
                        onChange={(e) => updateRecipient(idx, "address", e.target.value)}
                        onBlur={() => markTouched(`recipient_${idx}_address`)}
                        placeholder="Stellar address (G...)"
                        aria-label={`Recipient ${idx + 1} address`}
                        aria-invalid={addressTouched && !!rowErrors?.address}
                        className={`w-full rounded-2xl border px-4 py-3 text-sm focus:outline-none transition-all font-inter ${
                          addressTouched && rowErrors?.address
                            ? "border-red-400 bg-red-50/40 focus:border-red-500"
                            : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                        }`}
                      />
                      {addressTouched && rowErrors?.address && (
                        <p className="text-xs text-red-600" role="alert">
                          {rowErrors.address}
                        </p>
                      )}
                    </div>
                    <div className="w-full sm:w-28 space-y-1">
                      <div className="relative">
                        <input
                          type="number"
                          min={1}
                          max={100}
                          step={1}
                          value={recipient.percentage}
                          onChange={(e) =>
                            updateRecipient(idx, "percentage", parseFloat(e.target.value) || 0)
                          }
                          onBlur={() => markTouched(`recipient_${idx}_percentage`)}
                          aria-label={`Recipient ${idx + 1} percentage`}
                          aria-invalid={pctTouched && !!rowErrors?.percentage}
                          className={`w-full rounded-2xl border px-4 py-3 pr-8 text-sm focus:outline-none transition-all font-inter ${
                            pctTouched && rowErrors?.percentage
                              ? "border-red-400 bg-red-50/40 focus:border-red-500"
                              : "border-gray-200 bg-gray-50/50 focus:border-brand-500 focus:bg-white"
                          }`}
                        />
                        <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs font-bold text-gray-400">
                          %
                        </span>
                      </div>
                      {pctTouched && rowErrors?.percentage && (
                        <p className="text-xs text-red-600" role="alert">
                          {rowErrors.percentage}
                        </p>
                      )}
                    </div>
                    <button
                      type="button"
                      onClick={() => removeRecipient(idx)}
                      disabled={form.recipients.length <= 1}
                      aria-label={`Remove recipient ${idx + 1}`}
                      className="mt-2.5 rounded-xl p-2.5 text-gray-400 hover:bg-red-50 hover:text-red-500 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                );
              })}
            </div>

            {/* Split sum indicator */}
            <div className="flex items-center justify-between text-sm">
              <span className="text-gray-500 font-inter">Total split:</span>
              <span
                className={`font-bold tabular-nums ${
                  Math.round(recipientSum) === REQUIRED_SPLIT_SUM
                    ? "text-green-600"
                    : "text-red-600"
                }`}
                aria-label={`Total recipient split: ${recipientSum}%`}
              >
                {recipientSum.toFixed(2)}%{" "}
                {Math.round(recipientSum) !== REQUIRED_SPLIT_SUM && (
                  <span className="text-xs font-normal text-red-500">
                    (must be 100%)
                  </span>
                )}
              </span>
            </div>

            {(shouldShowError("recipients") || submitAttempted) && errors.recipients && (
              <p className="text-sm text-red-600 mt-1" role="alert">
                {errors.recipients}
              </p>
            )}
          </div>

          {/* Progress / server error */}
          {isLoading && progress && (
            <div className="flex items-center gap-3 rounded-2xl bg-brand-50 px-6 py-4 text-sm font-semibold text-brand-700 animate-pulse">
              <Loader2 size={20} className="animate-spin" />
              {progress}
            </div>
          )}
          {error && (
            <p className="rounded-2xl bg-red-50 px-6 py-4 text-sm font-bold text-red-600 border border-red-100">
              {error}
            </p>
          )}

          {/* Buttons */}
          <div className="flex flex-col sm:flex-row gap-4 pt-4">
            {isEdit && (
              <button
                type="button"
                onClick={onCancel}
                disabled={isLoading}
                className="flex-1 rounded-2xl border border-gray-200 py-4 text-lg font-semibold text-gray-600 hover:bg-gray-50 transition-all disabled:opacity-50"
              >
                Cancel
              </button>
            )}
            <GuardButton
              type="submit"
              disabled={isLoading || !hasTokenOptions || (submitAttempted && !formIsValid)}
              actionName={isEdit ? "to update your listing" : "to list your artwork"}
              className="flex-[2] flex items-center justify-center gap-3 rounded-2xl bg-brand-500 py-5 text-xl font-bold text-white shadow-2xl shadow-brand-500/30 hover:bg-brand-600 hover:scale-[1.01] transition-all active:scale-[0.98] disabled:opacity-50 disabled:hover:scale-100"
            >
              {isLoading ? (
                <>
                  <Loader2 size={24} className="animate-spin" />
                  {progress || "Processing…"}
                </>
              ) : (
                <>
                  {isEdit ? <Save size={24} /> : <Upload size={24} />}
                  {isEdit ? "Update Listing" : "Create Listing"}
                </>
              )}
            </GuardButton>
          </div>
        </form>
      </div>
    </div>
  );
}
