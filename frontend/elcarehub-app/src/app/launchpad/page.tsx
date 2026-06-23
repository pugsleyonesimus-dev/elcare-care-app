"use client";

import { useState } from "react";
import Link from "next/link";
import { useLaunchpadCollections } from "@/hooks/useLaunchpad";
import { useWalletContext } from "@/context/WalletContext";
import { useLaunchpadAdminCheck } from "@/hooks/useLaunchpadAdmin";
import { Loader2, Rocket, Palette, Zap, ExternalLink, TrendingUp, Users, Star, Shield } from "lucide-react";

export default function LaunchpadPage() {
  const { collections, isLoading, error } = useLaunchpadCollections();
  const { publicKey, isConnected } = useWalletContext();
  const { isAdmin } = useLaunchpadAdminCheck(publicKey);
  const [featuredCollections, setFeaturedCollections] = useState<any[]>([]);

  // Get some featured collections (first 3 for now)
  const featured = collections.slice(0, 3);

  return (
    <main className="min-h-screen bg-brand-50/20">
      <div className="pt-24 pb-12">
        {/* Hero Section */}
        <section className="relative overflow-hidden bg-gradient-to-br from-brand-500 via-terracotta-500 to-mint-500 text-white">
          <div className="absolute inset-0 bg-black/10"></div>
          <div className="relative mx-auto max-w-7xl px-4 py-20 sm:px-6 lg:px-8">
            <div className="text-center">
              <div className="mb-6 flex justify-center">
                <div className="flex items-center justify-center w-20 h-20 rounded-2xl bg-white/10 backdrop-blur-sm">
                  <Rocket size={40} className="text-white" />
                </div>
              </div>
              <h1 className="text-6xl font-display font-black mb-6">
                ELCARE-HUB Launchpad
              </h1>
              <p className="text-xl text-white/90 max-w-3xl mx-auto mb-8 font-inter leading-relaxed">
                Launch your NFT collections on Stellar with our powerful factory contract.
                Deploy ERC-721 and ERC-1155 collections with built-in royalties, lazy minting,
                and seamless marketplace integration.
              </p>
              <div className="flex flex-col sm:flex-row gap-4 justify-center">
                <Link
                  href="/launchpad/create"
                  className="inline-flex items-center gap-3 rounded-2xl bg-white px-8 py-4 text-lg font-bold text-brand-600 shadow-xl shadow-black/20 hover:shadow-2xl hover:-translate-y-1 transition-all hover:scale-[1.02] active:scale-[0.98]"
                >
                  <Palette size={20} />
                  Create Collection
                </Link>
                <Link
                  href="/launchpad/collections"
                  className="inline-flex items-center gap-3 rounded-2xl bg-white/10 backdrop-blur-sm border border-white/20 px-8 py-4 text-lg font-bold text-white hover:bg-white/20 transition-all"
                >
                  <ExternalLink size={20} />
                  Browse Collections
                </Link>
              </div>
            </div>
          </div>
        </section>

        {/* Features Section */}
        <section className="py-20">
          <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
            <div className="text-center mb-16">
              <span className="inline-block px-4 py-1.5 rounded-full bg-brand-100 text-brand-600 text-sm font-bold uppercase tracking-widest mb-4">
                Features
              </span>
              <h2 className="text-4xl font-display font-black text-gray-900 mb-4">
                Everything You Need to Launch
              </h2>
              <p className="text-gray-500 max-w-2xl mx-auto font-inter text-lg">
                Our launchpad provides all the tools and infrastructure for successful NFT collection deployment.
              </p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
              <div className="bg-white rounded-3xl p-8 border border-gray-100 shadow-sm hover:shadow-xl transition-all hover:-translate-y-2">
                <div className="w-12 h-12 rounded-2xl bg-brand-100 flex items-center justify-center mb-6">
                  <Palette size={24} className="text-brand-600" />
                </div>
                <h3 className="text-xl font-display font-bold text-gray-900 mb-3">
                  Multiple Standards
                </h3>
                <p className="text-gray-500 font-inter">
                  Deploy ERC-721 and ERC-1155 collections with normal or lazy minting capabilities.
                </p>
              </div>

              <div className="bg-white rounded-3xl p-8 border border-gray-100 shadow-sm hover:shadow-xl transition-all hover:-translate-y-2">
                <div className="w-12 h-12 rounded-2xl bg-mint-100 flex items-center justify-center mb-6">
                  <Zap size={24} className="text-mint-600" />
                </div>
                <h3 className="text-xl font-display font-bold text-gray-900 mb-3">
                  Gas Efficient
                </h3>
                <p className="text-gray-500 font-inter">
                  Shared WASM bytecode reduces deployment costs and network storage requirements.
                </p>
              </div>

              <div className="bg-white rounded-3xl p-8 border border-gray-100 shadow-sm hover:shadow-xl transition-all hover:-translate-y-2">
                <div className="w-12 h-12 rounded-2xl bg-terracotta-100 flex items-center justify-center mb-6">
                  <TrendingUp size={24} className="text-terracotta-600" />
                </div>
                <h3 className="text-xl font-display font-bold text-gray-900 mb-3">
                  Royalty Support
                </h3>
                <p className="text-gray-500 font-inter">
                  Built-in royalty enforcement ensures creators earn from secondary sales.
                </p>
              </div>
            </div>
          </div>
        </section>

        {/* Stats Section */}
        <section className="py-20 bg-white">
          <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
              <div className="text-center">
                <div className="text-4xl font-display font-black text-brand-600 mb-2">
                  {collections.length}
                </div>
                <div className="text-gray-500 font-inter font-medium">
                  Collections Deployed
                </div>
              </div>
              <div className="text-center">
                <div className="text-4xl font-display font-black text-mint-600 mb-2">
                  {new Set(collections.map(c => c.creator)).size}
                </div>
                <div className="text-gray-500 font-inter font-medium">
                  Active Creators
                </div>
              </div>
              <div className="text-center">
                <div className="text-4xl font-display font-black text-terracotta-600 mb-2">
                  4
                </div>
                <div className="text-gray-500 font-inter font-medium">
                  Collection Types
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* Featured Collections */}
        {featured.length > 0 && (
          <section className="py-20">
            <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
              <div className="flex flex-col md:flex-row md:items-end justify-between gap-6 mb-12">
                <div>
                  <span className="inline-block px-4 py-1.5 rounded-full bg-brand-100 text-brand-600 text-sm font-bold uppercase tracking-widest mb-4">
                    Featured
                  </span>
                  <h2 className="text-4xl font-display font-black text-gray-900 mb-4">
                    Latest Collections
                  </h2>
                  <p className="text-gray-500 max-w-xl font-inter text-lg">
                    Discover the newest collections launched on our platform.
                  </p>
                </div>
                <Link
                  href="/launchpad/collections"
                  className="flex items-center gap-2 rounded-2xl bg-gray-100 px-6 py-3 text-sm font-bold text-gray-900 hover:bg-brand-500 hover:text-white transition-all"
                >
                  View All
                  <ExternalLink size={16} />
                </Link>
              </div>

              {isLoading ? (
                <div className="flex flex-col items-center justify-center py-20 gap-4">
                  <Loader2 size={48} className="animate-spin text-brand-500" />
                  <p className="text-gray-500 font-medium font-inter">Loading collections...</p>
                </div>
              ) : error ? (
                <div className="rounded-3xl bg-red-50 p-12 text-center border border-red-100">
                  <p className="text-red-600 font-bold mb-4">{error}</p>
                </div>
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
                  {featured.map((c) => (
                    <div
                      key={c.address}
                      className="group bg-white rounded-3xl border border-gray-100 p-6 shadow-sm hover:shadow-xl hover:shadow-brand-900/5 transition-all hover:-translate-y-1"
                    >
                      <div className="flex justify-between items-start mb-4">
                        <span className={`px-3 py-1 rounded-full text-xs font-bold tracking-wider uppercase ${
                          c.kind.startsWith('Lazy') ? 'bg-amber-100 text-amber-700' : 'bg-brand-100 text-brand-700'
                        }`}>
                          {c.kind}
                        </span>
                      </div>
                      <h3 className="text-xl font-display font-bold text-gray-900 mb-2 truncate" title={c.address}>
                        {c.address.slice(0, 8)}...{c.address.slice(-8)}
                      </h3>
                      <div className="space-y-2 mb-4">
                        <div className="flex justify-between text-sm">
                          <span className="text-gray-400 font-inter">Creator</span>
                          <span className="text-gray-700 font-mono font-medium truncate ml-4 w-32 text-right">
                            {c.creator.slice(0, 4)}...{c.creator.slice(-4)}
                          </span>
                        </div>
                      </div>
                      <Link
                        href={`/launchpad/collections/${c.address}`}
                        className="mt-4 block w-full text-center py-3 rounded-2xl bg-gray-50 text-gray-900 font-bold hover:bg-brand-500 hover:text-white transition-all"
                      >
                        View Details
                      </Link>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </section>
        )}

        {/* CTA Section */}
        <section className="py-20 bg-gradient-to-r from-brand-500 to-terracotta-500 text-white">
          <div className="mx-auto max-w-4xl px-4 sm:px-6 lg:px-8 text-center">
            <h2 className="text-4xl font-display font-black mb-6">
              Ready to Launch Your Collection?
            </h2>
            <p className="text-xl text-white/90 mb-8 font-inter">
              Join hundreds of creators who have successfully launched their NFT collections on ELCARE-HUB.
            </p>
            <div className="flex flex-col sm:flex-row gap-4 justify-center">
              <Link
                href="/launchpad/create"
                className="inline-flex items-center gap-3 rounded-2xl bg-white px-8 py-4 text-lg font-bold text-brand-600 shadow-xl shadow-black/20 hover:shadow-2xl hover:-translate-y-1 transition-all hover:scale-[1.02] active:scale-[0.98]"
              >
                <Rocket size={20} />
                Create Collection
              </Link>
              {isConnected && (
                <Link
                  href="/launchpad/my-collections"
                  className="inline-flex items-center gap-3 rounded-2xl bg-white/10 backdrop-blur-sm border border-white/20 px-8 py-4 text-lg font-bold text-white hover:bg-white/20 transition-all"
                >
                  <Palette size={20} />
                  My Collections
                </Link>
              )}
              {isAdmin && (
                <Link
                  href="/launchpad/admin"
                  className="inline-flex items-center gap-3 rounded-2xl bg-white/10 backdrop-blur-sm border border-white/20 px-8 py-4 text-lg font-bold text-white hover:bg-white/20 transition-all"
                >
                  <Shield size={20} />
                  Admin Dashboard
                </Link>
              )}
            </div>
          </div>
        </section>
      </div>
    </main>
  );
}