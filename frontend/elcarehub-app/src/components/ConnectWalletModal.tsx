// -------------------------------------------------------------
// components/ConnectWalletModal.tsx
// Wallet chooser: Freighter — Lobstr — Magic (email/passkey)
// -------------------------------------------------------------

"use client";

import { useEffect, useState } from "react";
import { useWalletContext } from "@/context/WalletContext";
import {
  X,
  Wallet,
  ExternalLink,
  ShieldCheck,
  AlertTriangle,
  ArrowRight,
  Loader2,
  CheckCircle2,
  Mail,
} from "lucide-react";
import { config } from "@/lib/config";
import { MagicWalletModal } from "./MagicWalletModal";
import posthog from "posthog-js";

interface ConnectWalletModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type Choosing = "idle" | "freighter" | "lobstr" | "magic";

export function ConnectWalletModal({ isOpen, onClose }: ConnectWalletModalProps) {
  const {
    isConnected,
    publicKey,
    refresh,
    freighter,
    lobstr,
    magic,
    connectFreighter,
    connectLobstr,
  } = useWalletContext();

  const [choosing, setChoosing] = useState<Choosing>("idle");
  const [showMagicModal, setShowMagicModal] = useState(false);

  // Close when any wallet connects
  useEffect(() => {
    if (isConnected && choosing !== "idle") {
      posthog.capture("Wallet Connected", { type: choosing });
      const t = setTimeout(onClose, 900);
      return () => clearTimeout(t);
    }
  }, [isConnected, choosing, onClose]);

  if (!isOpen) return null;

  // -- Per-wallet helpers -------------------------------------

  const handleFreighter = async () => {
    setChoosing("freighter");
    await connectFreighter();
  };

  const handleLobstr = async () => {
    setChoosing("lobstr");
    await connectLobstr();
  };

  const handleMagic = () => {
    setChoosing("magic");
    setShowMagicModal(true);
  };

  // -- Shared state shortcuts ---------------------------------
  const freighterConnecting = choosing === "freighter" && freighter.isConnecting;
  const lobstrConnecting = choosing === "lobstr" && lobstr.isConnecting;
  const anyConnecting = freighterConnecting || lobstrConnecting || magic.isConnecting;

  const freighterNotInstalled =
    !freighter.isInstalled && !freighterConnecting;
  const lobstrNotInstalled = !lobstr.isInstalled && !lobstrConnecting;

  const wrongNetwork =
    (choosing === "freighter" && freighter.isWrongNetwork) ||
    (choosing === "lobstr" && lobstr.isWrongNetwork);

  const activeError =
    choosing === "freighter"
      ? freighter.error
      : choosing === "lobstr"
      ? lobstr.error
      : magic.error;

  // -- Render -------------------------------------------------

  return (
    <>
      <MagicWalletModal
        isOpen={showMagicModal}
        onClose={() => {
          setShowMagicModal(false);
          if (!magic.isConnected) setChoosing("idle");
        }}
      />

      <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 sm:p-6">
        {/* Backdrop */}
        <div
          className="absolute inset-0 bg-midnight-950/80 backdrop-blur-md animate-fade-in"
          onClick={onClose}
        />

        {/* Card */}
        <div className="relative w-full max-w-md overflow-hidden rounded-3xl bg-white shadow-2xl shadow-black/50 animate-scale-in">
          <div className="tribal-strip h-2" />

          {/* Header */}
          <div className="flex items-center justify-between p-6 pb-0">
            <h2 className="font-display text-2xl font-bold text-midnight-900">
              Connect <span className="text-brand-500">Wallet</span>
            </h2>
            <button
              onClick={onClose}
              className="rounded-full p-2 text-gray-400 hover:bg-gray-100 hover:text-midnight-900 transition-colors"
            >
              <X size={20} />
            </button>
          </div>

          <div className="p-6 pt-4">
            <p className="text-sm text-gray-500 mb-6 font-medium">
              Choose how you want to connect to ELCARE-HUB.
            </p>

            {/* -- Connected -- */}
            {isConnected ? (
              <div className="rounded-2xl border-2 border-mint-100 bg-mint-50/30 p-8 text-center animate-fade-in">
                <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-mint-100 text-mint-600">
                  <CheckCircle2 size={32} />
                </div>
                <h3 className="font-display font-bold text-midnight-900 text-xl">
                  Connected!
                </h3>
                <p className="mt-2 text-sm text-mint-800">
                  Your wallet is connected to ELCARE-HUB.
                </p>
                <p className="mt-3 font-mono text-[10px] text-mint-700/60 break-all px-4">
                  {publicKey}
                </p>
              </div>
            ) : wrongNetwork ? (
              /* -- Wrong network -- */
              <div className="rounded-2xl border-2 border-terracotta-100 bg-terracotta-50/30 p-5 text-center">
                <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-terracotta-100 text-terracotta-600">
                  <AlertTriangle size={24} />
                </div>
                <h3 className="font-display font-bold text-midnight-900">
                  Wrong Network
                </h3>
                <p className="mt-2 text-xs text-terracotta-800">
                  Please switch your wallet to{" "}
                  <b>{config.network}</b> and try again.
                </p>
                <button
                  onClick={refresh}
                  className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-terracotta-500 py-3 text-sm font-bold text-white hover:bg-terracotta-600 transition-all"
                >
                  Refresh Connection
                </button>
              </div>
            ) : (
              /* -- Wallet chooser -- */
              <div className="space-y-3">

                {/* -- Freighter -- */}
                {freighterNotInstalled ? (
                  <div className="rounded-2xl border-2 border-gray-100 p-4 flex items-center gap-4">
                    <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-brand-100 text-brand-400 flex-shrink-0">
                      <Wallet size={24} />
                    </div>
                    <div className="flex-1 text-left">
                      <p className="font-bold text-midnight-900">Freighter</p>
                      <p className="text-xs text-gray-500">Extension not detected</p>
                    </div>
                    <a
                      href="https://www.freighter.app/"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 text-xs font-bold text-brand-500 hover:underline"
                    >
                      Install <ExternalLink size={12} />
                    </a>
                  </div>
                ) : (
                  <button
                    onClick={handleFreighter}
                    disabled={anyConnecting}
                    className="group relative flex w-full items-center gap-4 rounded-2xl border-2 border-gray-100 p-4 hover:border-brand-300 hover:bg-brand-50/30 transition-all duration-300 disabled:opacity-60"
                  >
                    <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-brand-100 text-brand-600 group-hover:bg-brand-500 group-hover:text-white transition-colors flex-shrink-0">
                      {freighterConnecting ? (
                        <Loader2 size={24} className="animate-spin" />
                      ) : (
                        <Wallet size={24} />
                      )}
                    </div>
                    <div className="text-left">
                      <p className="font-bold text-midnight-900">Freighter</p>
                      <p className="text-xs text-gray-500">Official Stellar Wallet</p>
                    </div>
                    <ArrowRight
                      size={18}
                      className="absolute right-4 text-gray-300 group-hover:text-brand-500 group-hover:translate-x-1 transition-all"
                    />
                  </button>
                )}

                {/* -- Lobstr -- */}
                {lobstrNotInstalled ? (
                  <div className="rounded-2xl border-2 border-gray-100 p-4 flex items-center gap-4">
                    {/* Lobstr logo */}
                    <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#0B1E3E]/10 flex-shrink-0">
                      <svg width="28" height="28" viewBox="0 0 512 512" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <rect width="512" height="512" rx="100" fill="#0B1E3E"/>
                        <path d="M256 96C167.6 96 96 167.6 96 256s71.6 160 160 160 160-71.6 160-160S344.4 96 256 96zm0 280c-66.3 0-120-53.7-120-120s53.7-120 120-120 120 53.7 120 120-53.7 120-120 120z" fill="#FBBF24"/>
                        <circle cx="256" cy="256" r="50" fill="#FBBF24"/>
                      </svg>
                    </div>
                    <div className="flex-1 text-left">
                      <p className="font-bold text-midnight-900">Lobstr</p>
                      <p className="text-xs text-gray-500">Extension not detected</p>
                    </div>
                    <a
                      href="https://lobstr.co/uni/lobstr-signer-extension"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 text-xs font-bold text-amber-600 hover:underline"
                    >
                      Install <ExternalLink size={12} />
                    </a>
                  </div>
                ) : (
                  <button
                    onClick={handleLobstr}
                    disabled={anyConnecting}
                    className="group relative flex w-full items-center gap-4 rounded-2xl border-2 border-gray-100 p-4 hover:border-amber-300 hover:bg-amber-50/30 transition-all duration-300 disabled:opacity-60"
                  >
                    <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#0B1E3E]/10 group-hover:bg-[#0B1E3E] transition-colors flex-shrink-0">
                      {lobstrConnecting ? (
                        <Loader2 size={24} className="animate-spin text-amber-400" />
                      ) : (
                        <svg width="28" height="28" viewBox="0 0 512 512" fill="none" xmlns="http://www.w3.org/2000/svg">
                          <rect width="512" height="512" rx="100" fill="#0B1E3E"/>
                          <path d="M256 96C167.6 96 96 167.6 96 256s71.6 160 160 160 160-71.6 160-160S344.4 96 256 96zm0 280c-66.3 0-120-53.7-120-120s53.7-120 120-120 120 53.7 120 120-53.7 120-120 120z" fill="#FBBF24"/>
                          <circle cx="256" cy="256" r="50" fill="#FBBF24"/>
                        </svg>
                      )}
                    </div>
                    <div className="text-left">
                      <p className="font-bold text-midnight-900">Lobstr</p>
                      <p className="text-xs text-gray-500">
                        Popular Stellar Wallet &amp; Exchange
                      </p>
                    </div>
                    <ArrowRight
                      size={18}
                      className="absolute right-4 text-gray-300 group-hover:text-amber-500 group-hover:translate-x-1 transition-all"
                    />
                  </button>
                )}

                {/* -- Magic (email / passkey) -- */}
                <button
                  onClick={handleMagic}
                  disabled={anyConnecting}
                  className="group relative flex w-full items-center gap-4 rounded-2xl border-2 border-gray-100 p-4 hover:border-purple-300 hover:bg-purple-50/30 transition-all duration-300 disabled:opacity-60"
                >
                  <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-purple-100 text-purple-600 group-hover:bg-purple-500 group-hover:text-white transition-colors flex-shrink-0">
                    <Mail size={24} />
                  </div>
                  <div className="text-left">
                    <p className="font-bold text-midnight-900">Magic Wallet</p>
                    <p className="text-xs text-gray-500">Email or Passkey — no extension needed</p>
                  </div>
                  <ArrowRight
                    size={18}
                    className="absolute right-4 text-gray-300 group-hover:text-purple-500 group-hover:translate-x-1 transition-all"
                  />
                </button>

                {/* Security note */}
                <div className="relative py-1">
                  <div className="absolute inset-0 flex items-center" aria-hidden="true">
                    <div className="w-full border-t border-gray-100" />
                  </div>
                  <div className="relative flex justify-center text-xs uppercase tracking-widest text-gray-300">
                    <span className="bg-white px-2">Secure</span>
                  </div>
                </div>

                <div className="rounded-2xl bg-gray-50 p-4">
                  <div className="flex items-start gap-3">
                    <ShieldCheck size={18} className="text-mint-500 mt-0.5 flex-shrink-0" />
                    <p className="text-xs text-gray-600 leading-relaxed">
                      ELCARE-HUB never has access to your private keys and cannot
                      sign transactions without your explicit permission.
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* Error display */}
            {activeError && !isConnected && !anyConnecting && (
              <div className="mt-4 rounded-xl bg-terracotta-50 p-3 flex items-start gap-2 text-xs text-terracotta-700 animate-slide-up">
                <AlertTriangle size={14} className="flex-shrink-0 mt-0.5" />
                <p>{activeError}</p>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="bg-gray-50 p-4 text-center">
            <p className="text-[10px] text-gray-400 uppercase tracking-widest font-semibold flex items-center justify-center gap-2">
              Authenticated by Stellar Consensus <ShieldCheck size={10} />
            </p>
          </div>
        </div>
      </div>
    </>
  );
}
