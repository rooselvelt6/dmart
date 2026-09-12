import { test, expect } from '@playwright/test';

test.describe('Admin Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.fill('input[type="email"]', 'admin@uci.local');
    await page.fill('input[type="password"]', 'admin123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/pacientes');
  });

  test('should navigate to admin', async ({ page }) => {
    await page.click('a[href="/admin"], button:has-text("Admin")');
    await page.waitForURL('**/admin');
    await expect(page.locator('h1')).toContainText('Admin');
  });

  test('should show KPIs', async ({ page }) => {
    await page.goto('/admin');
    await expect(page.locator('text=Egresados')).toBeVisible();
    await expect(page.locator('text=Fallecidos')).toBeVisible();
    await expect(page.locator('text=Mortalidad')).toBeVisible();
    await expect(page.locator('text=LOS Promedio')).toBeVisible();
  });

  test('should show FHIR endpoints', async ({ page }) => {
    await page.goto('/admin');
    await expect(page.locator('text=FHIR')).toBeVisible();
    await expect(page.locator('a[href*="fhir"]')).toBeVisible();
  });

  test('should show QR code generation', async ({ page }) => {
    await page.goto('/admin');
    await page.click('button:has-text("QR"), a:has-text("QR")');
    await expect(page.locator('img[alt*="QR"], canvas')).toBeVisible();
  });
});
