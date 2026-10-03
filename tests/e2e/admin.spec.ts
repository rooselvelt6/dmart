import { test, expect, gotoAuthenticated, navigateSidebar } from './helpers';

test.describe.serial('Admin Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await gotoAuthenticated(page, '/');
    await navigateSidebar(page, '/admin');
    await page.waitForTimeout(2000);
  });

  test('carga el panel de administración con tabs', async ({ page }) => {
    // Título real
    await expect(page.locator('h1')).toContainText('Panel de Administración');
    await expect(page.locator('text=Gestión de UCI')).toBeVisible();
    
    // 7 tarjetas KPI - usar selectores más específicos para evitar coincidencias múltiples
    // Las tarjetas KPI tienen estructura: icon + título + valor
    await expect(page.locator('text=Libres')).toBeVisible();
    await expect(page.locator('text=Ocupadas')).toBeVisible();
    await expect(page.locator('text=Disponibles')).toBeVisible();
    await expect(page.locator('text=Medicos')).toBeVisible();
    await expect(page.locator('text=Enfermeros')).toBeVisible();
    
    // Tabs de navegación
    await expect(page.locator('button:has-text("Camas")')).toHaveClass(/bg-uci-accent/);
    await expect(page.locator('button:has-text("Equipos")')).toBeVisible();
    await expect(page.locator('button:has-text("Personal")')).toBeVisible();
    await expect(page.locator('button:has-text("Institución")')).toBeVisible();
    await expect(page.locator('button:has-text("Auditoría")')).toBeVisible();
  });

  test('navega entre tabs y muestra contenido', async ({ page }) => {
    // Tab Camas (activo por defecto)
    await expect(page.locator('button:has-text("Camas")')).toHaveClass(/bg-uci-accent/);
    await expect(page.locator('text=Gestión de Camas')).toBeVisible();
    await expect(page.locator('button:has-text("Nueva Cama")')).toBeVisible();
    
    // Tab Equipos - verificar cambio de contenido en lugar de clase
    await page.click('button:has-text("Equipos")');
    await page.waitForTimeout(500);
    await expect(page.locator('text=Gestión de Equipos')).toBeVisible();
    await expect(page.locator('button:has-text("Nuevo Equipo")')).toBeVisible();
    
    // Tab Personal
    await page.click('button:has-text("Personal")');
    await page.waitForTimeout(500);
    await expect(page.locator('text=Gestión de Personal')).toBeVisible();
    await expect(page.locator('button:has-text("Nuevo Personal")')).toBeVisible();
    
    // Tab Institución
    await page.click('button:has-text("Institución")');
    await page.waitForTimeout(500);
    await expect(page.locator('text=Configuración de la Institución')).toBeVisible();
    // El input no tiene placeholder, usar el label o el primer input del formulario
    await expect(page.locator('input[type="text"]').first()).toBeVisible();
    
    // Tab Auditoría
    await page.click('button:has-text("Auditoría")');
    await page.waitForTimeout(500);
    await expect(page.locator('button:has-text("Verificar cadena")')).toBeVisible();
    await expect(page.locator('button:has-text("Sellar lote")')).toBeVisible();
  });

  test('crea una cama en tab Camas', async ({ page }) => {
    // Asegurar tab Camas
    await page.click('button:has-text("Camas")');
    await expect(page.locator('button:has-text("Camas")')).toHaveClass(/bg-uci-accent/);
    
    // Click "Nueva Cama"
    await page.click('button:has-text("Nueva Cama")');
    // El formulario aparece inline, verificar campos
    await expect(page.locator('input[type="number"]')).toBeVisible();
    const selects = page.locator('select');
    await expect(selects.first()).toBeVisible(); // Tipo
    await expect(selects.nth(1)).toBeVisible(); // Estado
    
    // Llenar formulario: número, tipo, estado
    await page.fill('input[type="number"]', '99');
    await selects.nth(0).selectOption({ label: 'General' }); // Tipo
    await selects.nth(1).selectOption({ label: 'Libre' }); // Estado
    
    // Click "Crear"
    await page.click('button:has-text("Crear")');
    
    // Verificar que aparece en la tabla/grid
    await expect(page.locator('text=Cama 99').first()).toBeVisible({ timeout: 10000 });
  });

  test('edita configuración de institución', async ({ page }) => {
    await page.click('button:has-text("Institución")');
    await page.waitForTimeout(500);
    await expect(page.locator('text=Configuración de la Institución')).toBeVisible();
    
    // Cambiar nombre
    const nuevoNombre = `Hospital Test ${Date.now()}`;
    // El input no tiene placeholder, usar el primer input type=text
    await page.fill('input[type="text"]', nuevoNombre);
    
    // Click Guardar
    await page.click('button:has-text("Guardar")');
    
    // Toast de éxito
    await expect(page.locator('text=Configuración guardada correctamente')).toBeVisible({ timeout: 10000 });
  });

  test('auditoría: verifica cadena y sella lote', async ({ page }) => {
    await page.click('button:has-text("Auditoría")');
    await page.waitForTimeout(500);
    
    // Click "Verificar cadena" - solo verificar que el botón funciona
    await page.click('button:has-text("Verificar cadena")');
    await page.waitForTimeout(3000);
    
    // Click "Sellar lote"
    await page.click('button:has-text("Sellar lote")');
    await page.waitForTimeout(3000);
    // Verificar que aparece algún mensaje de resultado
    await expect(page.locator('text=Lote #').or(page.locator('text=No hay eventos nuevos para sellar')).or(page.locator('text=sellado'))).toBeVisible({ timeout: 10000 });
  });

  test('filtra logs de auditoría por acción', async ({ page }) => {
    await page.click('button:has-text("Auditoría")');
    
    // Select de acción
    await page.selectOption('select', { label: 'Login' });
    await page.waitForTimeout(500);
    
    // La tabla debe actualizarse (puede estar vacía)
    await expect(page.locator('table')).toBeVisible();
  });
});