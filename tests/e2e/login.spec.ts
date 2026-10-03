import { test, expect } from './helpers';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

test.describe('Login', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/login');
  });

  test('muestra el formulario de login real', async ({ page }) => {
    // El DOM real: #login-username (type=text, no email) y #login-password.
    await expect(page.locator('#login-username')).toBeVisible();
    await expect(page.locator('#login-password')).toBeVisible();
    // Hay dos botones submit (credenciales y MFA); el visible es el de credenciales.
    await expect(page.locator('form:visible button[type="submit"]')).toBeVisible();
    await expect(page.locator('#login-username')).toHaveAttribute('type', 'text');
  });

  test('acepta credenciales válidas y entra al dashboard', async ({ page }) => {
    await page.fill('#login-username', ADMIN_USER);
    await page.fill('#login-password', ADMIN_PASS);
    await page.click('button[type="submit"]');

    // El login navega a "/" (DashboardPage), no a /patients: ver login.rs:59.
    await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
    await expect(page).not.toHaveURL(/\/login/);
    await expect(page.locator('[role="alert"]')).toHaveCount(0);
  });

  test('muestra error con credenciales inválidas', async ({ page }) => {
    await page.fill('#login-username', ADMIN_USER);
    await page.fill('#login-password', 'definitivamente-incorrecta');
    await page.click('button[type="submit"]');

    // El servidor devuelve 401 y la app pinta el alert con `login-invalid`.
    await expect(page.locator('[role="alert"]')).toBeVisible({ timeout: 15000 });
    await expect(page).toHaveURL(/\/login/);
  });

  test('requiere usuario: el submit no pasa el HTML si falta el campo', async ({ page }) => {
    await page.fill('#login-password', ADMIN_PASS);
    await page.click('button[type="submit"]');
    // `required` en el input bloquea el submit nativo: seguimos en /login.
    await expect(page).toHaveURL(/\/login/);
    await expect(page.locator('#login-username')).toBeVisible();
  });

  test('una ruta protegida sin sesión redirige a /login', async ({ page }) => {
    await page.goto('/patients');
    await page.waitForURL(/\/login/, { timeout: 15000 });
    await expect(page.locator('#login-username')).toBeVisible();
  });
});
