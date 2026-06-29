import {
  AlertCircle,
  RefreshCw,
  Package,
  SearchX,
} from "lucide-react";
import Link from "next/link";

interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry: () => void;
  className?: string;
}

export function ErrorState({ title = "Failed to load", message, onRetry, className = "" }: ErrorStateProps) {
  return (
    <div className={`flex flex-col items-center justify-center py-20 ${className}`}>
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-red-50 text-red-500 mb-4">
        <AlertCircle size={32} />
      </div>
      <h3 className="font-display font-bold text-gray-900 text-lg">{title}</h3>
      <p className="mt-1 text-sm text-gray-500 max-w-sm text-center">{message}</p>
      <button
        onClick={onRetry}
        className="mt-6 flex items-center gap-2 rounded-xl bg-brand-500 px-6 py-2.5 text-sm font-bold text-white hover:bg-brand-600 transition-all"
      >
        <RefreshCw size={14} />
        Try Again
      </button>
    </div>
  );
}

interface EmptyStateAction {
  label: string;
  onClick?: () => void;
  href?: string;
}

interface EmptyStateProps {
  icon?: React.ElementType;
  title: string;
  description?: string;
  action?: EmptyStateAction;
  className?: string;
  iconClassName?: string;
  titleClassName?: string;
  descriptionClassName?: string;
}

export function EmptyState({
  icon: Icon = Package,
  title,
  description,
  action,
  className = "",
  iconClassName = "",
  titleClassName = "",
  descriptionClassName = "",
}: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center justify-center py-20 ${className}`}>
      <div className={`flex h-16 w-16 items-center justify-center rounded-2xl bg-brand-50 text-brand-500 mb-4 ${iconClassName}`}>
        <Icon size={32} />
      </div>
      <h3 className={`font-display font-bold text-gray-900 text-lg ${titleClassName}`}>{title}</h3>
      {description && (
        <p className={`mt-1 text-sm text-gray-500 max-w-sm text-center ${descriptionClassName}`}>{description}</p>
      )}
      {action && action.href ? (
        <Link
          href={action.href}
          className="mt-6 flex items-center gap-2 rounded-xl bg-brand-500 px-6 py-2.5 text-sm font-bold text-white hover:bg-brand-600 transition-all"
        >
          {action.label}
        </Link>
      ) : action?.onClick ? (
        <button
          onClick={action.onClick}
          className="mt-6 flex items-center gap-2 rounded-xl bg-brand-500 px-6 py-2.5 text-sm font-bold text-white hover:bg-brand-600 transition-all"
        >
          {action.label}
        </button>
      ) : null}
    </div>
  );
}

interface NoResultsProps {
  message?: string;
  onClearFilters: () => void;
  className?: string;
}

export function NoResults({
  message = "No artworks match the current filters. Try adjusting your search or filters.",
  onClearFilters,
  className = "",
}: NoResultsProps) {
  return (
    <div className={`flex flex-col items-center justify-center py-20 ${className}`}>
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-brand-50 text-brand-500 mb-4">
        <SearchX size={32} />
      </div>
      <h3 className="font-display font-bold text-gray-900 text-lg">No results found</h3>
      <p className="mt-1 text-sm text-gray-500 max-w-sm text-center">{message}</p>
      <button
        onClick={onClearFilters}
        className="mt-6 flex items-center gap-2 rounded-xl bg-brand-500 px-6 py-2.5 text-sm font-bold text-white hover:bg-brand-600 transition-all"
      >
        Clear Filters
      </button>
    </div>
  );
}
