// ─────────────────────────────────────────────────────────────
// components/Navbar.tsx — ELCARE-HUB Navigation (Redesigned)
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { useWalletContext } from "@/context/WalletContext";
import { Wallet, Store, LayoutDashboard, Menu, X, AlertTriangle, LogOut, ShieldCheck, Tag, Inbox, Compass, User, Gavel, Settings, HelpCircle, Rocket, ChevronDown } from "lucide-react";
import { ConnectWalletModal } from "./ConnectWalletModal";
import { WalletMenu } from "./WalletMenu";

export function Navbar() {
  const { publicKey, isConnected, isConnecting, disconnect, isWrongNetwork, status, balance, isLoadingBalance } =
    useWalletContext();
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [showWalletMenu, setShowWalletMenu] = useState(false);

  const shortKey = publicKey
    ? `${publicKey.slice(0, 4)}...${publicKey.slice(-4)}`
    : null;

  // Detect scroll for transparent → solid transition
  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 60);
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <>
      <nav
        className={`fixed top-0 left-0 right-0 z-50 transition-all duration-500 ${scrolled
          ? "bg-midnight-800/95 backdrop-blur-xl border-b border-brand-500/10 shadow-lg shadow-midnight-950/40"
          : "bg-transparent"
          }`}
      >
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 sm:px-6 py-4">
          {/* Logo */}
          <Link
            href="/"
            className="flex items-center gap-2.5 group"
          >
            {/* ElcareHub logo mark — medical cross + pulse line */}
            <span className="flex items-center justify-center w-10 h-10 rounded-xl bg-brand-500 shadow-lg shadow-brand-500/30 group-hover:shadow-brand-500/50 transition-all duration-300 group-hover:scale-105">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                {/* Cross arms */}
                <rect x="9" y="2" width="6" height="20" rx="2" fill="white"/>
                <rect x="2" y="9" width="20" height="6" rx="2" fill="white"/>
                {/* Pulse dot overlay */}
                <circle cx="12" cy="12" r="2.5" fill="#E27D60"/>
              </svg>
            </span>
            <span className="text-xl font-display font-bold text-white tracking-tight">
              Elcare<span className="text-brand-400">Hub</span>
            </span>
          </Link>

          {/* Desktop nav links */}
          <div className="hidden md:flex items-center gap-8 text-sm font-medium">
            <Link
              href="/"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <Store size={16} />
              Marketplace
            </Link>
            <Link
              href="/explore"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <Compass size={16} />
              Explore
            </Link>
            <Link
              href="/auctions"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <Gavel size={16} />
              Auctions
            </Link>
            <Link
              href="/launchpad"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <Rocket size={16} />
              Launchpad
            </Link>
            {isConnected && (
              <>
                <Link
                  href="/dashboard"
                  className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
                >
                  <LayoutDashboard size={16} />
                  Dashboard
                </Link>
                <Link
                  href="/profile"
                  className="flex items-center gap-1.5 text-white/70 hover:text-mint-400 transition-colors duration-300"
                >
                  <User size={16} />
                  My Profile
                </Link>
              </>
            )}
            {isConnected && (
              <Link
                href="/offers"
                className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
              >
                <Tag size={16} />
                My Offers
              </Link>
            )}
            {isConnected && (
              <Link
                href="/offers/incoming"
                className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
              >
                <Inbox size={16} />
                Offer Inbox
              </Link>
            )}
            <Link
              href="/settings"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <Settings size={16} />
              Settings
            </Link>
            <Link
              href="/help"
              className="flex items-center gap-1.5 text-white/70 hover:text-brand-400 transition-colors duration-300"
            >
              <HelpCircle size={16} />
              Help
            </Link>
          </div>


          {/* Desktop wallet button */}
          <div className="hidden md:flex items-center gap-4">
            {isConnected ? (
              <div className="flex items-center gap-3">
                {isWrongNetwork ? (
                  <button
                    onClick={() => setIsModalOpen(true)}
                    className="flex items-center gap-2 rounded-full bg-terracotta-500/20 border border-terracotta-500/30 px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-terracotta-400 hover:bg-terracotta-500/30 transition-all"
                  >
                    <AlertTriangle size={12} />
                    Wrong Network
                  </button>
                ) : (
                  <div className="flex items-center gap-2 rounded-full bg-mint-500/10 border border-mint-500/20 px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-mint-400">
                    <ShieldCheck size={12} />
                    Connected
                  </div>
                )}

                <div className="relative" data-testid="wallet-connected">
                  <button
                    onClick={() => setShowWalletMenu(!showWalletMenu)}
                    className="flex items-center gap-2 pl-3 pr-2 py-1.5 rounded-xl bg-white/5 border border-white/10 hover:bg-white/10 transition-colors cursor-pointer group"
                  >
                    <div className="flex flex-col items-end">
                      <span className="text-[10px] font-bold text-white/40 uppercase tracking-tighter">Connected Address</span>
                      <span className="text-xs font-mono text-white/90 leading-none">{shortKey}</span>
                    </div>
                    <div className="h-6 w-px bg-white/10 mx-1" />
                    <ChevronDown size={14} className={`text-white/40 transition-transform duration-300 ${showWalletMenu ? "rotate-180" : ""}`} />
                  </button>

                  {showWalletMenu && (
                    <div className="absolute top-full right-0 mt-3 w-64 animate-in fade-in slide-in-from-top-2 duration-300">
                      <WalletMenu
                        address={publicKey!}
                        balance={balance}
                        isLoadingBalance={isLoadingBalance}
                        onDisconnect={() => {
                          disconnect();
                          setShowWalletMenu(false);
                        }}
                      />
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <button
                onClick={() => setIsModalOpen(true)}
                disabled={isConnecting}
                className="flex items-center gap-2.5 rounded-xl bg-gradient-to-r from-brand-500 to-terracotta-500 px-6 py-2.5 text-sm font-bold text-white shadow-lg shadow-brand-500/25 hover:shadow-brand-500/40 hover:-translate-y-0.5 active:translate-y-0 transition-all duration-300"
              >
                <Wallet size={16} />
                {isConnecting ? "Connecting…" : "Connect Wallet"}
              </button>
            )}
          </div>

          {/* Mobile menu button */}
          <button
            onClick={() => setMobileOpen(!mobileOpen)}
            className="md:hidden flex items-center justify-center w-10 h-10 rounded-xl bg-white/5 text-white/70 hover:bg-white/10 border border-white/10 transition-all"
          >
            {mobileOpen ? <X size={20} /> : <Menu size={20} />}
          </button>
        </div>

        {/* Mobile drawer */}
        <div
          className={`md:hidden overflow-hidden transition-all duration-500 ${mobileOpen ? "max-h-96 opacity-100" : "max-h-0 opacity-0"
            }`}
        >
          <div className="bg-midnight-950/98 backdrop-blur-xl border-t border-white/5 px-4 py-8 space-y-6">
            <div className="grid grid-cols-1 gap-4">
              <Link
                href="/"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <Store size={20} className="text-brand-500" />
                Marketplace
              </Link>
              <Link
                href="/explore"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <Compass size={20} className="text-brand-500" />
                Explore
              </Link>
              <Link
                href="/auctions"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <Gavel size={20} className="text-brand-500" />
                Auctions
              </Link>
              <Link
                href="/launchpad"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <Rocket size={20} className="text-brand-500" />
                Launchpad
              </Link>
              {isConnected && (
                <>
                  <Link
                    href="/dashboard"
                    onClick={() => setMobileOpen(false)}
                    className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
                  >
                    <LayoutDashboard size={20} className="text-brand-500" />
                    Dashboard
                  </Link>
                  <Link
                    href="/profile"
                    onClick={() => setMobileOpen(false)}
                    className="flex items-center gap-3 text-white/80 hover:text-mint-400 transition-colors text-lg font-display"
                  >
                    <User size={20} className="text-mint-400" />
                    My Profile
                  </Link>
                </>
              )}
              {isConnected && (
                <Link
                  href="/offers"
                  onClick={() => setMobileOpen(false)}
                  className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
                >
                  <Tag size={20} className="text-brand-500" />
                  My Offers
                </Link>
              )}
              {isConnected && (
                <Link
                  href="/offers/incoming"
                  onClick={() => setMobileOpen(false)}
                  className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
                >
                  <Inbox size={20} className="text-brand-500" />
                  Offer Inbox
                </Link>
              )}
              <Link
                href="/settings"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <Settings size={20} className="text-gray-400" />
                Settings
              </Link>
              <Link
                href="/help"
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 text-white/80 hover:text-brand-400 transition-colors text-lg font-display"
              >
                <HelpCircle size={20} className="text-gray-400" />
                Help
              </Link>
            </div>

            <div className="pt-6 border-t border-white/5">
              {isConnected ? (
                <div className="space-y-4">
                  <WalletMenu
                    address={publicKey!}
                    balance={balance}
                    isLoadingBalance={isLoadingBalance}
                    onDisconnect={() => {
                      disconnect();
                      setMobileOpen(false);
                    }}
                  />
                </div>
              ) : (
                <button
                  onClick={() => {
                    setIsModalOpen(true);
                    setMobileOpen(false);
                  }}
                  disabled={isConnecting}
                  className="w-full flex items-center justify-center gap-2.5 rounded-xl bg-brand-500 py-4 text-base font-bold text-white shadow-xl shadow-brand-500/20"
                >
                  <Wallet size={20} />
                  {isConnecting ? "Connecting…" : "Connect Wallet"}
                </button>
              )}
            </div>
          </div>
        </div>
      </nav>

      <ConnectWalletModal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)} />
    </>
  );
}

