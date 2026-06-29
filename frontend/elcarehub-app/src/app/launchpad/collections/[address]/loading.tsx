export default function CollectionDetailLoading() {
  return (
    <div className="min-h-screen bg-brand-50/20 pt-24 pb-12 px-4">
      <div className="max-w-7xl mx-auto">
        <div className="w-36 h-6 rounded-lg bg-gray-200 animate-pulse mb-8" />
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
          <div className="lg:col-span-2 space-y-8">
            <div className="bg-white rounded-3xl p-12 space-y-6">
              <div className="flex gap-3">
                <div className="w-28 h-7 rounded-full bg-gray-200 animate-pulse" />
                <div className="w-20 h-7 rounded-full bg-gray-200 animate-pulse" />
              </div>
              <div className="w-3/4 h-12 rounded-2xl bg-gray-200 animate-pulse" />
              <div className="grid grid-cols-2 gap-6 pt-6 border-t border-gray-100">
                <div className="h-16 rounded-2xl bg-gray-100 animate-pulse" />
                <div className="h-16 rounded-2xl bg-gray-100 animate-pulse" />
              </div>
            </div>
            <div className="bg-white rounded-3xl p-8">
              <div className="w-32 h-8 rounded-lg bg-gray-200 animate-pulse mb-6" />
              <div className="h-40 rounded-2xl bg-gray-100 animate-pulse" />
            </div>
          </div>
          <div className="bg-white rounded-3xl p-8 space-y-6 h-fit">
            <div className="w-36 h-7 rounded-lg bg-gray-200 animate-pulse" />
            <div className="h-20 rounded-2xl bg-gray-100 animate-pulse" />
            <div className="h-14 rounded-2xl bg-gray-200 animate-pulse" />
          </div>
        </div>
      </div>
    </div>
  );
}
