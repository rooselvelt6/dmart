import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8081/dist',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  // Servidor estático local solo si no se apunta a uno externo (p. ej. CI usa
  // el propio backend sirviendo `dist/` en :3000 vía PLAYWRIGHT_BASE_URL).
  webServer: process.env.PLAYWRIGHT_BASE_URL
    ? undefined
    : {
        command: 'python3 -m http.server 8081',
        url: 'http://localhost:8081/dist',
        reuseExistingServer: !process.env.CI,
        timeout: 60000,
      },
});
