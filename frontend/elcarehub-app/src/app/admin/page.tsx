// ─────────────────────────────────────────────────────────────
// app/admin/page.tsx — Administrative Dashboard & Moderation Panel
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useCallback, useEffect } from "react";
import { useWallet } from "@/hooks/useWallet";
import { useAdminStats, useModeration, useTokenManagement, useAdminCheck } from "@/hooks/useAdmin";
import { useAdminSession } from "@/hooks/useAdminSession";
import { AdminConfirmationModal } from "@/components/AdminConfirmationModal";
import {
    Users,
    Palette,
    ShieldAlert,
    ShieldCheck,
    Search,
    Plus,
    Trash2,
    BarChart3,
    Wallet,
    Settings,
    Lock,
    Loader2,
    CheckCircle2,
    AlertCircle,
    KeyRound,
    History
} from "lucide-react";
import { stroopsToXlm } from "@/lib/contract";

export default function AdminPage() {
    const { publicKey } = useWallet();
    const { isAdmin, isLoading: isCheckingAdmin } = useAdminCheck(publicKey);
    const { stats, isLoading: isLoadingStats, refresh: refreshStats } = useAdminStats();
    const { revoke, reinstate, checkStatus, isProcessing: isModerating } = useModeration(publicKey);
    const {
        whitelistedTokens,
        whitelist,
        unwhitelist,
        isLoading: isLoadingTokens,
        isProcessing: isManagingTokens,
        error: tokenError,
        refresh: refreshTokens
    } = useTokenManagement(publicKey);

    const { isAuthenticated, authenticate, logout, sessionExpiresIn } = useAdminSession();

    // Local state for moderation search
    const [artistSearch, setArtistSearch] = useState("");
    const [searchResult, setSearchResult] = useState<{ address: string; isRevoked: boolean } | null>(null);
    const [searchError, setSearchError] = useState<string | null>(null);

    // Local state for token management
    const [newTokenAddress, setNewTokenAddress] = useState("");

    // Confirmation Modal state
    const [confirmConfig, setConfirmConfig] = useState<{
        isOpen: boolean;
        title: string;
        actionDescription: string;
        consequences: string[];
        onConfirm: () => void;
        variant: "danger" | "warning" | "info";
    }>({
        isOpen: false,
        title: "",
        actionDescription: "",
        consequences: [],
        onConfirm: () => { },
        variant: "danger"
    });

    const handleSearchArtist = async () => {
        if (!artistSearch) return;
        setSearchError(null);
        try {
            const isRevoked = await checkStatus(artistSearch);
            setSearchResult({ address: artistSearch, isRevoked });
        } catch {
            setSearchError("Invalid address or error fetching status.");
        }
    };

    const handleToggleArtistStatus = async () => {
        if (!searchResult) return;

        const action = searchResult.isRevoked ? "reinstate" : "revoke";
        
        setConfirmConfig({
            isOpen: true,
            title: searchResult.isRevoked ? "Reinstate Artist" : "Revoke Artist",
            actionDescription: `${searchResult.isRevoked ? "Restoring" : "Removing"} permissions for artist ${searchResult.address}.`,
            consequences: searchResult.isRevoked 
                ? ["Artist will be able to create new listings and auctions again.", "Their existing profile will be visible to all users."]
                : ["Artist will no longer be able to create new listings or auctions.", "This action will be recorded on the blockchain.", "Existing listings may need to be manually managed."],
            variant: searchResult.isRevoked ? "info" : "danger",
            onConfirm: async () => {
                const success = searchResult.isRevoked
                    ? await reinstate(searchResult.address)
                    : await revoke(searchResult.address);

                if (success) {
                    setSearchResult({ ...searchResult, isRevoked: !searchResult.isRevoked });
                    setConfirmConfig(prev => ({ ...prev, isOpen: false }));
                }
            }
        });
    };

    const handleWhitelistToken = async () => {
        if (!newTokenAddress) return;

        setConfirmConfig({
            isOpen: true,
            title: "Whitelist Token",
            actionDescription: `Adding token ${newTokenAddress} to the whitelisted payment options.`,
            consequences: [
                "Users will be able to list and buy NFTs using this token.",
                "The marketplace contract will interact with this token contract.",
                "Ensure the token address is correct and the token is trusted."
            ],
            variant: "info",
            onConfirm: async () => {
                const success = await whitelist(newTokenAddress);
                if (success) {
                    setNewTokenAddress("");
                    setConfirmConfig(prev => ({ ...prev, isOpen: false }));
                }
            }
        });
    };

    const handleRemoveToken = async (addr: string) => {
        setConfirmConfig({
            isOpen: true,
            title: "Remove Token from Whitelist",
            actionDescription: `Removing token ${addr} from whitelisted payment options.`,
            consequences: [
                "Users will no longer be able to create new listings using this token.",
                "Existing listings using this token may become un-purchasable.",
                "This action is immediate and affects all users."
            ],
            variant: "danger",
            onConfirm: async () => {
                const success = await unwhitelist(addr);
                if (success) {
                    setConfirmConfig(prev => ({ ...prev, isOpen: false }));
                }
            }
        });
    };

    if (isCheckingAdmin) {
        return (
            <div className="flex h-[80vh] items-center justify-center">
                <Loader2 className="h-10 w-10 animate-spin text-brand-500" />
            </div>
        );
    }

    if (!isAdmin) {
        return (
            <div className="flex h-[80vh] flex-col items-center justify-center px-4 text-center">
                <div className="mb-6 rounded-full bg-red-100 p-6">
                    <Lock className="h-12 w-12 text-red-600" />
                </div>
                <h1 className="font-display text-4xl font-bold tracking-tight text-midnight-900 sm:text-5xl">
                    Access Denied
                </h1>
                <p className="mt-4 max-w-lg text-lg text-gray-600">
                    This page is reserved for marketplace administrators.
                    Please connect the administrator wallet to view this panel.
                </p>
            </div>
        );
    }

    if (!isAuthenticated) {
        return (
            <div className="flex h-[80vh] flex-col items-center justify-center px-4 text-center">
                <div className="mb-6 rounded-full bg-brand-100 p-6">
                    <ShieldCheck className="h-12 w-12 text-brand-600" />
                </div>
                <h1 className="font-display text-4xl font-bold tracking-tight text-midnight-900 sm:text-5xl">
                    Admin Session Required
                </h1>
                <p className="mt-4 max-w-lg text-lg text-gray-600 mb-8">
                    To perform sensitive administrative actions, you must start a secure session.
                    This session will automatically expire after 15 minutes of inactivity.
                </p>
                <button
                    onClick={authenticate}
                    className="flex items-center gap-2 rounded-2xl bg-brand-600 px-8 py-4 text-lg font-bold text-white shadow-lg shadow-brand-200 transition-all hover:bg-brand-700 active:scale-95"
                >
                    <KeyRound className="h-6 w-6" />
                    Start Admin Session
                </button>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-brand-50 pb-20 pt-10">
            <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
                {/* Header */}
                <div className="mb-10 flex flex-col justify-between gap-6 sm:flex-row sm:items-end">
                    <div>
                        <div className="flex items-center gap-2 mb-2">
                            <span className="inline-flex items-center rounded-full bg-brand-100 px-3 py-1 text-xs font-semibold uppercase tracking-wider text-brand-700">
                                Admin Control Center
                            </span>
                            <span className="inline-flex items-center gap-1 rounded-full bg-green-100 px-3 py-1 text-xs font-semibold text-green-700">
                                <History className="h-3 w-3" />
                                Session: {Math.floor(sessionExpiresIn / 60000)}m remaining
                            </span>
                        </div>
                        <h1 className="mt-3 font-display text-4xl font-bold text-midnight-950 sm:text-5xl">
                            Marketplace <span className="text-brand-500">Overview</span>
                        </h1>
                    </div>
                    <div className="flex gap-3">
                        <button
                            type="button"
                            onClick={logout}
                            className="flex items-center gap-2 rounded-full bg-white px-5 py-2.5 text-sm font-semibold text-red-600 shadow-sm transition-all hover:bg-red-50 border border-red-100"
                        >
                            End Session
                        </button>
                        <button
                            type="button"
                            onClick={() => { refreshStats(); refreshTokens(); }}
                            className="flex items-center gap-2 rounded-full bg-white px-5 py-2.5 text-sm font-semibold text-midnight-900 shadow-sm transition-all hover:bg-brand-50 hover:shadow-md border border-brand-100"
                        >
                            <Loader2 className={`h-4 w-4 ${isLoadingStats || isLoadingTokens ? 'animate-spin' : ''}`} />
                            Refresh Data
                        </button>
                    </div>
                </div>

                <div className="mb-12 grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
                    <StatCard
                        title="Total Listed NFTs"
                        value={stats?.totalListings?.toString() || "0"}
                        icon={<Palette className="h-6 w-6" />}
                        color="bg-primary shadow-primary/20"
                    />
                    <StatCard
                        title="Active Artists"
                        value={stats?.totalUsers?.toString() || "0"}
                        icon={<Users className="h-6 w-6" />}
                        color="bg-secondary shadow-secondary/20"
                    />
                    <StatCard
                        title="Platform Fee"
                        value={`${stats?.protocolFeeBps || 0} BPS`}
                        icon={<BarChart3 className="h-6 w-6" />}
                        color="bg-primary-dark shadow-primary-dark/20"
                    />
                    <StatCard
                        title="Treasury Status"
                        value={stats?.treasuryAddress ? "Active" : "Not Set"}
                        icon={<ShieldCheck className="h-6 w-6" />}
                        color="bg-midnight-600 shadow-midnight/20"
                    />
                </div>

                <div className="grid gap-8 lg:grid-cols-2">
                    {/* Moderation Panel */}
                    <section className="rounded-3xl bg-white p-8 shadow-sm border border-brand-100">
                        <div className="mb-6 flex items-center gap-3">
                            <div className="rounded-xl bg-orange-100 p-2.5">
                                <ShieldAlert className="h-6 w-6 text-orange-600" />
                            </div>
                            <h2 className="font-display text-2xl font-bold text-midnight-950">Artist Moderation</h2>
                        </div>

                        <p className="mb-6 text-gray-600">
                            Restrict or reinstate artist permissions. Revoked artists can browse and buy, but cannot create new listings or auctions.
                        </p>

                        <div className="flex gap-2">
                            <div className="relative flex-1">
                                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
                                <input
                                    type="text"
                                    placeholder="Artist Stellar Address (G...)"
                                    value={artistSearch}
                                    onChange={(e) => setArtistSearch(e.target.value)}
                                    className="w-full rounded-xl border border-gray-200 py-3 pl-10 pr-4 text-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500"
                                />
                            </div>
                            <button
                                type="button"
                                onClick={handleSearchArtist}
                                className="rounded-xl bg-midnight-900 px-6 py-3 text-sm font-bold text-white transition-all hover:bg-midnight-800"
                            >
                                Inspect
                            </button>
                        </div>

                        {searchError && (
                            <div className="mt-4 flex items-center gap-2 rounded-xl bg-red-50 p-4 text-sm text-red-600 border border-red-100">
                                <AlertCircle className="h-4 w-4" />
                                {searchError}
                            </div>
                        )}

                        {searchResult && (
                            <div className="mt-8 rounded-2xl border border-gray-100 bg-gray-50/50 p-6 animate-fade-in-up">
                                <div className="mb-4 flex flex-col gap-1">
                                    <span className="text-xs font-bold uppercase tracking-wider text-gray-400">Inspecting Address</span>
                                    <code className="break-all font-mono text-sm font-medium text-midnight-900">
                                        {searchResult.address}
                                    </code>
                                </div>

                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3">
                                        {searchResult.isRevoked ? (
                                            <>
                                                <div className="h-3 w-3 rounded-full bg-red-500 animate-pulse" />
                                                <span className="font-semibold text-red-600 uppercase tracking-tighter text-xs">Revoked / Suspended</span>
                                            </>
                                        ) : (
                                            <>
                                                <div className="h-3 w-3 rounded-full bg-secondary" />
                                                <span className="font-semibold text-secondary-dark uppercase tracking-tighter text-xs">Active / Verified</span>
                                            </>
                                        )}
                                    </div>

                                    <button
                                        type="button"
                                        disabled={isModerating}
                                        onClick={handleToggleArtistStatus}
                                        className={`flex items-center gap-2 rounded-full px-6 py-2.5 text-sm font-bold transition-all ${searchResult.isRevoked
                                            ? 'bg-secondary-light/20 text-secondary-dark hover:bg-secondary-light/40'
                                            : 'bg-red-50 text-red-700 hover:bg-red-100'
                                            } disabled:opacity-50`}
                                    >
                                        {isModerating ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                                        {searchResult.isRevoked ? 'Reinstate Permission' : 'Revoke Permission'}
                                    </button>
                                </div>
                            </div>
                        )}
                    </section>

                    {/* Treasury Balances */}
                    <section className="rounded-3xl bg-white p-8 shadow-sm border border-brand-100">
                        <div className="mb-6 flex items-center gap-3">
                            <div className="rounded-xl bg-purple-100 p-2.5">
                                <Wallet className="h-6 w-6 text-purple-600" />
                            </div>
                            <h2 className="font-display text-2xl font-bold text-midnight-950">Treasury Balances</h2>
                        </div>

                        <div className="mb-6 rounded-2xl bg-midnight-900 p-6 text-white overflow-hidden relative">
                            <BarChart3 className="absolute -bottom-6 -right-6 h-32 w-32 opacity-10 rotate-12" />
                            <div className="relative z-10">
                                <p className="text-sm font-medium text-white/60 mb-1">Treasury Address</p>
                                <code className="break-all font-mono text-xs opacity-80 block mb-4">
                                    {stats?.treasuryAddress || "Not configured"}
                                </code>

                                <div className="flex items-end gap-2">
                                    <p className="text-3xl font-bold tracking-tight">
                                        {stats?.treasuryBalances.find(b => b.asset_type === 'native')?.balance || "0.00"}
                                    </p>
                                    <p className="mb-1 text-sm font-medium text-white/60 uppercase">XLM</p>
                                </div>
                            </div>
                        </div>

                        <div className="space-y-4">
                            <h3 className="text-xs font-bold uppercase tracking-widest text-gray-400">Other Assets</h3>
                            {stats?.treasuryBalances && stats.treasuryBalances.length > 1 ? (
                                <div className="divide-y divide-gray-100">
                                    {stats.treasuryBalances.filter(b => b.asset_type !== 'native').map((balance: any, idx) => (
                                        <div key={idx} className="flex items-center justify-between py-3">
                                            <div className="flex flex-col">
                                                <span className="font-bold text-midnight-900">{balance.asset_code}</span>
                                                <span className="text-[10px] text-gray-400 font-mono truncate max-w-[150px]">{balance.asset_issuer}</span>
                                            </div>
                                            <span className="font-medium text-midnight-700">{balance.balance}</span>
                                        </div>
                                    ))}
                                </div>
                            ) : (
                                <div className="flex flex-col items-center justify-center py-6 text-center">
                                    <AlertCircle className="mb-2 h-8 w-8 text-gray-200" />
                                    <p className="text-sm text-gray-400 italic">No custom assets found in treasury</p>
                                </div>
                            )}
                        </div>
                    </section>

                    {/* Token Whitelist Panel */}
                    <section className="lg:col-span-2 rounded-3xl bg-white p-8 shadow-sm border border-brand-100">
                        <div className="mb-8 flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
                            <div className="flex items-center gap-3">
                                <div className="rounded-xl bg-mint-100 p-2.5">
                                    <Settings className="h-6 w-6 text-mint-600" />
                                </div>
                                <h2 className="font-display text-2xl font-bold text-midnight-950">Whitelisted Payment Tokens</h2>
                            </div>

                            <div className="flex max-w-md gap-2">
                                <input
                                    type="text"
                                    placeholder="Token Contract ID (C...)"
                                    value={newTokenAddress}
                                    onChange={(e) => setNewTokenAddress(e.target.value)}
                                    className="flex-1 rounded-xl border border-gray-200 px-4 py-2.5 text-sm focus:border-mint-500 focus:outline-none focus:ring-1 focus:ring-mint-500"
                                />
                                <button
                                    type="button"
                                    disabled={isManagingTokens || !newTokenAddress}
                                    onClick={handleWhitelistToken}
                                    className="flex items-center gap-2 rounded-xl bg-secondary-dark px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-secondary disabled:opacity-50 shadow-md shadow-secondary/10"
                                >
                                    {isManagingTokens ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
                                    Add
                                </button>
                            </div>
                        </div>

                        {tokenError && (
                            <div className="mb-6 flex items-center gap-2 rounded-xl bg-red-50 p-4 text-sm text-red-600 border border-red-100">
                                <AlertCircle className="h-4 w-4" />
                                {tokenError}
                            </div>
                        )}

                        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            <div className="rounded-2xl border-2 border-gray-100 bg-gray-50/20 p-6">
                                <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-full bg-brand-100 text-brand-600 font-bold">
                                    X
                                </div>
                                <h4 className="font-bold text-midnight-950">Native Stellar (XLM)</h4>
                                <p className="mt-1 text-xs text-gray-500">Built-in default currency</p>
                                <div className="mt-4 flex items-center gap-2 rounded-full bg-secondary-light/30 px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-secondary-dark w-fit">
                                    <CheckCircle2 className="h-3 w-3" />
                                    Default Enabled
                                </div>
                            </div>

                            {whitelistedTokens.map((token) => (
                                <div key={token} className="group relative rounded-2xl border border-gray-100 bg-white p-6 shadow-sm hover:border-brand-200 transition-all">
                                    <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-full bg-mint-50 text-mint-600 font-bold">
                                        T
                                    </div>
                                    <h4 className="font-bold text-midnight-950 truncate" title={token}>
                                        {token.slice(0, 8)}...{token.slice(-8)}
                                    </h4>
                                    <p className="mt-1 text-xs text-gray-500 font-mono">{token.slice(0, 16)}...</p>
                                    
                                    <button
                                        type="button"
                                        aria-label="Remove token"
                                        title="Remove token"
                                        onClick={() => handleRemoveToken(token)}
                                        className="absolute right-4 top-4 rounded-lg p-2 text-gray-300 hover:bg-red-50 hover:text-red-500 transition-all opacity-0 group-hover:opacity-100"
                                    >
                                        <Trash2 size={18} />
                                    </button>
                                </div>
                            ))}

                            {isLoadingTokens && (
                                <div className="flex h-32 flex-col items-center justify-center rounded-2xl border-2 border-dashed border-gray-50 bg-gray-50/10 p-6 text-center text-gray-400">
                                    <Loader2 className="h-8 w-8 animate-spin opacity-20 mb-2" />
                                    <p className="text-xs">Loading tokens…</p>
                                </div>
                            )}

                            {!isLoadingTokens && whitelistedTokens.length === 0 && (
                                <div className="flex h-32 flex-col items-center justify-center rounded-2xl border-2 border-dashed border-gray-100 p-6 text-center text-gray-400">
                                    <AlertCircle className="mb-2 h-8 w-8 opacity-20" />
                                    <p className="text-xs italic">No additional SRC-20 tokens whitelisted.</p>
                                </div>
                            )}
                        </div>
                    </section>
                </div>
            </div >

            <AdminConfirmationModal
                isOpen={confirmConfig.isOpen}
                onClose={() => setConfirmConfig(prev => ({ ...prev, isOpen: false }))}
                onConfirm={confirmConfig.onConfirm}
                title={confirmConfig.title}
                actionDescription={confirmConfig.actionDescription}
                consequences={confirmConfig.consequences}
                variant={confirmConfig.variant}
                isProcessing={isModerating || isManagingTokens}
            />
        </div >
    );
}

function StatCard({ title, value, icon, color }: { title: string; value: string; icon: React.ReactNode; color: string }) {
    return (
        <div className="group rounded-3xl bg-white p-6 shadow-sm transition-all hover:shadow-md border border-brand-100">
            <div className={`mb-4 inline-flex items-center justify-center rounded-2xl p-3 text-white ${color} shadow-lg transition-transform group-hover:scale-110`}>
                {icon}
            </div>
            <p className="text-sm font-medium text-gray-500">{title}</p>
            <p className="mt-1 font-display text-3xl font-bold text-midnight-950">{value}</p>
        </div>
    );
}
