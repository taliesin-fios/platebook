// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useRef, useState } from 'react';
import { useAuth } from '../App';
import { TOUR_STEPS, hasSeenTour, markTourSeen } from '../utils/tour';

interface TourProps {
  open: boolean;
  /** Skip, Done, Escape or a tap outside the card. */
  onClose: () => void;
  /** The last step's "Open the guide" button; left out where the guide is already open. */
  onOpenGuide?: () => void;
  initialStep?: number;
}

/**
 * A short walkthrough of the main screens, one card at a time. It sits over
 * the page like the other dialogs (bottom sheet on a phone, centered on
 * desktop) and doesn't point at individual buttons.
 */
export default function Tour(props: TourProps) {
  // Mounted only while open, so each opening starts from the first step.
  return props.open ? <TourCard {...props} /> : null;
}

function TourCard({ onClose, onOpenGuide, initialStep = 0 }: TourProps) {
  const [index, setIndex] = useState(initialStep);
  const cardRef = useRef<HTMLDivElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; }, [onClose]);

  // Escape closes; Tab stays inside the card; focus returns where it was.
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    primaryRef.current?.focus();
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        closeRef.current();
        return;
      }
      if (e.key !== 'Tab' || !cardRef.current) return;
      const buttons = Array.from(cardRef.current.querySelectorAll<HTMLButtonElement>('button:not([disabled])'));
      if (buttons.length === 0) return;
      const first = buttons[0];
      const last = buttons[buttons.length - 1];
      const active = document.activeElement;
      if (!cardRef.current.contains(active)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', handler);
    return () => {
      document.removeEventListener('keydown', handler);
      before?.focus?.();
    };
  }, []);

  const step = TOUR_STEPS[index];
  const isFirst = index === 0;
  const isLast = index === TOUR_STEPS.length - 1;
  const ghost = 'py-3 px-4 rounded-xl bg-warm-white border border-gray-200 text-navy font-semibold text-sm';

  return (
    <div className="overlay" onClick={onClose}>
      <div
        ref={cardRef}
        className="modal sheet-enter"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tour-title"
        aria-describedby="tour-body"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between gap-3 mb-4">
          <div className="text-xs font-semibold text-gray-500" aria-live="polite">
            Step {index + 1} of {TOUR_STEPS.length}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-sm font-semibold text-gray-600 underline px-2"
          >
            Skip tour
          </button>
        </div>

        <div className="flex items-start gap-3 mb-6">
          <div className="w-11 h-11 shrink-0 rounded-full bg-green-light flex items-center justify-center text-xl" aria-hidden="true">
            {step.icon}
          </div>
          <div className="flex-1 min-w-0">
            <h2 id="tour-title" className="font-display text-lg font-semibold text-green-dark mb-1">{step.title}</h2>
            <p id="tour-body" className="text-sm text-gray-600 leading-relaxed">{step.body}</p>
          </div>
        </div>

        <div className="flex flex-wrap gap-3">
          {!isFirst && (
            <button type="button" onClick={() => setIndex(i => i - 1)} className={ghost}>
              Back
            </button>
          )}
          {isLast && onOpenGuide && (
            <button type="button" onClick={onOpenGuide} className={ghost}>
              Open the guide
            </button>
          )}
          <button
            ref={primaryRef}
            type="button"
            onClick={isLast ? onClose : () => setIndex(i => i + 1)}
            className="flex-1 py-3 px-4 rounded-xl bg-navy text-white font-semibold text-sm"
          >
            {isLast ? 'Done' : 'Next'}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Shows the tour once per account on this device. Closing it any way (Skip,
 * Done, Escape, Open the guide) counts as seen; the guide page replays it.
 */
export function FirstRunTour({ onOpenGuide }: { onOpenGuide: () => void }) {
  const { chef } = useAuth();
  const accountId = chef?.id;
  // The chef is loaded before any page renders, so this can be decided up front.
  const [open, setOpen] = useState(() => accountId != null && !hasSeenTour(accountId));

  const close = () => {
    if (accountId != null) markTourSeen(accountId);
    setOpen(false);
  };

  return <Tour open={open} onClose={close} onOpenGuide={() => { close(); onOpenGuide(); }} />;
}
