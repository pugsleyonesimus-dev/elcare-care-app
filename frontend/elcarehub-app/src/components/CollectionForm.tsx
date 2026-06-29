"use client";

import { useEffect, useMemo, useState } from "react";
import { useDeployCollection, DeployCollectionInput } from "@/hooks/useLaunchpad";
import { useWalletContext } from "@/context/WalletContext";
import { useToast } from "@/components/ToastProvider";
import { Loader2, Rocket, CheckCircle, ArrowRight, ArrowLeft, Check } from "lucide-react";
import { GuardButton } from "./WalletGuard";
import { CollectionKind } from "@/lib/launchpad";
import { DEFAULT_TOKEN } from "@/config/tokens";
import { useSupportedTokens } from "@/hooks/useSupportedTokens";
import { getDefaultSupportedToken } from "@/lib/token-support";

const STEPS = ["Collection Kind", "Details", "Economics", "Review"] as const;

const KIND_OPTIONS = [
  { id: "Normal721", label: "Standard 721", desc: "Classic one-of-a-kind NFTs" },
  { id: "Normal1155", label: "Standard 1155", desc: "Multi-edition fungible tokens" },
  { id: "LazyMint721", label: "Lazy 721", desc: "Mint only when sold (Gasless)" },
  { id: "LazyMint1155", label: "Lazy 1155", desc: "Multi-edition lazy minting" },
] as const;

