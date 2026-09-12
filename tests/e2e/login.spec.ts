import { test, expect } from '@playwright/test';

test.describe('Login', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should show login page', async ({ page }) => {
    await expect(page.locator('h1')).toContainText('dMart UCI');
    await expect(page.locator('input[type="email"]')).toBeVisible();
    await expect(page.locator('input[type="password"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toBeVisible();
  });

  test('should login with valid credentials', async ({ page }) => {
    await page.fill('input[type="email"]', 'admin@uci.local');
    await page.fill('input[type="password"]', 'admin123');
    await page.click('button[type="submit"]');
    
    // Should redirect to dashboard/patients
    await expect(page.locator('h1')).toContainText(/Pacientes|Dashboard/i);
  });

  test('should show error with invalid credentials', async ({ page }) => {
    await page.fill('input[type="email"]', 'wrong@uci.local');
    await page.fill('input[type="password"]', 'wrong');
    await page.click('button[type="submit"]');
    
    await expect(page.locator('.toast, .alert, [role="alert"]')).toBeVisible();
  });
});
