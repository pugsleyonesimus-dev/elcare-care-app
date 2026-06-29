export default function AuctionDetailLoading() {
  return (
    <div className="min-h-screen bg-gray-50 pt-24 pb-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto">
        <div className="w-32 h-6 rounded-lg bg-gray-200 animate-pulse mb-8" />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-12">
          <div className="h-96 rounded-3xl bg-gray-200 animate-pulse" />
          <div className="space-y-6">
            <div className="w-3/4 h-10 rounded-xl bg-gray-200 animate-pulse" />
            <div className="w-1/2 h-6 rounded-lg bg-gray-200 animate-pulse" />
            <div className="h-32 rounded-2xl bg-gray-200 animate-pulse" />
            <div className="h-14 rounded-2xl bg-gray-200 animate-pulse" />
          </div>
        </div>
      </div>
    </div>
  );
}
