export default function AdminLoading() {
  return (
    <div className="min-h-screen bg-gray-50 pt-24 pb-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto space-y-8">
        <div className="w-48 h-8 rounded-xl bg-gray-200 animate-pulse" />
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-24 rounded-xl bg-gray-200 animate-pulse" />
          ))}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          <div className="h-64 rounded-xl bg-gray-200 animate-pulse" />
          <div className="h-64 rounded-xl bg-gray-200 animate-pulse" />
        </div>
        <div className="h-48 rounded-xl bg-gray-200 animate-pulse" />
      </div>
    </div>
  );
}
