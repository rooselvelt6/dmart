import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test('login and check session', async ({ page }) => {
  page.on('console', msg => console.log(`BROWSER CONSOLE [${msg.type()}]: ${msg.text()}`));
  page.on('pageerror', err => console.log(`PAGE ERROR: ${err.message}`));
  page.on('response', resp => {
    if (resp.url().includes('/api/auth/') || resp.url().includes('/api/patients')) {
      console.log(`RESPONSE ${resp.status()}: ${resp.url()}`);
    }
  });
  
  await page.goto('/login');
  await page.waitForLoadState('networkidle');
  
  console.log('At login page');
  await page.fill('#login-username', ADMIN_USER);
  await page.fill('#login-password', ADMIN_PASS);
  await page.click('button[type="submit"]');
  
  try {
    await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
    console.log('Login successful, URL:', page.url());
  } catch (e) {
    console.log('Login failed, current URL:', page.url());
    const body = await page.textContent('body');
    console.log('Body:', body?.substring(0, 500));
  }
  
  // Check if we have a session
  const cookies = await page.context().cookies();
  console.log('Cookies:', cookies.map(c => c.name));
  
  // Try to access a protected page
  await page.goto('/patients/new');
  await page.waitForLoadState('networkidle');
  console.log('At patients/new, URL:', page.url());
  
  const h1 = await page.locator('h1').textContent().catch(() => 'NO H1');
  console.log('H1:', h1);
});