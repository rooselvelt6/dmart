# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: measurements.spec.ts >> Mediciones y Escalas >> muestra chips de escalas en el detalle del paciente
- Location: tests/e2e/measurements.spec.ts:33:7

# Error details

```
TimeoutError: page.waitForURL: Timeout 15000ms exceeded.
=========================== logs ===========================
waiting for navigation until "load"
============================================================
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - main [ref=e3]:
    - generic [ref=e7]:
      - generic [ref=e8]:
        - generic [ref=e9]: D
        - heading "DMART" [level=1] [ref=e11]
      - alert [ref=e12]: Invalid username or password
      - generic [ref=e13]:
        - generic [ref=e14]:
          - generic [ref=e15]: Username
          - textbox "Username" [ref=e16]:
            - /placeholder: admin_uci
            - text: admin
        - generic [ref=e17]:
          - generic [ref=e18]:
            - generic [ref=e19]: Password
            - link "Forgot password?" [ref=e20] [cursor=pointer]:
              - /url: "#"
          - textbox "Password" [ref=e21]:
            - /placeholder: ••••••••
            - text: testadmin123
        - button "Sign In" [ref=e22] [cursor=pointer]
      - paragraph [ref=e24]: V.1.9 — Restricted Access
  - status
```

# Test source

```ts
  1  | import { test, expect, Page } from '@playwright/test';
  2  | 
  3  | /**
  4  |  * Helpers compartidos por los specs E2E.
  5  |  *
  6  |  * Estos selectores reflejan el DOM real de `dmart-app`. La versión anterior de
  7  |  * los specs apuntaba a una UI que nunca existió (`input[type="email"]`, rutas
  8  |  * `/pacientes`, credenciales `admin@uci.local`), así que fallaban sin llegar a
  9  |  * ejecutar nada.
  10 |  *
  11 |  * La sesión NO se puede falsificar por `localStorage`: el access token vive
  12 |  * solo en memoria WASM y la cookie de refresco es httpOnly. El login se hace
  13 |  * por el formulario real para que Playwright capture el `Set-Cookie`.
  14 |  *
  15 |  * IMPORTANTE: La navegación directa con `page.goto()` a rutas protegidas NO
  16 |  * funciona bien porque el router verifica `is_auth` antes de que la sesión
  17 |  * se restaure vía `/auth/refresh`. La navegación debe ser client-side
  18 |  * (click en enlaces del sidebar) DESPUÉS del login inicial.
  19 |  */
  20 | 
  21 | export const ADMIN_USER = process.env.ADMIN_USERNAME || 'admin';
  22 | export const ADMIN_PASS = process.env.ADMIN_PASSWORD || 'E2eTest!2026';
  23 | 
  24 | /** Login por el formulario real (`#login-username` type=text). */
  25 | export async function loginViaForm(page: Page) {
  26 |   await page.goto('/login');
  27 |   await page.fill('#login-username', ADMIN_USER);
  28 |   await page.fill('#login-password', ADMIN_PASS);
  29 |   await page.click('button[type="submit"]');
> 30 |   await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
     |              ^ TimeoutError: page.waitForURL: Timeout 15000ms exceeded.
  31 |   // Esperar a que la app hidrate y la sesión esté lista en el dashboard
  32 |   await page.waitForTimeout(2000);
  33 | }
  34 | 
  35 | /**
  36 |  * Navegación client-side vía sidebar (funciona porque la sesión ya está lista).
  37 |  * Requiere estar en una página con sidebar visible (ej. tras login estamos en /).
  38 |  */
  39 | export async function navigateSidebar(page: Page, href: string) {
  40 |   // El sidebar puede estar colapsado en móvil; forzar apertura si hace falta
  41 |   const sidebarLink = page.locator(`nav a[href="${href}"]`);
  42 |   if (await sidebarLink.count() === 0) {
  43 |     // Sidebar colapsado: abrir menú móvil
  44 |     await page.click('button[aria-label="Abrir menú de navegación"]');
  45 |   }
  46 |   await sidebarLink.click();
  47 |   await page.waitForURL(href, { timeout: 15000 });
  48 |   await page.waitForTimeout(1000); // hidratación de la nueva vista
  49 | }
  50 | 
  51 | /**
  52 |  * Flujo completo: login + navegación client-side a una ruta interna.
  53 |  * Úsalo en `beforeEach` de los specs.
  54 |  */
  55 | export async function gotoAuthenticated(page: Page, path: string) {
  56 |   await loginViaForm(page);
  57 |   if (path !== '/') {
  58 |     await navigateSidebar(page, path);
  59 |   }
  60 | }
  61 | 
  62 | export { test, expect };
```