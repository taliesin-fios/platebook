// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * InvoicePay — the client's invoice page, from the link in their invoice email.
 * Route: /invoice/:id?token=… (public; the token is checked by the server)
 * Shows the invoice and, when the chef takes card payments, a Pay button
 * (Stripe Checkout). Stripe sends the client back here with &paid=1 or
 * &cancelled=1.
 */
import { useEffect, useState } from 'react';
import { useParams, useSearchParams } from 'react-router';
import { fmtSessionDate, fmtUtcTimestamp } from '../utils/dateUtils';
import type { ClientInvoiceView } from '../types';
import PoweredBy from '../components/PoweredBy';

const money = (v: number | null | undefined) => `$${(Number(v) || 0).toFixed(2)}`;

export default function InvoicePay() {
  const { id } = useParams();
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token') || '';
  const paidBack = searchParams.get('paid') === '1';
  const cancelledBack = searchParams.get('cancelled') === '1';

  const [invoice, setInvoice] = useState<ClientInvoiceView | null>(null);
  const [loadError, setLoadError] = useState('');
  const [paying, setPaying] = useState(false);
  const [payError, setPayError] = useState('');

  useEffect(() => {
    fetch(`/api/invoices/${id}/public?token=${encodeURIComponent(token)}`)
      .then(r => {
        if (r.status === 401 || r.status === 403) throw new Error('link');
        if (!r.ok) throw new Error('missing');
        return r.json();
      })
      .then((data: ClientInvoiceView) => {
        setInvoice(data);
        if (data.business_name) document.title = data.business_name;
      })
      .catch((err: Error) => setLoadError(err.message === 'link'
        ? 'This invoice link is invalid or has expired. Please ask for a new one.'
        : 'We couldn’t find this invoice.'));
  }, [id, token]);

  const pay = async () => {
    setPaying(true);
    setPayError('');
    try {
      const res = await fetch(`/api/invoices/${id}/checkout?token=${encodeURIComponent(token)}`, { method: 'POST' });
      const data: { checkout_url?: string | null; error?: string; code?: string } = await res.json();
      if (res.ok && data.checkout_url) {
        window.location.assign(data.checkout_url);
        return;
      }
      setPayError(data.code === 'ALREADY_PAID'
        ? 'Your payment has gone through. It can take a minute to show here.'
        : data.error || 'Could not start the payment. Please try again.');
    } catch {
      setPayError('Network error — please check your connection and try again');
    }
    setPaying(false);
  };

  if (loadError || !invoice) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center px-6 py-12" style={{ background: '#FBF8F1' }}>
        <div className="max-w-md w-full text-center">
          {loadError ? (
            <>
              <div className="text-5xl mb-4">🔒</div>
              <h1 className="text-xl font-bold mb-2" style={{ color: '#A94442' }}>Unable to load</h1>
              <p className="text-gray-600 text-sm">{loadError}</p>
            </>
          ) : (
            <p style={{ color: '#9CA3AF', fontSize: '14px' }}>Loading your invoice…</p>
          )}
        </div>
      </div>
    );
  }

  const settled = invoice.status === 'paid' || invoice.status === 'void';
  const canPay = invoice.can_pay_online && !settled && (Number(invoice.total) || 0) > 0 && !paidBack;
  const label = invoice.invoice_number ? `Invoice ${invoice.invoice_number}` : 'Invoice';

  return (
    <div className="min-h-screen px-4 py-8" style={{ background: '#FBF8F1', color: '#28302A' }}>
      <div className="max-w-lg mx-auto">
        <div className="text-center mb-6">
          <h1 className="text-3xl font-bold font-serif mb-1" style={{ color: '#3A5C32' }}>
            {invoice.business_name || label}
          </h1>
          <p className="text-gray-500 text-sm">{invoice.business_name ? label : null}</p>
        </div>

        {invoice.status === 'paid' && (
          <div className="mb-5 px-4 py-3 rounded-xl text-sm font-medium" style={{ background: '#EEF3EA', color: '#3A5C32' }}>
            ✓ Paid{invoice.paid_at ? ` on ${fmtUtcTimestamp(invoice.paid_at, 'MMMM d, yyyy')}` : ''}. Thank you!
          </div>
        )}
        {invoice.status === 'void' && (
          <div className="mb-5 px-4 py-3 rounded-xl text-sm font-medium bg-gray-100 text-gray-600">
            This invoice was cancelled. There’s nothing to pay.
          </div>
        )}
        {!settled && paidBack && (
          <div className="mb-5 px-4 py-3 rounded-xl text-sm font-medium" style={{ background: '#EEF3EA', color: '#3A5C32' }}>
            Thanks! Your payment went through. It can take a minute to show here.
          </div>
        )}
        {!settled && cancelledBack && (
          <div className="mb-5 px-4 py-3 rounded-xl text-sm font-medium bg-amber-50 text-amber-700">
            Payment cancelled. You can try again whenever you’re ready.
          </div>
        )}

        <div className="rounded-2xl border bg-white p-5 mb-5" style={{ borderColor: '#E5E7EB' }}>
          <div className="flex justify-between text-sm mb-4 gap-4">
            <div>
              {invoice.client_name && <div className="font-medium">{invoice.client_name}</div>}
              {invoice.issue_date && <div className="text-gray-500">Issued {fmtSessionDate(invoice.issue_date, 'MMM d, yyyy')}</div>}
            </div>
            {invoice.due_date && !settled && (
              <div className="text-right text-gray-500">Due<br /><span className="font-medium" style={{ color: '#28302A' }}>{fmtSessionDate(invoice.due_date, 'MMM d, yyyy')}</span></div>
            )}
          </div>

          {invoice.line_items.length > 0 && (
            <table className="w-full text-sm mb-3">
              <tbody>
                {invoice.line_items.map((l, i) => (
                  <tr key={i} className="border-t border-gray-100">
                    <td className="py-2 pr-2">
                      {l.description}
                      {(Number(l.quantity) || 0) !== 1 && (
                        <span className="text-gray-400"> × {Number(l.quantity) || 0} @ {money(l.unit_price)}</span>
                      )}
                    </td>
                    <td className="py-2 text-right whitespace-nowrap">{money(l.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          <div className="border-t border-gray-200 pt-3 space-y-1 text-sm">
            {(Number(invoice.tax_amount) || 0) > 0 && (
              <>
                <div className="flex justify-between text-gray-500"><span>Subtotal</span><span>{money(invoice.subtotal)}</span></div>
                <div className="flex justify-between text-gray-500"><span>Tax</span><span>{money(invoice.tax_amount)}</span></div>
              </>
            )}
            <div className="flex justify-between font-bold text-base"><span>Total</span><span>{money(invoice.total)}</span></div>
          </div>

          {invoice.message && <p className="mt-4 text-sm text-gray-600 whitespace-pre-line">{invoice.message}</p>}
        </div>

        {canPay && (
          <button type="button" onClick={pay} disabled={paying}
            className="w-full py-3.5 rounded-xl text-white font-semibold text-sm disabled:opacity-60"
            style={{ background: '#5E7F57' }}>
            {paying ? 'Opening secure checkout…' : `Pay ${money(invoice.total)} →`}
          </button>
        )}
        {payError && <p className="text-xs mt-2 text-center" style={{ color: '#A94442' }}>{payError}</p>}

        <PoweredBy className="mt-8" />
      </div>
    </div>
  );
}
