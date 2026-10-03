import { test, expect, Page } from '@playwright/test';

/**
 * Helpers compartidos por los specs E2E.
 *
 * Estos selectores reflejan el DOM real de `dmart-app`. La versión anterior de
 * los specs apuntaba a una UI que nunca existió (`input[type="email"]`, rutas
 * `/pacientes`, credenciales `admin@uci.local`), así que fallaban sin llegar a
 * ejecutar nada.
 *
 * La sesión NO se puede falsificar por `localStorage`: el access token vive
 * solo en memoria WASM y la cookie de refresco es httpOnly. El login se hace
 * por el formulario real para que Playwright capture el `Set-Cookie`.
 *
 * IMPORTANTE: La navegación directa con `page.goto()` a rutas protegidas NO
 * funciona bien porque el router verifica `is_auth` antes de que la sesión
 * se restaure vía `/auth/refresh`. La navegación debe ser client-side
 * (click en enlaces del sidebar) DESPUÉS del login inicial.
 */

export const ADMIN_USER = process.env.ADMIN_USERNAME || 'admin';
export const ADMIN_PASS = process.env.ADMIN_PASSWORD || 'WfBZSHynbV0jl5Hv';

/** Login por el formulario real (`#login-username` type=text). */
export async function loginViaForm(page: Page) {
  await page.goto('/login');
  await page.fill('#login-username', ADMIN_USER);
  await page.fill('#login-password', ADMIN_PASS);
  await page.click('button[type="submit"]');
  await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
  // Esperar a que la app hidrate y la sesión esté lista en el dashboard
  await page.waitForTimeout(2000);
}

/**
 * Navegación client-side vía sidebar (funciona porque la sesión ya está lista).
 * Requiere estar en una página con sidebar visible (ej. tras login estamos en /).
 */
export async function navigateSidebar(page: Page, href: string) {
  // El sidebar puede estar colapsado en móvil; forzar apertura si hace falta
  const sidebarLink = page.locator(`nav a[href="${href}"]`);
  if (await sidebarLink.count() === 0) {
    // Sidebar colapsado: abrir menú móvil
    await page.click('button[aria-label="Abrir menú de navegación"]');
  }
  await sidebarLink.click();
  await page.waitForURL(href, { timeout: 15000 });
  await page.waitForTimeout(1000); // hidratación de la nueva vista
}

/**
 * Flujo completo: login + navegación client-side a una ruta interna.
 * Úsalo en `beforeEach` de los specs.
 */
export async function gotoAuthenticated(page: Page, path: string) {
  await loginViaForm(page);
  if (path !== '/') {
    await navigateSidebar(page, path);
  }
}

export { test, expect };