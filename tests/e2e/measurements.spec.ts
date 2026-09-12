import { test, expect } from '@playwright/test';

test.describe('Mediciones y Escalas', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.fill('input[type="email"]', 'admin@uci.local');
    await page.fill('input[type="password"]', 'admin123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/pacientes');
    
    // Navigate to first patient
    const firstRow = page.locator('tbody tr').first();
    await firstRow.locator('button:has-text("Ver")').click();
    await page.waitForURL(/\/pacientes\/\d+/);
  });

  test('should show patient scales', async ({ page }) => {
    await expect(page.locator('text=APACHE II')).toBeVisible();
    await expect(page.locator('text=SOFA')).toBeVisible();
    await expect(page.locator('text=NEWS2')).toBeVisible();
    await expect(page.locator('text=SAPS III')).toBeVisible();
    await expect(page.locator('text=GCS')).toBeVisible();
  });

  test('should calculate APACHE II', async ({ page }) => {
    await page.click('button:has-text("APACHE II")');
    await expect(page.locator('h2')).toContainText('APACHE II');
    
    // Fill required fields
    await page.fill('input[name="temperatura"]', '38.5');
    await page.fill('input[name="presion_arterial_media"]', '85');
    await page.fill('input[name="frecuencia_cardiaca"]', '110');
    await page.fill('input[name="frecuencia_respiratoria"]', '25');
    await page.fill('input[name="fio2"]', '0.5');
    await page.fill('input[name="pao2"]', '120');
    await page.fill('input[name="ph_arterial"]', '7.35');
    await page.fill('input[name="sodio_serico"]', '140');
    await page.fill('input[name="potasio_serico"]', '4.0');
    await page.fill('input[name="creatinina"]', '1.2');
    await page.fill('input[name="hematocrito"]', '35');
    await page.fill('input[name="leucocitos"]', '12');
    await page.selectOption('select[name="gcs_ojos"]', '4');
    await page.selectOption('select[name="gcs_verbal"]', '5');
    await page.selectOption('select[name="gcs_motor"]', '6');
    await page.selectOption('select[name="edad"]', '65');
    
    await page.click('button[type="submit"]:has-text("Calcular")');
    await expect(page.locator('text=Score:')).toBeVisible();
  });

  test('should calculate GCS', async ({ page }) => {
    await page.click('button:has-text("GCS")');
    await expect(page.locator('h2')).toContainText('Glasgow');
    
    await page.selectOption('select[name="gcs_ojos"]', '3');
    await page.selectOption('select[name="gcs_verbal"]', '4');
    await page.selectOption('select[name="gcs_motor"]', '5');
    
    await page.click('button[type="submit"]:has-text("Calcular")');
    await expect(page.locator('text=Total:')).toBeVisible();
  });

  test('should show animated score bars', async ({ page }) => {
    // Check that score bars have animation classes
    const scoreBars = page.locator('.score-pulse-normal, .score-pulse-warning, .score-pulse-critical');
    await expect(scoreBars.first()).toBeVisible();
  });
});
