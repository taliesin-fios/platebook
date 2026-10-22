// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// /api is proxied to the Rust API server (server-rs, :3002);
// API_PROXY_TARGET (e.g. http://localhost:3003) overrides it.
export default defineConfig(({ mode }) => ({
  plugins: [react(), tailwindcss()],
  server: {
    host: true,
    port: 5174,
    proxy: {
      '/api': {
        target: loadEnv(mode, '.', '').API_PROXY_TARGET || 'http://localhost:3002',
        changeOrigin: true,
      },
    },
  },
}));
