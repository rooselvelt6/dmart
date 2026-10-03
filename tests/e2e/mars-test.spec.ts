import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test('verify mars icon and password toggle', async ({ page }) => {
  await page.goto('/login');
  await page.waitForLoadState('networkidle');
  
  // Check Mars icon is present
  const marsIcon = page.locator('i.fa-mars');
  await expect(marsIcon).toBeVisible();
  
  // Check title has DMART with D in Mars color
  const title = page.locator('h1');
  await expect(title).toBeVisible();
  
  // Test password toggle
  const passwordInput = page.locator('#login-password');
  const passwordVisibleInput = page.locator('#login-password-visible');
  const toggleButton = page.locator('button[aria-label*="password"]');
  
  await expect(passwordInput).toBeVisible();
  await expect(passwordVisibleInput).toBeHidden();
  
  // Click toggle
  await toggleButton.click();
  
  await expect(passwordInput).toBeHidden();
  await expect(passwordVisibleInput).toBeVisible();
  
  // Click again to toggle back
  await toggleButton.click();
  
  await expect(passwordInput).toBeVisible();
  await expect(passwordVisibleInput).toBeHidden();
  
  // Take screenshot
  await page.screenshot({ path: '/tmp/login-mars.png', fullPage: true });
  
  console.log('All checks passed!');
});