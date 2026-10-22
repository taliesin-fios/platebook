// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { createContext, useContext, useState, useCallback, useRef, type ReactNode } from 'react';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

interface ToastItem {
  id: number;
  message: ReactNode;
  type: ToastType;
  undoFn: (() => void) | null;
}

interface ToastContextValue {
  show: (message: ReactNode, type?: ToastType, duration?: number, undoFn?: (() => void) | null) => number;
  dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export const useToast = (): ToastContextValue => {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used inside <ToastProvider>');
  return ctx;
};

let toastId = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  // C7: track timer IDs so we can clear them on early dismiss
  const timers = useRef<Record<number, ReturnType<typeof setTimeout>>>({});

  const dismiss = useCallback((id: number) => {
    clearTimeout(timers.current[id]);
    delete timers.current[id];
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const show = useCallback((message: ReactNode, type: ToastType = 'success', duration = 3000, undoFn: (() => void) | null = null) => {
    const id = ++toastId;
    setToasts(prev => [...prev, { id, message, type, undoFn }]);
    timers.current[id] = setTimeout(() => dismiss(id), duration + 300);
    return id;
  }, [dismiss]);

  const typeStyles: Record<ToastType, string> = {
    success: 'bg-green-dark text-white',
    error: 'bg-danger text-white',
    info: 'bg-navy text-white',
    warning: 'bg-amber-500 text-white',
  };

  return (
    <ToastContext.Provider value={{ show, dismiss }}>
      {children}
      <div className="fixed bottom-20 left-4 right-4 z-100 flex flex-col gap-2 pointer-events-none md:left-[220px]">
        {toasts.map(t => (
          <div
            key={t.id}
            className={`toast-enter pointer-events-auto flex items-center justify-between gap-3 px-4 py-3 rounded-xl shadow-lg ${typeStyles[t.type] || typeStyles.success}`}
          >
            <span className="text-sm font-medium">{t.message}</span>
            <div className="flex items-center gap-2">
              {t.undoFn && (
                <button
                  onClick={() => { t.undoFn?.(); dismiss(t.id); }}
                  className="text-sm font-bold underline opacity-90 hover:opacity-100"
                  style={{ minHeight: 'auto' }}
                >
                  Undo
                </button>
              )}
              <button
                onClick={() => dismiss(t.id)}
                className="opacity-70 hover:opacity-100 font-bold"
                style={{ minHeight: 'auto' }}
              >
                ×
              </button>
            </div>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
