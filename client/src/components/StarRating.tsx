// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

interface StarRatingProps {
  value?: number | null;
  max?: number;
  onChange?: (value: number) => void;
  size?: number;
}

export default function StarRating({ value: rawValue = 0, max = 5, onChange, size = 20 }: StarRatingProps) {
  const value = rawValue ?? 0;
  return (
    <div className="flex gap-1">
      {Array.from({ length: max }).map((_, i) => (
        <button
          key={i}
          onClick={() => onChange && onChange(i + 1)}
          style={{ minHeight: 'auto', padding: 0 }}
          className={i < value ? 'star-filled' : 'star-empty'}
        >
          <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
          </svg>
        </button>
      ))}
    </div>
  );
}

export function StarDisplay({ value: rawValue = 0, max = 5, size = 16 }: Omit<StarRatingProps, 'onChange'>) {
  const value = rawValue ?? 0;
  return (
    <div className="flex gap-0.5">
      {Array.from({ length: max }).map((_, i) => (
        <svg key={i} width={size} height={size} viewBox="0 0 24 24" fill="currentColor"
          className={i < value ? 'star-filled' : 'star-empty'}>
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </svg>
      ))}
    </div>
  );
}
