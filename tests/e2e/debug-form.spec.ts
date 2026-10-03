import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test('debug form submission', async ({ page }) => {
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

  // Verificar campos antes de submit
  const nombre = await page.inputValue('input[placeholder="Ej: Juan Alberto"]');
  const apellido = await page.inputValue('input[placeholder="Ej: Pérez García"]');
  const cedula = await page.inputValue('input[placeholder="V-00000000"]');
  const hc = await page.inputValue('input[placeholder="HC-00000"]');
  console.log('Fields before submit:', { nombre, apellido, cedula, hc });

  // Submit
  await page.click('button[type="submit"]:has-text("Registrar Paciente")');
  await page.waitForTimeout(3000);

  // Verificar URL y contenido
  console.log('URL after submit:', page.url());
  const h1 = await page.locator('h1').textContent().catch(() => 'NO H1');
  console.log('H1:', h1);

  // Verificar si hay error visible
  const error = await page.locator('text=/error|Error|invalid|Invalid/i').first().textContent().catch(() => 'NO ERROR');
  console.log('Error visible:', error);

  // Verificar si hay alert
  const alert = await page.locator('[role="alert"]').textContent().catch(() => 'NO ALERT');
  console.log('Alert:', alert);
});