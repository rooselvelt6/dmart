import { test, expect, gotoAuthenticated, PATIENT_DETAIL_URL } from './helpers';

test.describe.serial('Pacientes', () => {
  test.beforeEach(async ({ page }) => {
    await gotoAuthenticated(page, '/patients');
  });

  test('muestra la lista de pacientes con columnas reales', async ({ page }) => {
    // Título real: "Registro de Pacientes"
    await expect(page.locator('h1')).toContainText('Registro de Pacientes');
    // Buscador real
    await expect(page.locator('input[placeholder="Buscar por nombre, cedula o historia clinica..."]')).toBeVisible();
    // Filtros de estado reales
    await expect(page.locator('button:has-text("Activos")')).toBeVisible();
    await expect(page.locator('button:has-text("Egresados")')).toBeVisible();
    await expect(page.locator('button:has-text("Todos")')).toBeVisible();
    // Botón nuevo paciente real (el de la página, no el del sidebar)
    await expect(page.locator('a.btn-primary:has-text("Nuevo Paciente")')).toBeVisible();
  });

  test('filtra pacientes por búsqueda', async ({ page }) => {
    const search = page.locator('input[placeholder="Buscar por nombre, cedula o historia clinica..."]');
    await search.fill('xyz_no_existe');
    // Esperar debounce (300ms) y carga
    await page.waitForTimeout(500);
    // El virtual scrolling renderiza filas con data-testid
    const rows = page.locator('[data-testid="patient-row"]');
    await expect(rows).toHaveCount(0);
  });

  test('crea un nuevo paciente vía formulario real', async ({ page }) => {
    // Click en "+ Nuevo Paciente" -> navega a /patients/new
    await page.click('a.btn-primary:has-text("Nuevo Paciente")');
    await page.waitForURL(/\/patients\/new/, { timeout: 10000 });
    
    // Título real
    await expect(page.locator('h1')).toContainText('Expediente Clínico');
    
    // Llenar campos obligatorios (selectores basados en register.rs)
    await page.fill('input[placeholder="Ej: Juan Alberto"]', 'Test');
    await page.fill('input[placeholder="Ej: Pérez García"]', 'Paciente');
    
    // Cédula: formato V-00000000
    const cedula = `V-${Date.now().toString().slice(-7)}`;
    await page.fill('input[placeholder="V-00000000"]', cedula);
    
    // Historia clínica: formato HC-00000
    const hc = `HC-${Date.now().toString().slice(-5)}`;
    await page.fill('input[placeholder="HC-00000"]', hc);
    
    await page.selectOption('select', { label: 'Masculino' }); // Sexo
    await page.fill('input[type="date"]', '1990-01-01'); // Fecha nacimiento
    
    // Fechas de ingreso (datetime-local) - usar selectores más específicos
    const now = new Date();
    const iso = now.toISOString().slice(0, 16);
    const datetimeInputs = page.locator('input[type="datetime-local"]');
    await datetimeInputs.nth(0).fill(iso); // Hospital
    await datetimeInputs.nth(1).fill(iso); // UCI
    
    // Submit: botón "Registrar Paciente"
    await page.click('button[type="submit"]:has-text("Registrar Paciente")');
    
    // Debe navegar al detalle del paciente creado
    await page.waitForURL(PATIENT_DETAIL_URL, { timeout: 15000 });
    await expect(page.locator('h1')).toContainText('Test Paciente');
  });

  // Va DESPUÉS del alta a propósito: el listado se ordena por `created_at DESC`,
  // así que el paciente recién creado es la primera fila. Antes esto era un
  // `test.skip()` condicional que en CI (BD limpia) saltaba en silencio.
  test('navega al detalle de paciente desde la fila', async ({ page }) => {
    const row = page.locator('[data-testid="patient-row"]', { hasText: 'Test Paciente' }).first();
    await expect(row).toBeVisible({ timeout: 15000 });

    // El enlace "Ver" es un <a> con clase btn-like, no un button
    const verLink = row.locator('a:has-text("Ver")');
    await expect(verLink).toBeVisible();
    await verLink.click();

    // La ruta real es /patients/:id (id = UUID)
    await page.waitForURL(PATIENT_DETAIL_URL, { timeout: 15000 });
    // En el detalle el título es el nombre del paciente
    await expect(page.locator('h1')).toContainText('Test Paciente');
  });

  test('filtra por pestaña Egresados', async ({ page }) => {
    await page.click('button:has-text("Egresados")');
    await expect(page.locator('button:has-text("Egresados")')).toHaveClass(/btn-primary/);
    // Mensaje de vacío o filas
    await page.waitForTimeout(500);
  });
});