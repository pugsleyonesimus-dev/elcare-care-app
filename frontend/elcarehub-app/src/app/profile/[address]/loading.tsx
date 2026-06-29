export default function ProfileDetailLoading() {
  return (
    <div className="min-h-screen bg-gray-50 pt-24 pb-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center gap-6 mb-10">
          <div className="w-24 h-24 rounded-full bg-gray-200 animate-pulse" />
          <div className="space-y-3">
            <div className="w-56 h-7 rounded-lg bg-gray-200 animate-pulse" />
            <div className="w-40 h-4 rounded-lg bg-gray-200 animate-pulse" />
          </div>
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className="h-56 rounded-2xl bg-gray-200 animate-pulse" />
          ))}
        </div>
      </div>
    </div>
  );
}
