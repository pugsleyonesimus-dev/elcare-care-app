// ─────────────────────────────────────────────────────────────
// app/layout.tsx — Root layout
// ─────────────────────────────────────────────────────────────

import type { Metadata } from "next";
import "./globals.css";
import { WalletProvider } from "@/context/WalletContext";
import { Navbar } from "@/components/Navbar";
import { RootErrorBoundary } from "@/components/RootErrorBoundary";
import { ToastProvider } from "@/components/ToastProvider";
import { CSPostHogProvider } from "@/providers/PostHogProvider";
import { E2eMockChainInit } from "@/components/E2eMockChainInit";

export const metadata: Metadata = {
  title: "ELCARE-HUB — African Art on Stellar",
  description:
    "Decentralized marketplace for African art. Buy and sell unique artworks using Stellar blockchain.",
  openGraph: {
    title: "ELCARE-HUB",
    description: "Decentralized marketplace for African art on Stellar",
    type: "website",
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-brand-50 text-gray-900">
        <WalletProvider>
          <CSPostHogProvider>
            <ToastProvider>
              <RootErrorBoundary>
                <E2eMockChainInit />
                <Navbar />
                <main className="w-full">{children}</main>
                <footer className="bg-midnight-950 border-t border-white/5 py-10 text-center text-sm text-white/30">
                  <div className="mx-auto max-w-7xl px-4 sm:px-6">
                    <p className="font-display text-lg font-bold text-white/50 mb-3 flex items-center justify-center gap-2">
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <rect x="9" y="2" width="6" height="20" rx="2" fill="currentColor" opacity="0.6"/>
                        <rect x="2" y="9" width="20" height="6" rx="2" fill="currentColor" opacity="0.6"/>
                        <circle cx="12" cy="12" r="2.5" fill="#E27D60" opacity="0.8"/>
                      </svg>
                      Elcare<span className="text-brand-400/60">Hub</span>
                    </p>
                    <p>
                      © {new Date().getFullYear()} ELCARE-HUB · Built on{" "}
                      <a
                        href="https://stellar.org"
                        className="text-brand-400/70 hover:text-brand-400 hover:underline transition-colors"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        Stellar
                      </a>
                      {" "}·{" "}
                      <a
                        href="https://freighter.app"
                        className="text-brand-400/70 hover:text-brand-400 hover:underline transition-colors"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        Freighter Wallet
                      </a>
                      {" "}·{" "}
                      <a
                        href="/settings"
                        className="text-brand-400/70 hover:text-brand-400 hover:underline transition-colors"
                      >
                        Settings
                      </a>
                      {" "}·{" "}
                      <a
                        href="/help"
                        className="text-brand-400/70 hover:text-brand-400 hover:underline transition-colors"
                      >
                        Help
                      </a>
                    </p>
                    <p className="mt-3 text-xs text-white/15">
                      Celebrating African art and heritage through blockchain technology.
                    </p>
                  </div>
                </footer>
              </RootErrorBoundary>
            </ToastProvider>
          </CSPostHogProvider>
        </WalletProvider>
      </body>
    </html>
  );
}
