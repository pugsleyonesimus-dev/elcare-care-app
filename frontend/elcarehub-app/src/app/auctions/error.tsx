"use client";

import { useEffect } from "react";
import { AlertCircle, RefreshCw, Home } from "lucide-react";
import Link from "next/link";

export default function AuctionsError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error("Auctions Route Error:", error);
  }, [error]);

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col items-center justify-center p-4 text-center">
      <div className="w-16 h-16 rounded-full bg-red-50 flex items-center justify-center mb-6">
        <AlertCircle size={32} className="text-red-400" />
      </div>
      <h2 className="text-2xl font-display font-bold text-gray-900 mb-3">Something went wrong</h2>
      <p className="text-gray-500 mb-8 max-w-sm font-inter">
        We hit an error loading the auctions page. This is usually a temporary issue.
      </p>
      <div className="flex flex-col sm:flex-row gap-4">
        <button
          onClick={reset}
          className="px-6 py-3 rounded-xl bg-brand-500 text-white font-bold hover:bg-brand-600 transition-all flex items-center justify-center gap-2"
        >
          <RefreshCw size={16} />
          Try Again
        </button>
        <Link
          href="/"
          className="px-6 py-3 rounded-xl border border-gray-200 bg-white text-gray-700 font-bold hover:bg-gray-50 transition-all flex items-center justify-center gap-2"
        >
          <Home size={16} />
          Back to Home
        </Link>
      </div>
      {error.digest && (
        <p className="mt-8 text-xs font-mono text-gray-400 uppercase tracking-widest">
          Error ID: {error.digest}
        </p>
      )}
    </div>
  );
}
