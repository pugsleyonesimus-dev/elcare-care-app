export default function OffersLoading() {
  return (
    <div className="min-h-screen bg-gray-50 pt-24 pb-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto space-y-6">
        <div className="w-40 h-10 rounded-xl bg-gray-200 animate-pulse" />
        {[1, 2, 3, 4, 5].map((i) => (
          <div key={i} className="h-20 rounded-2xl bg-gray-200 animate-pulse" />
        ))}
      </div>
    </div>
  );
}