export function CollectionForm() {
  const { publicKey } = useWalletContext();
  const { deploy, isDeploying, error } = useDeployCollection(publicKey);
  const { pushToast } = useToast();
  const { tokens: supportedTokens } = useSupportedTokens();
  const hasSupportedTokens = supportedTokens.length > 0;

  const [step, setStep] = useState(0);
  const [successAddress, setSuccessAddress] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    symbol: "",
    kind: "Normal721" as CollectionKind,
    maxSupply: 10000,
    royaltyBps: 500,
    royaltyReceiver: publicKey || "",
    currencyAddress: DEFAULT_TOKEN.address,
  });

  useEffect(() => {
    if (supportedTokens.length === 0) return;
    if (!supportedTokens.some((token) => token.address === form.currencyAddress)) {
      setForm((current) => ({
        ...current,
        currencyAddress: getDefaultSupportedToken(supportedTokens).address,
      }));
    }
  }, [form.currencyAddress, supportedTokens]);

  const is721 = form.kind.includes("721");

  const stepValid = useMemo(() => {
    switch (step) {
      case 0:
        return true;
      case 1:
        if (!form.name.trim()) return false;
        if (is721 && !form.symbol.trim()) return false;
        if (is721 && form.maxSupply < 1) return false;
        return true;
      case 2:
        return form.royaltyBps >= 0 && form.royaltyBps <= 10000 && hasSupportedTokens;
      case 3:
        return true;
      default:
        return false;
    }
  }, [step, form, is721, hasSupportedTokens]);

  const handleDeploy = async () => {
    if (!publicKey) return;

    const input: DeployCollectionInput = {
      ...form,
      royaltyReceiver: form.royaltyReceiver || publicKey,
    };

    if (form.kind.startsWith("LazyMint")) {
      try {
        const sdk = await import("@stellar/stellar-sdk");
        const decoded = sdk.StrKey.decodeEd25519PublicKey(publicKey);
        input.creatorPubkeyBytes = Buffer.from(decoded);
      } catch (err) {
        console.error("Failed to decode public key", err);
        pushToast("Failed to decode wallet public key.", "error");
        return;
      }
    }

    pushToast("Deploying your collection…", "info");
    const addr = await deploy(input);
    if (addr) {
      pushToast("Collection deployed successfully!", "success");
      setSuccessAddress(addr);
    } else {
      pushToast(error || "Deployment failed. Please try again.", "error");
    }
  };

  if (successAddress) {
    return (
      <div className="max-w-xl mx-auto flex flex-col items-center gap-6 rounded-3xl border border-green-100 bg-white p-12 text-center shadow-2xl shadow-green-900/5">
        <div className="rounded-full bg-green-50 p-4">
          <CheckCircle size={56} className="text-green-500" />
        </div>
        <div className="space-y-2">
          <h3 className="text-3xl font-display font-bold text-gray-900">
            Collection Deployed!
          </h3>
          <p className="text-gray-500 font-inter">
            Your collection has been successfully created on the Stellar network.
          </p>
          <div className="mt-4 p-4 bg-gray-50 rounded-2xl break-all font-mono text-sm text-gray-600 border border-gray-100">
            {successAddress}
          </div>
        </div>
        <a
          href={`/launchpad/collections/${successAddress}`}
          className="w-full flex items-center justify-center gap-2 rounded-2xl bg-brand-500 px-6 py-4 text-lg font-bold text-white hover:bg-brand-600 shadow-lg shadow-brand-500/20 transition-all hover:scale-[1.02] active:scale-[0.98]"
        >
          View Collection <ArrowRight size={20} />
        </a>
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto px-4 py-8">
      {/* Step indicator */}
      <div className="flex items-center justify-between mb-10">
        {STEPS.map((label, i) => (
          <div key={label} className="flex items-center flex-1">
            <div className="flex flex-col items-center">
              <div
                className={`w-9 h-9 rounded-full flex items-center justify-center font-bold text-sm border-2 transition-all ${
                  i < step
                    ? "bg-brand-500 border-brand-500 text-white"
                    : i === step
                    ? "border-brand-500 text-brand-600 bg-brand-50"
                    : "border-gray-200 text-gray-400 bg-white"
                }`}
              >
                {i < step ? <Check size={16} /> : i + 1}
              </div>
              <span
                className={`mt-1.5 text-xs font-bold hidden sm:block ${
                  i <= step ? "text-brand-600" : "text-gray-400"
                }`}
              >
                {label}
              </span>
            </div>
            {i < STEPS.length - 1 && (
              <div
                className={`flex-1 h-0.5 mx-2 transition-all ${
                  i < step ? "bg-brand-500" : "bg-gray-200"
                }`}
              />
            )}
          </div>
        ))}
      </div>

      <div className="bg-white rounded-3xl shadow-2xl shadow-brand-900/5 border border-brand-100/50 p-6 md:p-10">
        {/* Step 0: Kind Selection */}
        {step === 0 && (
          <div>
            <h2 className="text-2xl font-display font-bold text-gray-900 mb-2">
              Choose Collection Type
            </h2>
            <p className="text-gray-500 font-inter mb-8">
              Select the kind of NFT collection you want to deploy.
            </p>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              {KIND_OPTIONS.map((type) => (
                <label
                  key={type.id}
                  className={`flex flex-col p-5 rounded-2xl border-2 cursor-pointer transition-all ${
                    form.kind === type.id
                      ? "border-brand-500 bg-brand-50/50"
                      : "border-gray-100 bg-gray-50/30 hover:border-brand-200"
                  }`}
                >
                  <input
                    type="radio"
                    name="kind"
                    value={type.id}
                    checked={form.kind === type.id}
                    onChange={(e) =>
                      setForm({ ...form, kind: e.target.value as CollectionKind })
                    }
                    className="sr-only"
                  />
                  <span className="font-bold text-gray-900 text-base">{type.label}</span>
                  <span className="text-sm text-gray-500 mt-1">{type.desc}</span>
                </label>
              ))}
            </div>
          </div>
        )}

        {/* Step 1: Metadata */}
        {step === 1 && (
          <div>
            <h2 className="text-2xl font-display font-bold text-gray-900 mb-2">
              Collection Details
            </h2>
            <p className="text-gray-500 font-inter mb-8">
              Name and describe your collection.
            </p>
            <div className="space-y-6">
              <div className="space-y-2">
                <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                  Collection Name *
                </label>
                <input
                  required
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  className="w-full rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter"
                  placeholder="e.g. African Legends"
                />
              </div>

              {is721 && (
                <>
                  <div className="space-y-2">
                    <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                      Symbol *
                    </label>
                    <input
                      required
                      value={form.symbol}
                      onChange={(e) =>
                        setForm({ ...form, symbol: e.target.value.toUpperCase() })
                      }
                      className="w-full rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter"
                      placeholder="e.g. AFRL"
                      maxLength={10}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                      Max Supply *
                    </label>
                    <input
                      required
                      type="number"
                      min={1}
                      value={form.maxSupply}
                      onChange={(e) =>
                        setForm({ ...form, maxSupply: parseInt(e.target.value) || 1 })
                      }
                      className="w-full rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter"
                    />
                  </div>
                </>
              )}
            </div>
          </div>
        )}

        {/* Step 2: Economics */}
        {step === 2 && (
          <div>
            <h2 className="text-2xl font-display font-bold text-gray-900 mb-2">
              Economics
            </h2>
            <p className="text-gray-500 font-inter mb-8">
              Set royalties and the fee token for your collection.
            </p>
            <div className="space-y-6">
              <div className="space-y-2">
                <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                  Royalty (BPS) *
                </label>
                <div className="relative">
                  <input
                    required
                    type="number"
                    min={0}
                    max={10000}
                    value={form.royaltyBps}
                    onChange={(e) =>
                      setForm({ ...form, royaltyBps: parseInt(e.target.value) || 0 })
                    }
                    className="w-full rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 pr-16 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter"
                  />
                  <span className="absolute right-5 top-1/2 -translate-y-1/2 text-sm font-bold text-brand-600">
                    {((form.royaltyBps / 10000) * 100).toFixed(1)}%
                  </span>
                </div>
                <p className="text-xs text-gray-500 font-inter">
                  Basis points: 500 = 5%, 1000 = 10%
                </p>
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                  Fee Payment Token *
                </label>
                <select
                  required
                  disabled={!hasSupportedTokens}
                  value={form.currencyAddress}
                  onChange={(e) => setForm({ ...form, currencyAddress: e.target.value })}
                  className="w-full appearance-none rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter"
                >
                  {hasSupportedTokens ? (
                    supportedTokens.map((token) => (
                      <option key={token.address} value={token.address}>
                        {token.name} ({token.symbol})
                      </option>
                    ))
                  ) : (
                    <option value="">No supported tokens available</option>
                  )}
                </select>
              </div>

              <div className="space-y-2">
                <label className="block text-sm font-bold text-gray-950 uppercase tracking-wider font-inter">
                  Royalty Receiver Address
                </label>
                <input
                  value={form.royaltyReceiver}
                  onChange={(e) => setForm({ ...form, royaltyReceiver: e.target.value })}
                  className="w-full rounded-2xl border border-gray-200 bg-gray-50/50 px-5 py-4 text-base focus:border-brand-500 focus:bg-white focus:outline-none transition-all shadow-sm font-inter font-mono text-sm"
                  placeholder={publicKey || "G... (defaults to creator)"}
                />
              </div>
            </div>
          </div>
        )}

        {/* Step 3: Review & Deploy */}
        {step === 3 && (
          <div>
            <h2 className="text-2xl font-display font-bold text-gray-900 mb-2">
              Review &amp; Deploy
            </h2>
            <p className="text-gray-500 font-inter mb-8">
              Confirm your collection settings before deploying to Stellar.
            </p>

            <div className="space-y-3 mb-8 rounded-2xl border border-gray-100 bg-gray-50/50 p-6">
              {[
                { label: "Collection Type", value: form.kind },
                { label: "Name", value: form.name || "—" },
                ...(is721
                  ? [
                      { label: "Symbol", value: form.symbol || "—" },
                      { label: "Max Supply", value: form.maxSupply.toLocaleString() },
                    ]
                  : []),
                {
                  label: "Royalty",
                  value: `${form.royaltyBps} BPS (${((form.royaltyBps / 10000) * 100).toFixed(1)}%)`,
                },
                {
                  label: "Fee Token",
                  value:
                    supportedTokens.find((t) => t.address === form.currencyAddress)
                      ?.symbol ?? form.currencyAddress,
                },
                {
                  label: "Royalty Receiver",
                  value: form.royaltyReceiver || publicKey || "—",
                  mono: true,
                },
              ].map(({ label, value, mono }) => (
                <div
                  key={label}
                  className="flex flex-wrap items-start justify-between gap-2 py-3 border-b border-gray-100 last:border-0"
                >
                  <span className="text-sm font-bold text-gray-500 uppercase tracking-wider font-inter">
                    {label}
                  </span>
                  <span
                    className={`text-sm font-medium text-gray-900 text-right break-all ${
                      mono ? "font-mono" : ""
                    }`}
                  >
                    {value}
                  </span>
                </div>
              ))}
            </div>

            {error && (
              <p className="rounded-2xl bg-red-50 px-6 py-4 text-sm font-bold text-red-600 border border-red-100 mb-6">
                {error}
              </p>
            )}

            <GuardButton
              type="button"
              disabled={isDeploying || !hasSupportedTokens}
              actionName="to deploy your collection"
              onAction={handleDeploy}
              className="w-full flex items-center justify-center gap-3 rounded-2xl bg-brand-500 py-5 text-xl font-bold text-white shadow-2xl shadow-brand-500/30 hover:bg-brand-600 hover:scale-[1.01] transition-all active:scale-[0.98] disabled:opacity-50 disabled:hover:scale-100"
            >
              {isDeploying ? (
                <>
                  <Loader2 size={24} className="animate-spin" />
                  Deploying to Stellar…
                </>
              ) : (
                <>
                  <Rocket size={24} />
                  Deploy Collection
                </>
              )}
            </GuardButton>
          </div>
        )}

        {/* Step navigation */}
        <div className="flex justify-between mt-10 pt-6 border-t border-gray-100">
          {step > 0 ? (
            <button
              type="button"
              onClick={() => setStep((s) => s - 1)}
              className="flex items-center gap-2 px-6 py-3 rounded-xl border border-gray-200 bg-white text-gray-700 font-bold hover:bg-gray-50 transition-all"
            >
              <ArrowLeft size={18} />
              Back
            </button>
          ) : (
            <div />
          )}

          {step < STEPS.length - 1 && (
            <button
              type="button"
              disabled={!stepValid}
              onClick={() => setStep((s) => s + 1)}
              className="flex items-center gap-2 px-8 py-3 rounded-xl bg-brand-500 text-white font-bold hover:bg-brand-600 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Next
              <ArrowRight size={18} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
