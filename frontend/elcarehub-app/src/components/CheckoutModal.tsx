"use client";

import { useState, useEffect } from "react";
import {
  X,
  CreditCard,
  Wallet,
  Loader2,
  CheckCircle2,
} from "lucide-react";
import { Listing, stroopsToXlm, getProtocolFee } from "@/lib/contract";
import { useSupportedTokens } from "@/hooks/useSupportedTokens";
import { TokenConfig, getTokenConfigByAddress } from "@/config/tokens";
import { resolveSupportedTokens, getDefaultSupportedToken } from "@/lib/token-support";
import posthog from "posthog-js";
import { useModalA11y } from "@/hooks/useModalA11y";

interface CheckoutModalProps {
  isOpen: boolean;
  onClose: () => void;
  listing: Listing;
  onCryptoPurchase: () => Promise<boolean>;
  onPurchased?: () => void;
  isBuyingCrypto: boolean;
}

export function CheckoutModal({
  isOpen,
  onClose,
  listing,
  onCryptoPurchase,
  onPurchased,
  isBuyingCrypto,
}: CheckoutModalProps) {
  const { dialogRef, titleId } = useModalA11y(isOpen, onClose);
  const [method, setMethod] = useState<"crypto" | "fiat">("crypto");
  const [selectedToken, setSelectedToken] = useState<TokenConfig | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [protocolFee, setProtocolFee] = useState(0);
  const { tokens: allTokens, isLoading: loadingTokens, error: tokensError } = useSupportedTokens();
  const [supportedTokens, setSupportedTokens] = useState<TokenConfig[]>([]);

  useEffect(() => {
    if (!isOpen) {
      setConfirmed(false);
    }
  }, [isOpen]);

  useEffect(() => {
    const init = async () => {
      if (allTokens.length > 0) {
        const tokens = resolveSupportedTokens([]);
        setSupportedTokens(tokens);
        
        const listingToken = getTokenConfigByAddress(listing.token);
        const defaultToken = listingToken || getDefaultSupportedToken(tokens);
        setSelectedToken(defaultToken);

        const fee = await getProtocolFee();
        setProtocolFee(fee);
      }
    };
    if (isOpen) {
      init();
    }
  }, [isOpen, allTokens, listing.token]);

  if (!isOpen || !selectedToken) return null;

  const priceXlm = Number(stroopsToXlm(listing.price));
  const estimatedFiat = (priceXlm * 0.12).toFixed(2);
  
  // Calculate fee breakdown
  const protocolFeeAmount = (priceXlm * protocolFee) / 10000;
  const royaltyAmount = 0; // Placeholder for future royalty implementation
  const totalAmount = priceXlm + protocolFeeAmount + royaltyAmount;

  const handleCryptoPurchase = async () => {
    if (!confirmed) {
      setConfirmed(true);
      return;
    }
    
    const success = await onCryptoPurchase();
    if (success) {
      posthog.capture("Purchase Successful", {
        listing_id: listing.listing_id,
        price_xlm: priceXlm,
        method: "crypto",
      });
      onPurchased?.();
      onClose();
      setConfirmed(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-midnight-950/80 backdrop-blur-sm"
        onClick={onClose}
        aria-hidden="true"
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-testid="checkout-modal"
        tabIndex={-1}
        className="relative w-full max-w-md overflow-hidden rounded-3xl bg-white shadow-2xl animate-scale-in outline-none"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-100 p-6">
          <h2 id={titleId} className="font-display text-xl font-bold text-gray-900">
            Checkout
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close checkout"
            className="rounded-full p-2 text-gray-700 hover:bg-gray-100 hover:text-gray-900 transition"
          >
            <X size={20} aria-hidden="true" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6">
          {/* Token Selection */}
          <div className="space-y-3">
            <h3 className="text-sm font-bold uppercase tracking-wider text-gray-500">
              Payment Token
            </h3>
            {loadingTokens ? (
              <div className="flex items-center gap-2 text-gray-400 text-sm">
                <Loader2 className="animate-spin" size={16} />
                Loading tokens...
              </div>
            ) : tokensError ? (
              <div className="text-red-500 text-sm">{tokensError}</div>
            ) : (
              <div className="grid grid-cols-1 gap-2">
                {supportedTokens.map((token) => (
                  <button
                    key={token.address}
                    onClick={() => setSelectedToken(token)}
                    className={`flex items-center justify-between p-4 rounded-2xl border-2 transition-all ${
                      selectedToken.address === token.address
                        ? "border-brand-500 bg-brand-50 text-brand-600"
                        : "border-gray-100 hover:border-gray-200 text-gray-600"
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-full bg-gray-100 flex items-center justify-center font-bold text-gray-700">
                        {token.symbol.charAt(0)}
                      </div>
                      <div className="text-left">
                        <p className="font-semibold">{token.symbol}</p>
                        <p className="text-xs text-gray-400">{token.name}</p>
                      </div>
                    </div>
                    {selectedToken.address === token.address && (
                      <CheckCircle2 size={20} />
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Fee Breakdown */}
          <div className="space-y-3">
            <h3 className="text-sm font-bold uppercase tracking-wider text-gray-500">
              Breakdown
            </h3>
            <div className="rounded-2xl bg-gray-50 p-4 space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-gray-600">Item Price</span>
                <span className="font-semibold text-gray-900">{priceXlm} {selectedToken.symbol}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-gray-600">Protocol Fee ({protocolFee / 100}%)</span>
                <span className="font-semibold text-gray-900">{protocolFeeAmount.toFixed(7)} {selectedToken.symbol}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-gray-600">Royalties</span>
                <span className="font-semibold text-gray-900">{royaltyAmount.toFixed(7)} {selectedToken.symbol}</span>
              </div>
              <div className="border-t border-gray-200 pt-3 flex justify-between items-center">
                <span className="font-bold text-gray-900">Total</span>
                <span className="font-display text-xl font-bold text-brand-600">
                  {totalAmount.toFixed(7)} {selectedToken.symbol}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Estimated</span>
                <span className="text-sm font-semibold text-brand-500">~${estimatedFiat}</span>
              </div>
            </div>
          </div>

          {/* Payment method selector */}
          <div className="space-y-4">
            <h3 className="text-sm font-bold uppercase tracking-wider text-gray-500">
              Select Payment Method
            </h3>

            <div className="grid grid-cols-2 gap-3">
              <button
                onClick={() => setMethod("crypto")}
                className={`flex flex-col items-center gap-3 rounded-2xl border-2 p-4 transition-all ${method === "crypto" ? "border-brand-500 bg-brand-50 text-brand-600" : "border-gray-100 hover:border-gray-200 text-gray-600"}`}
              >
                <Wallet size={24} />
                <span className="text-sm font-semibold">Crypto</span>
              </button>

              {/* Fiat payment — coming soon */}
              <div className="relative flex flex-col items-center gap-3 rounded-2xl border-2 border-gray-100 p-4 text-gray-400 cursor-not-allowed select-none opacity-60">
                <CreditCard size={24} />
                <span className="text-sm font-semibold">Credit Card</span>
                <span className="absolute -top-2 right-2 rounded-full bg-orange-100 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-orange-600">
                  Coming Soon
                </span>
              </div>
            </div>

            <button
              type="button"
              data-testid="checkout-pay-button"
              onClick={handleCryptoPurchase}
              disabled={isBuyingCrypto}
              className="mt-2 flex w-full items-center justify-center gap-2 rounded-2xl bg-brand-500 py-5 font-bold text-white shadow-lg shadow-brand-500/20 hover:bg-brand-600 transition-all disabled:opacity-50"
            >
              {isBuyingCrypto ? (
                <>
                  <Loader2 className="animate-spin" size={18} /> Processing...
                </>
              ) : confirmed ? (
                "Confirm & Pay"
              ) : (
                `Pay ${priceXlm} ${selectedToken.symbol}`
              )}
            </button>
            
            {confirmed && (
              <p className="text-center text-sm text-gray-500">
                Click again to confirm and complete your purchase
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
