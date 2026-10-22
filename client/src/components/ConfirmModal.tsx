// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useRef, type ReactNode } from 'react';

interface ConfirmModalProps {
  open: boolean;
  title: ReactNode;
  message?: ReactNode;
  confirmLabel?: string;
  confirmClass?: string;
  onConfirm?: () => unknown;
  onCancel?: () => void;
}

// C6: disable while pending, Escape key to cancel, focus trap
export default function ConfirmModal({ open, title, message, confirmLabel = 'Delete', confirmClass = 'bg-danger', onConfirm, onCancel }: ConfirmModalProps) {
  const [pending, setPending] = useState(false);
  const confirmRef = useRef<HTMLButtonElement>(null);

  // Reset pending state when modal opens/closes
  useEffect(() => {
    if (!open) setPending(false);
  }, [open]);

  // Escape key + focus trap
  useEffect(() => {
    if (!open) return;
    confirmRef.current?.focus();
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !pending) onCancel?.();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open, pending, onCancel]);

  if (!open) return null;

  const handleConfirm = async () => {
    if (pending) return;
    setPending(true);
    try {
      await onConfirm?.();
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="overlay" onClick={!pending ? onCancel : undefined}>
      <div className="modal sheet-enter" onClick={e => e.stopPropagation()} style={{ maxHeight: '60vh' }}>
        <h3 className="font-display text-lg font-semibold text-navy mb-2">{title}</h3>
        {message && <p className="text-sm text-gray-600 mb-6">{message}</p>}
        <div className="flex gap-3">
          <button
            onClick={onCancel}
            disabled={pending}
            className="flex-1 py-3 rounded-xl border border-gray-200 text-gray-600 font-medium text-sm disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            ref={confirmRef}
            onClick={handleConfirm}
            disabled={pending}
            className={`flex-1 py-3 rounded-xl text-white font-medium text-sm ${confirmClass} disabled:opacity-60`}
          >
            {pending ? 'Working…' : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
