// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

export function SkeletonCard({ lines = 3 }: { lines?: number }) {
  return (
    <div className="card mb-3">
      <div className="skeleton h-5 w-3/4 mb-3" />
      {Array.from({ length: lines - 1 }).map((_, i) => (
        <div key={i} className={`skeleton h-4 mb-2 ${i === lines - 2 ? 'w-1/2' : 'w-full'}`} />
      ))}
    </div>
  );
}

export function SkeletonList({ count = 4 }: { count?: number }) {
  return (
    <div className="px-4">
      {Array.from({ length: count }).map((_, i) => (
        <SkeletonCard key={i} lines={3} />
      ))}
    </div>
  );
}

export function SkeletonStatGrid() {
  return (
    <div className="grid grid-cols-2 gap-3 px-4 mb-4">
      {Array.from({ length: 4 }).map((_, i) => (
        <div key={i} className="card">
          <div className="skeleton h-4 w-1/2 mb-2" />
          <div className="skeleton h-7 w-3/4 mb-1" />
          <div className="skeleton h-3 w-full" />
        </div>
      ))}
    </div>
  );
}
