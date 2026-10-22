// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useCallback } from 'react';
import type { ApiError } from '../types';

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

/** Message of an unknown thrown value (errors from apiCall are always `Error`s). */
export const errorMessage = (err: unknown): string =>
  err instanceof Error ? err.message : String(err);

export function useApi<T>(url: string | null, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // C9: stringify deps so non-memoized object/array references don't trigger infinite loops
  const depsKey = JSON.stringify(deps);

  const fetchData = useCallback(async () => {
    if (!url) return;
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as T;
      setData(json);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, [url]);

  useEffect(() => {
    fetchData();
  }, [fetchData, depsKey]);

  return { data, loading, error, refetch: fetchData };
}

/**
 * JSON fetch helper. Resolves with the parsed body, or throws an `Error`
 * carrying the server's `error` message on a non-2xx response.
 *
 * `T` is the caller's claim about the response shape (see `types/`); nothing
 * checks it at runtime.
 */
export async function apiCall<T = unknown>(url: string, method: HttpMethod = 'GET', body: unknown = null): Promise<T> {
  const options: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (body) options.body = JSON.stringify(body);
  const res = await fetch(url, options);
  const text = await res.text();
  let json: unknown = {};
  try {
    json = text ? JSON.parse(text) : {};
  } catch {
    throw new Error('Server returned an unexpected response');
  }
  if (!res.ok) throw new Error((json as Partial<ApiError>).error || `HTTP ${res.status}`);
  return json as T;
}
