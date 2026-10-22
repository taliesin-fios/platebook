// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  resetKey?: unknown;
  children?: ReactNode;
}

interface State {
  error: Error | null;
}

// C5: Reset error state when the route changes by accepting a `resetKey` prop
// (parent passes location.key so the boundary clears on navigation).
export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidUpdate(prevProps: Props) {
    if (this.state.error && prevProps.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('ErrorBoundary caught:', error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: '2rem', textAlign: 'center', fontFamily: 'sans-serif' }}>
          <div style={{ fontSize: '2.5rem', marginBottom: '1rem' }}>⚠️</div>
          <div style={{ fontSize: '1.1rem', fontWeight: 600, color: '#0D2B4B', marginBottom: '0.5rem' }}>
            Something went wrong
          </div>
          <div style={{ fontSize: '0.85rem', color: '#666', marginBottom: '1.5rem' }}>
            {this.state.error.message}
          </div>
          <button
            onClick={() => { this.setState({ error: null }); window.location.href = '/'; }}
            style={{
              padding: '0.6rem 1.4rem',
              background: '#0D2B4B',
              color: '#fff',
              border: 'none',
              borderRadius: '0.75rem',
              fontSize: '0.9rem',
              cursor: 'pointer',
            }}
          >
            Back to Dashboard
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
