export default function CreateCollectionLoading() {
  return (
    <div className="min-h-screen bg-brand-50/20 pt-24 pb-12 px-4">
      <div className="max-w-3xl mx-auto space-y-6">
        <div className="flex flex-col items-center mb-8 gap-4">
          <div className="w-32 h-7 rounded-full bg-gray-200 animate-pulse" />
          <div className="w-64 h-14 rounded-2xl bg-gray-200 animate-pulse" />
        </div>
        <div className="bg-white rounded-3xl p-10 space-y-8">
          <div className="grid grid-cols-2 gap-4">
            {[1, 2, 3, 4].map((i) => (
              <div key={i} className="h-20 rounded-2xl bg-gray-100 animate-pulse" />
            ))}
          </div>
          <div className="h-14 rounded-2xl bg-gray-100 animate-pulse" />
          <div className="grid grid-cols-2 gap-4">
            <div className="h-14 rounded-2xl bg-gray-100 animate-pulse" />
            <div className="h-14 rounded-2xl bg-gray-100 animate-pulse" />
          </div>
          <div className="h-16 rounded-2xl bg-gray-200 animate-pulse" />
        </div>
      </div>
    </div>
  );
}
