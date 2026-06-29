// ─────────────────────────────────────────────────────────────
// components/Skeletons.tsx — Reusable skeleton loading components
// ─────────────────────────────────────────────────────────────

export function ListingCardSkeleton() {
  return (
    <div className="flex flex-col overflow-hidden rounded-2xl border border-gray-100 bg-white shadow-sm animate-pulse">
      {/* Image skeleton */}
      <div className="aspect-square bg-gray-100" />
      
      {/* Info skeleton */}
      <div className="flex flex-1 flex-col p-4 space-y-3">
        <div className="h-4 w-3/4 rounded bg-gray-100" />
        <div className="h-3 w-1/2 rounded bg-gray-100" />
        <div className="mt-3 space-y-1">
          <div className="h-3 w-2/3 rounded bg-gray-100" />
        </div>
        <div className="mt-4 flex items-center justify-between">
          <div className="h-6 w-24 rounded bg-gray-100" />
          <div className="h-9 w-24 rounded-lg bg-gray-100" />
        </div>
      </div>
    </div>
  );
}

export function AuctionCardSkeleton() {
  return (
    <div className="rounded-2xl border border-gray-100 bg-white shadow-sm overflow-hidden animate-pulse">
      {/* Image skeleton */}
      <div className="aspect-square bg-gray-100" />
      
      {/* Info skeleton */}
      <div className="p-4 space-y-3">
        <div className="h-4 w-3/4 rounded bg-gray-100" />
        <div className="flex items-center justify-between">
          <div className="h-4 w-28 rounded bg-gray-100" />
          <div className="h-3 w-20 rounded bg-gray-100" />
        </div>
        <div className="h-3 w-32 rounded bg-gray-100" />
      </div>
    </div>
  );
}

export function FeaturedListingSkeleton() {
  return (
    <div className="relative aspect-[4/5] overflow-hidden rounded-2xl animate-pulse bg-gray-100" />
  );
}
