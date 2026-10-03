import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test('crea un nuevo paciente vía formulario real - debug', async ({ page }) => {
  // Capture console messages
  page.on('console', msg => console.log(`BROWSER CONSOLE [${msg.type()}]: ${msg.text()}`));
  page.on('pageerror', err => console.log(`PAGE ERROR: ${err.message}`));
  
  await page.goto('/login');
  await page.fill('#login-username', ADMIN_USER);
  await page.fill('#login-password', ADMIN_PASS);
  await page.click('button[type="submit"]');
  await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
  await page.waitForTimeout(2000);

  await page.goto('/patients/new');
  await page.waitForTimeout(2000);

  // Llenar formulario
  await page.fill('input[placeholder="Ej: Juan Alberto"]', 'Test');
  await page.fill('input[placeholder="Ej: Pérez García"]', 'Paciente');
  await page.fill('input[placeholder="V-00000000"]', 'V-12345678');
  await page.fill('input[placeholder="HC-00000"]', 'HC-12345');
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
  
  const h1 = await page.locator('h1').textContent();
  console.log('H1 text:', h1);
  
  // Check for error messages
  const errorAlert = await page.locator('[role="alert"]').textContent().catch(() => null);
  console.log('Error alert:', errorAlert);
  
  // Check current URL
  if (url.includes('/patients/') && !url.includes('/new')) {
    console.log('SUCCESS: Navigated to patient detail');
  } else {
    console.log('FAILED: Still on new patient page or other');
  }
});