// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import type { PnlReport } from '../types';

/** The server accepts years from 2000. */
const MIN_YEAR = 2000;

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const money = (n: number) =>
  `${n < 0 ? '-' : ''}$${Math.abs(n).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

/** Monthly profit and loss for tax time: cash basis, with a CSV download. */
export default function ProfitLoss() {
  // null until the first answer, so the server picks "this year" in the account's time zone
  const [year, setYear] = useState<number | null>(null);
  const [thisYear, setThisYear] = useState<number | null>(null);
  const { data, loading, error } = useApi<PnlReport>(
    `/api/costs/pnl${year ? `?year=${year}` : ''}`,
    [year],
  );
  if (data && thisYear === null) setThisYear(data.year);

  const shown = year ?? data?.year ?? null;
  // While another year loads, don't show the previous year's numbers under the new label.
  const report = data && data.year === shown ? data : null;
  const go = (delta: number) => {
    if (shown !== null) setYear(shown + delta);
  };

  return (
    <div className="card mb-4">
      <div className="flex items-center justify-between mb-2">
        <div className="section-header mb-0">Profit &amp; Loss</div>
        <div className="flex items-center gap-2">
          <button type="button" aria-label="Previous year" onClick={() => go(-1)}
            disabled={shown === null || shown <= MIN_YEAR}
            className="px-2 text-navy disabled:opacity-30" style={{ minHeight: 'auto' }}>‹</button>
          <span className="text-sm font-medium text-navy">{shown ?? ''}</span>
          <button type="button" aria-label="Next year" onClick={() => go(1)}
            disabled={thisYear === null || shown === null || shown >= thisYear}
            className="px-2 text-navy disabled:opacity-30" style={{ minHeight: 'auto' }}>›</button>
        </div>
      </div>
      <p className="text-xs text-gray-400 mb-3">
        Paid invoices by the month they were paid, less ingredient costs of sessions held that month.
        Sales tax collected is shown apart and isn&apos;t income.
      </p>

      {loading && !report && <div className="text-sm text-gray-400">Loading…</div>}
      {error && !loading && <div className="text-sm text-red-600">Couldn&apos;t load the report.</div>}

      {report && (
        <>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-xs text-gray-500 text-right">
                  <th className="text-left font-medium py-1">Month</th>
                  <th className="font-medium py-1">Revenue</th>
                  <th className="font-medium py-1">Costs</th>
                  <th className="font-medium py-1">Net</th>
                </tr>
              </thead>
              <tbody>
                {report.months.map(m => (
                  <tr key={m.month} className="border-t border-gray-100 text-right">
                    <td className="text-left py-1.5 text-navy">{MONTHS[m.month - 1]}</td>
                    <td className="py-1.5">{money(m.revenue)}</td>
                    <td className="py-1.5">{money(m.ingredient_costs)}</td>
                    <td className={`py-1.5 font-medium ${m.net < 0 ? 'text-red-600' : 'text-navy'}`}>{money(m.net)}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr className="border-t-2 border-gray-200 text-right font-bold">
                  <td className="text-left py-1.5 text-navy">Total</td>
                  <td className="py-1.5">{money(report.totals.revenue)}</td>
                  <td className="py-1.5">{money(report.totals.ingredient_costs)}</td>
                  <td className={`py-1.5 ${report.totals.net < 0 ? 'text-red-600' : 'text-navy'}`}>{money(report.totals.net)}</td>
                </tr>
              </tfoot>
            </table>
          </div>
          <div className="text-xs text-gray-500 mt-3 space-y-1">
            <div>Sales tax collected: {money(report.totals.sales_tax)}</div>
            <div>
              Booking deposits received: {money(report.deposits_received)} (not in the totals; if you
              put a deposit on an invoice it is already counted there)
            </div>
          </div>
          <button type="button"
            onClick={() => window.open(`/api/costs/pnl/export-csv?year=${report.year}`, '_blank')}
            className="mt-3 text-xs text-navy-mid font-medium border border-navy-mid px-2 py-1 rounded-lg"
            style={{ minHeight: 'auto' }}>Export CSV</button>
        </>
      )}
    </div>
  );
}
