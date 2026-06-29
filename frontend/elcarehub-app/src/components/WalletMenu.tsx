"use client";

import { useState } from "react";
import { Copy, Check, LogOut, Wallet, Loader2 } from "lucide-react";

interface WalletMenuProps {
  address: string;
  balance: string | null;
  isLoadingBalance: boolean;
  onDisconnect: () => void;
  className?: string;
}

export function WalletMenu({
  address,
  balance,
  isLoadingBalance,
  onDisconnect,
  className = "",
}: WalletMenuProps) {
  const [copied, setCopied] = useState(false);

  const truncatedAddress = `${address.slice(0, 4)}...${address.slice(-4)}`;

  const handleCopy = () => {
    navigator.clipboard.writeText(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={`flex flex-col gap-2 p-4 bg-midnight-900/50 backdrop-blur-md border border-white/10 rounded-2xl shadow-xl ${className}`}>
      {/* Address & Copy */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-2">
          <div className="p-2 rounded-lg bg-brand-500/20 text-brand-400">
            <Wallet size={16} />
          </div>
          <div>
            <p className="text-[10px] uppercase tracking-widest text-white/40 font-bold">Wallet Address</p>
            <p className="text-sm font-mono text-white/90">{truncatedAddress}</p>
          </div>
        </div>
        <button
          onClick={handleCopy}
          className="p-2 rounded-lg bg-white/5 text-white/60 hover:bg-white/10 hover:text-white transition-all"
          title="Copy Address"
        >
          {copied ? <Check size={16} className="text-mint-400" /> : <Copy size={16} />}
        </button>
      </div>

      {/* Balance */}
      <div className="mt-2 p-3 rounded-xl bg-white/5 border border-white/5">
        <p className="text-[10px] uppercase tracking-widest text-white/40 font-bold mb-1">Available Balance</p>
        <div className="flex items-baseline gap-1.5">
          {isLoadingBalance ? (
            <div className="flex items-center gap-2 text-white/60">
              <Loader2 size={14} className="animate-spin" />
              <span className="text-sm font-medium italic">Fetching...</span>
            </div>
          ) : (
            <>
              <span className="text-xl font-display font-bold text-white">
                {balance ? parseFloat(balance).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 7 }) : "0.00"}
              </span>
              <span className="text-xs font-bold text-brand-400">XLM</span>
            </>
          )}
        </div>
      </div>

      {/* Actions */}
      <button
        onClick={onDisconnect}
        className="mt-2 flex w-full items-center justify-center gap-2 rounded-xl bg-terracotta-500/10 py-2.5 text-sm font-bold text-terracotta-400 hover:bg-terracotta-500/20 border border-terracotta-500/20 transition-all"
      >
        <LogOut size={16} />
        Disconnect Wallet
      </button>
    </div>
  );
}
