// SPDX-License-Identifier: MIT
import { defineConfig } from '@playwright/test';
export default defineConfig({ testDir: './tests/ui', workers: 1, timeout: 60000, expect: { timeout: 15000 }, use: { baseURL: 'http://127.0.0.1:1421', viewport: { width: 1280, height: 900 }, headless: true, ...(process.env.HUSHLINE_EDGE ? { channel: 'msedge' } : {}) }, webServer: { command: 'npm exec vite preview -- --host 127.0.0.1 --port 1421 --strictPort', url: 'http://127.0.0.1:1421', reuseExistingServer: false, timeout: 120000 } });
