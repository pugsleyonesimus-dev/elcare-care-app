export default function LaunchpadAdminLoading() {
  return (
    <div className="min-h-screen bg-gray-50">
      <div className="bg-white border-b border-gray-200 px-8 py-6">
        <div className="max-w-7xl mx-auto flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-gray-200 animate-pulse" />
          <div className="space-y-2">
            <div className="w-56 h-6 rounded-lg bg-gray-200 animate-pulse" />
            <div className="w-40 h-4 rounded-lg bg-gray-200 animate-pulse" />
          </div>
        </div>
      </div>
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 space-y-8">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-24 rounded-xl bg-gray-200 animate-pulse" />
          ))}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          <div className="h-64 rounded-xl bg-gray-200 animate-pulse" />
          <div className="h-64 rounded-xl bg-gray-200 animate-pulse" />
        </div>
      </div>
    </div>
  );
}
