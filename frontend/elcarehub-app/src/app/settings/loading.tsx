export default function SettingsLoading() {
  return (
    <div className="min-h-screen bg-gray-50 pt-24 pb-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-2xl mx-auto space-y-6">
        <div className="w-40 h-10 rounded-xl bg-gray-200 animate-pulse" />
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-32 rounded-2xl bg-gray-200 animate-pulse" />
        ))}
      </div>
    </div>
  );
}
