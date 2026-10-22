// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

export default tseslint.config(
  { ignores: ['dist/', 'node_modules/'] },
  js.configs.recommended,
  tseslint.configs.recommended,
  {
    files: ['**/*.{js,ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { react, 'react-hooks': reactHooks },
    settings: { react: { version: 'detect' } },
    rules: {
      // Catch real bugs. TypeScript already reports undefined names.
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' }],
      'eqeqeq': ['warn', 'always', { null: 'ignore' }],

      // C18: parseISO('YYYY-MM-DD') creates UTC midnight, which renders as the
      // previous day in US time zones. Use parseLocalDate/fmtSessionDate from
      // utils/dateUtils for date-only fields, or fmtUtcTimestamp for
      // datetime('now') timestamp fields.
      'no-restricted-imports': ['error', {
        paths: [{
          name: 'date-fns',
          importNames: ['parseISO'],
          message: 'parseISO on a date-only string shifts the date in US time zones (C18). Use parseLocalDate/fmtSessionDate from utils/dateUtils instead.',
        }],
      }],

      // React
      'react/jsx-uses-vars': 'error',    // Prevent false "no-unused-vars" for JSX components

      // Hooks
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',

      // Style (warn only — not blocking)
      'no-var': 'warn',
      'prefer-const': 'warn',
    },
  },
  {
    // The one place parseISO is allowed: the helpers that wrap it safely
    files: ['src/utils/dateUtils.ts'],
    rules: { 'no-restricted-imports': 'off' },
  },
);
