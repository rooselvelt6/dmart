import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test('measurements debug - create patient and check scale chips', async ({ page }) => {
  page.on('console', msg => console.log(`BROWSER CONSOLE [${msg.type()}]: ${msg.text()}`));
  page.on('pageerror', err => console.log(`PAGE ERROR: ${err.message}`));
  page.on('response', resp => {
    if (resp.url().includes('/api/patients') || resp.url().includes('/api/auth/')) {
      console.log(`RESPONSE ${resp.status()}: ${resp.url()}`);
    }
  });
  
  // Login
  await page.goto('/login');
  await page.fill('#login-username', 'admin');
  await page.fill('#login-password', 'WfBZSHynbV0jl5Hv');
  await page.click('button[type="submit"]');
  await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
  await page.waitForTimeout(2000);
  
  // Go to patients/new
  await page.goto('/patients/new');
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1000);
  
  // Fill form
  await page.fill('input[placeholder="Ej: Juan Alberto"]', 'Test');
  await page.fill('input[placeholder="Ej: Pérez García"]', 'Paciente');
  
  const cedula = `V-${Date.now().toString().slice(-7)}`;
  const hc = `HC-${Date.now().toString().slice(-5)}`;
  console.log('Using cedula:', cedula, 'hc:', hc);
  
  await page.fill('input[placeholder="V-00000000"]', cedula);
  await page.fill('input[placeholder="HC-00000"]', hc);
  await page.selectOption('select', { label: 'Masculino' });
  await page.fill('input[type="date"]', '1990-01-01');
  
  const datetimeInputs = page.locator('input[type="datetime-local"]');
  const iso = new Date().toISOString().slice(0, 16);
  await datetimeInputs.nth(0).fill(iso);
  await datetimeInputs.nth(1).fill(iso);
  
  // Submit
  await page.click('button[type="submit"]:has-text("Registrar Paciente")');
  
  // Wait for navigation or error
  await page.waitForTimeout(5000);
  
  const url = page.url();
  console.log('Current URL:', url);
  
  const h1 = await page.locator('h1').textContent().catch(() => 'NO H1');
  console.log('H1 text:', h1);
  
  // Check for error messages
  const errorAlert = await page.locator('[role="alert"]').textContent().catch(() => null);
  console.log('Error alert:', errorAlert);
  
  // Check for scale chips
  const scaleChips = await page.locator('.scale-chip').count();
  console.log('Scale chips count:', scaleChips);
  
  if (url.includes('/patients/') && !url.includes('/new')) {
    console.log('SUCCESS: Navigated to patient detail');
    const scaleChips = page.locator('.scale-chip');
    for (const name of ['APACHE II', 'GCS', 'SOFA', 'SAPS III', 'NEWS2']) {
      const chip = page.locator(`.scale-chip:has-text("${name}")`);
      const visible = await chip.isVisible().catch(() => false);
      console.log(`Scale chip ${name}:`, visible ? 'visible' : 'not visible');
    }
  } else {
    console.log('FAILED: Still on new patient page or other');
  }
});