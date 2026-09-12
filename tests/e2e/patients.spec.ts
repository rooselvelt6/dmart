import { test, expect } from '@playwright/test';

test.describe('Pacientes', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.fill('input[type="email"]', 'admin@uci.local');
    await page.fill('input[type="password"]', 'admin123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/pacientes');
  });

  test('should show patients table', async ({ page }) => {
    await expect(page.locator('h1')).toContainText('Pacientes');
    await expect(page.locator('table')).toBeVisible();
    await expect(page.locator('thead th')).toContainText(['Nombre', 'Edad', 'GCS', 'Gravedad']);
  });

  test('should filter patients', async ({ page }) => {
    await page.fill('input[placeholder*="buscar" i]', 'Juan');
    await expect(page.locator('tbody tr')).toHaveCount(0);
  });

  test('should navigate to patient detail', async ({ page }) => {
    const firstRow = page.locator('tbody tr').first();
    await expect(firstRow).toBeVisible();
    await firstRow.locator('button:has-text("Ver")').click();
    await page.waitForURL(/\/pacientes\/\d+/);
    await expect(page.locator('h1')).toContainText('Detalle');
  });

  test('should create new patient', async ({ page }) => {
    await page.click('button:has-text("Nuevo")');
    await expect(page.locator('h1')).toContainText('Nuevo Paciente');
    
    await page.fill('input[name="nombre"]', 'Test');
    await page.fill('input[name="apellidos"]', 'Paciente');
    await page.fill('input[name="fecha_nacimiento"]', '1990-01-01');
    await page.selectOption('select[name="sexo"]', 'M');
    await page.click('button[type="submit"]:has-text("Guardar")');
    
    await expect(page.locator('.toast, [role="alert"]')).toContainText(/creado|guardado/i);
  });
});
