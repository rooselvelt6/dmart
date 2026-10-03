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
 * por la API dentro del contexto del navegador para que Playwright capture el
 * `Set-Cookie`, y la app reanuda sola con `/auth/refresh` al cargar.
 */

export const ADMIN_USER = process.env.ADMIN_USERNAME || 'admin';
export const ADMIN_PASS = process.env.ADMIN_PASSWORD || 'E2eTest!2026';

/** Login por la API desde el contexto del navegador (captura la cookie de refresco). */
export async function loginViaApi(page: Page, username = ADMIN_USER, password = ADMIN_PASS) {
  const res = await page.request.post('/api/auth/login', { data: { username, password } });
  expect(res.ok(), `login falló con ${res.status()}`).toBeTruthy();
  return (await res.json()).data as { token: string; user: unknown };
}

/** Login por el formulario real (`#login-username`), para probar el login en sí. */
export async function loginViaForm(page: Page) {
  await page.goto('/login');
  await page.fill('#login-username', ADMIN_USER);
  await page.fill('#login-password', ADMIN_PASS);
  await page.click('button[type="submit"]');
  await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
}

/** Sesión resuelta y navegación a una ruta interna. */
export async function gotoAuthenticated(page: Page, path: string) {
  await loginViaApi(page);
  await page.goto(path);
  // La app reanuda la sesión con /auth/refresh al montar; sin esta espera las
  // rutas protegidas aún no han comprobado el token.
  await page.waitForTimeout(1500);
}

export { test, expect };
