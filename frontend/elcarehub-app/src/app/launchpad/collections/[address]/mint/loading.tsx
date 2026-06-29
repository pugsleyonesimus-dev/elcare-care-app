export default function MintLoading() {
  return (
    <div className="min-h-screen bg-brand-50/20 pt-24 pb-12 px-4">
      <div className="max-w-2xl mx-auto space-y-6">
        <div className="w-40 h-6 rounded-lg bg-gray-200 animate-pulse" />
        <div className="bg-white rounded-3xl p-8 space-y-4">
          <div className="w-24 h-4 rounded-lg bg-gray-200 animate-pulse" />
          <div className="w-2/3 h-9 rounded-xl bg-gray-200 animate-pulse" />
          <div className="w-full h-4 rounded-lg bg-gray-100 animate-pulse" />
        </div>
        <div className="bg-white rounded-3xl p-8 space-y-4">
          <div className="w-32 h-7 rounded-lg bg-gray-200 animate-pulse" />
          <div className="h-14 rounded-2xl bg-gray-100 animate-pulse" />
          <div className="h-14 rounded-2xl bg-gray-100 animate-pulse" />
          <div className="h-14 rounded-2xl bg-gray-200 animate-pulse" />
        </div>
      </div>
    </div>
  );
}
