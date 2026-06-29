import React from "react";
import { AlertTriangle, ShieldAlert, X, Loader2 } from "lucide-react";

interface AdminConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  actionDescription: string;
  consequences: string[];
  isProcessing?: boolean;
  confirmText?: string;
  confirmVariant?: "danger" | "warning" | "info";
}

export function AdminConfirmationModal({
  isOpen,
  onClose,
  onConfirm,
  title,
  actionDescription,
  consequences,
  isProcessing = false,
  confirmText = "Confirm Action",
  confirmVariant = "danger",
}: AdminConfirmationModalProps) {
  if (!isOpen) return null;

  const variantClasses = {
    danger: "bg-red-600 hover:bg-red-700 focus:ring-red-500 shadow-red-200",
    warning: "bg-orange-500 hover:bg-orange-600 focus:ring-orange-400 shadow-orange-200",
    info: "bg-brand-500 hover:bg-brand-600 focus:ring-brand-400 shadow-brand-200",
  };

  const iconClasses = {
    danger: "text-red-600 bg-red-50",
    warning: "text-orange-600 bg-orange-50",
    info: "text-brand-600 bg-brand-50",
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-midnight-950/40 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="relative w-full max-w-md overflow-hidden rounded-3xl bg-white shadow-2xl animate-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-100 p-6">
          <div className="flex items-center gap-3">
            <div className={`flex h-10 w-10 items-center justify-center rounded-xl ${iconClasses[confirmVariant]}`}>
              <ShieldAlert className="h-6 w-6" />
            </div>
            <h3 className="font-display text-xl font-bold text-midnight-950">{title}</h3>
          </div>
          <button
            onClick={onClose}
            className="rounded-full p-1 text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-600"
            disabled={isProcessing}
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6">
          <div className="mb-6 rounded-2xl bg-gray-50 p-4 text-sm text-gray-600 border border-gray-100">
            <p className="font-medium text-midnight-900 mb-2">You are about to:</p>
            <p className="leading-relaxed">{actionDescription}</p>
          </div>

          <div className="mb-8">
            <p className="mb-3 flex items-center gap-2 text-xs font-bold uppercase tracking-widest text-gray-400">
              <AlertTriangle className="h-3.5 w-3.5 text-orange-500" />
              Potential Consequences
            </p>
            <ul className="space-y-2">
              {consequences.map((c, i) => (
                <li key={i} className="flex items-start gap-2.5 text-sm text-gray-600">
                  <div className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-gray-300" />
                  {c}
                </li>
              ))}
            </ul>
          </div>

          <div className="flex flex-col gap-3">
            <button
              onClick={onConfirm}
              disabled={isProcessing}
              className={`flex w-full items-center justify-center gap-2 rounded-2xl py-3.5 text-sm font-bold text-white shadow-lg transition-all focus:outline-none focus:ring-2 focus:ring-offset-2 active:scale-95 disabled:opacity-50 disabled:active:scale-100 ${variantClasses[confirmVariant]}`}
            >
              {isProcessing ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Processing...
                </>
              ) : (
                confirmText
              )}
            </button>
            <button
              onClick={onClose}
              disabled={isProcessing}
              className="w-full rounded-2xl py-3 text-sm font-semibold text-gray-500 transition-colors hover:bg-gray-50 hover:text-gray-700"
            >
              Cancel
            </button>
          </div>
        </div>

        {/* Footer Note */}
        <div className="bg-gray-50/50 px-6 py-4 text-center">
          <p className="text-[10px] font-medium uppercase tracking-widest text-gray-400">
            Requires Wallet Signature to proceed
          </p>
        </div>
      </div>
    </div>
  );
}
