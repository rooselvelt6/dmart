# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: login.spec.ts >> Login >> should show login page
- Location: tests/e2e/login.spec.ts:8:7

# Error details

```
Error: expect(locator).toContainText(expected) failed

Locator: locator('h1')
Expected substring: "dMart UCI"
Received string:    "Directory listing for /"
Timeout: 5000ms

Call log:
  - Expect "toContainText" locator('h1') with timeout 5000ms
  - waiting for locator('h1')
    14 × locator resolved to <h1>Directory listing for /</h1>
       - unexpected value "Directory listing for /"

```

```yaml
- heading "Directory listing for /" [level=1]
```

# Test source

```ts
  1  | import { test, expect } from '@playwright/test';
  2  | 
  3  | test.describe('Login', () => {
  4  |   test.beforeEach(async ({ page }) => {
  5  |     await page.goto('/');
  6  |   });
  7  | 
  8  |   test('should show login page', async ({ page }) => {
> 9  |     await expect(page.locator('h1')).toContainText('dMart UCI');
     |                                      ^ Error: expect(locator).toContainText(expected) failed
  10 |     await expect(page.locator('input[type="email"]')).toBeVisible();
  11 |     await expect(page.locator('input[type="password"]')).toBeVisible();
  12 |     await expect(page.locator('button[type="submit"]')).toBeVisible();
  13 |   });
  14 | 
  15 |   test('should login with valid credentials', async ({ page }) => {
  16 |     await page.fill('input[type="email"]', 'admin@uci.local');
  17 |     await page.fill('input[type="password"]', 'admin123');
  18 |     await page.click('button[type="submit"]');
  19 |     
  20 |     // Should redirect to dashboard/patients
  21 |     await expect(page.locator('h1')).toContainText(/Pacientes|Dashboard/i);
  22 |   });
  23 | 
  24 |   test('should show error with invalid credentials', async ({ page }) => {
  25 |     await page.fill('input[type="email"]', 'wrong@uci.local');
  26 |     await page.fill('input[type="password"]', 'wrong');
  27 |     await page.click('button[type="submit"]');
  28 |     
  29 |     await expect(page.locator('.toast, .alert, [role="alert"]')).toBeVisible();
  30 |   });
  31 | });
  32 | 
```