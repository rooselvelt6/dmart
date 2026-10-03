import { test, expect, gotoAuthenticated } from './helpers';

test.describe.serial('Mediciones y Escalas', () => {
  let patientId: string;

  test.beforeEach(async ({ page }) => {
    await gotoAuthenticated(page, '/patients');
    
    await page.click('a.btn-primary:has-text("Nuevo Paciente")');
    await page.waitForURL(/\/patients\/new/, { timeout: 10000 });
    
    const cedula = `V-${Date.now().toString().slice(-7)}`;
    const hc = `HC-${Date.now().toString().slice(-5)}`;
    const iso = new Date().toISOString().slice(0, 16);
    
    await page.fill('input[placeholder="Ej: Juan Alberto"]', 'Escala');
    await page.fill('input[placeholder="Ej: Pérez García"]', 'Test');
    await page.fill('input[placeholder="V-00000000"]', cedula);
    await page.fill('input[placeholder="HC-00000"]', hc);
    await page.selectOption('select', { label: 'Masculino' });
    await page.fill('input[type="date"]', '1980-05-15');
    const datetimeInputs = page.locator('input[type="datetime-local"]');
    await datetimeInputs.nth(0).fill(iso);
    await datetimeInputs.nth(1).fill(iso);
    
    await page.click('button[type="submit"]:has-text("Registrar Paciente")');
    await page.waitForURL(/\/patients\/([^/]+)$/, { timeout: 15000 });
    await expect(page.locator('.scale-chip:has-text("APACHE II")')).toBeVisible({ timeout: 20000 });
    
    patientId = page.url().match(/\/patients\/([^/]+)$/)?.[1] || '';
  });

  test('muestra chips de escalas en el detalle del paciente', async ({ page }) => {
    await expect(page.locator('.scale-chip:has-text("APACHE II")')).toBeVisible();
    await expect(page.locator('.scale-chip:has-text("GCS")')).toBeVisible();
    await expect(page.locator('.scale-chip:has-text("SOFA")')).toBeVisible();
    await expect(page.locator('.scale-chip:has-text("SAPS III")')).toBeVisible();
    await expect(page.locator('.scale-chip:has-text("NEWS2")')).toBeVisible();
  });

  test('calcula APACHE II mediante sliders', async ({ page }) => {
    await page.click('.scale-chip:has-text("APACHE II")');
    await expect(page.locator('button:has-text("APACHE II"):not(:has-text("Registrar"))')).toHaveClass(/scale-105/);
    
    const tempSlider = page.locator('input[type="range"][aria-label="Temp (°C)"]');
    await expect(tempSlider).toBeVisible();
    
    await tempSlider.evaluate((el: HTMLInputElement) => {
      el.value = '38.5';
      el.dispatchEvent(new Event('input', { bubbles: true }));
    });
    
    await expect(page.locator('span:has-text("38.5")').first()).toBeVisible();
    
    const sliders = [
      { label: 'PAM', value: '85' },
      { label: 'FC', value: '110' },
      { label: 'FR', value: '25' },
      { label: 'FiO2', value: '0.5' },
      { label: 'PaO2', value: '120' },
      { label: 'pH', value: '7.35' },
      { label: 'Sodio', value: '140' },
      { label: 'Potasio', value: '4.0' },
      { label: 'Creatinina', value: '1.2' },
      { label: 'Hematocrito', value: '35' },
      { label: 'Leucocitos', value: '12' },
    ];
    
    for (const s of sliders) {
      const slider = page.locator(`input[type="range"][aria-label="${s.label}"]`);
      await slider.evaluate((el: HTMLInputElement, v: string) => {
        el.value = v;
        el.dispatchEvent(new Event('input', { bubbles: true }));
      }, s.value);
    }
    
    const edadSlider = page.locator('input[type="range"][aria-label="Edad"]');
    await edadSlider.evaluate((el: HTMLInputElement) => {
      el.value = '65';
      el.dispatchEvent(new Event('input', { bubbles: true }));
    });
    
    await page.locator('input[type="range"][aria-label="Ojos"]').evaluate((el: HTMLInputElement) => { el.value = '4'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.locator('input[type="range"][aria-label="Verbal"]').evaluate((el: HTMLInputElement) => { el.value = '5'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.locator('input[type="range"][aria-label="Motor"]').evaluate((el: HTMLInputElement) => { el.value = '6'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    
    await expect(page.locator('text=APACHE').first()).toBeVisible();
    const apacheScore = page.locator('text=/^\\d+$/').first();
    await expect(apacheScore).toBeVisible();
    
    await page.click('button:has-text("Registrar APACHE II")');
    await page.waitForURL(/\/patients\/[^/]+$/, { timeout: 15000 });
    await page.waitForTimeout(1000);
    await expect(page.locator('h1')).toContainText('Escala Test');
  });

  test('calcula GCS mediante sliders', async ({ page }) => {
    await page.click('.scale-chip:has-text("GCS")');
    await page.waitForURL(/\/measure\?escala=gcs/, { timeout: 10000 });
    await page.waitForTimeout(1000);
    
    await expect(page.locator('button:has-text("GCS"):not(:has-text("Registrar"))')).toHaveClass(/scale-105/);
    
    await page.locator('input[type="range"][aria-label="Ojos (E)"]').evaluate((el: HTMLInputElement) => { el.value = '3'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.locator('input[type="range"][aria-label="Verbal (V)"]').evaluate((el: HTMLInputElement) => { el.value = '4'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.locator('input[type="range"][aria-label="Motor (M)"]').evaluate((el: HTMLInputElement) => { el.value = '5'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    
    await expect(page.locator('text=12').first()).toBeVisible();
    await expect(page.locator('text=/15')).toBeVisible();
    
    await page.click('button:has-text("Registrar GCS")');
    await page.waitForURL(/\/patients\/[^/]+$/, { timeout: 15000 });
  });

  test('cambia entre pestañas de escalas y ve sliders correspondientes', async ({ page }) => {
    // Click en chip SOFA → el botón se activa (clase scale-105) y aparecen sus sliders
    await page.click('.scale-chip:has-text("SOFA")');
    await expect(page.locator('button:has-text("SOFA"):not(:has-text("Registrar"))')).toHaveClass(/scale-105/);
    await expect(page.locator('input[type="range"][aria-label="Respiratorio (PaO2/FiO2)"]')).toBeVisible();
    await expect(page.locator('input[type="range"][aria-label="Cardiovascular (PAM)"]')).toBeVisible();
    
    // Click en SAPS III
    await page.click('button:has-text("SAPS III"):not(:has-text("Registrar"))');
    await expect(page.locator('button:has-text("SAPS III"):not(:has-text("Registrar"))')).toHaveClass(/scale-105/);
    await expect(page.locator('input[type="range"][aria-label="GCS"]')).toBeVisible();
    
    // Click en NEWS2
    await page.click('button:has-text("NEWS2"):not(:has-text("Registrar"))');
    await expect(page.locator('button:has-text("NEWS2"):not(:has-text("Registrar"))')).toHaveClass(/scale-105/);
    await expect(page.locator('input[type="range"][aria-label="FR"]')).toBeVisible();
    await expect(page.locator('button:has-text("Sí (+2)")')).toBeVisible();
    await expect(page.locator('button:has-text("Aire Ambiente")')).toBeVisible();
    
    // Scores en tiempo real (sidebar/radar)
    await expect(page.locator('text=Scores en Tiempo Real')).toBeVisible();
    await expect(page.locator('text=APACHE').first()).toBeVisible();
    await expect(page.locator('text=GCS').first()).toBeVisible();
    await expect(page.locator('text=SOFA').first()).toBeVisible();
  });
});