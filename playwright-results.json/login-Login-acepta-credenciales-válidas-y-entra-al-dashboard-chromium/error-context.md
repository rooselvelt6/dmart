# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: login.spec.ts >> Login >> acepta credenciales válidas y entra al dashboard
- Location: tests/e2e/login.spec.ts:18:7

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
  1  | import { test, expect } from './helpers';
  2  | import { ADMIN_USER, ADMIN_PASS } from './helpers';
  3  | 
  4  | test.describe('Login', () => {
  5  |   test.beforeEach(async ({ page }) => {
  6  |     await page.goto('/login');
  7  |   });
  8  | 
  9  |   test('muestra el formulario de login real', async ({ page }) => {
  10 |     // El DOM real: #login-username (type=text, no email) y #login-password.
  11 |     await expect(page.locator('#login-username')).toBeVisible();
  12 |     await expect(page.locator('#login-password')).toBeVisible();
  13 |     // Hay dos botones submit (credenciales y MFA); el visible es el de credenciales.
  14 |     await expect(page.locator('form:visible button[type="submit"]')).toBeVisible();
  15 |     await expect(page.locator('#login-username')).toHaveAttribute('type', 'text');
  16 |   });
  17 | 
  18 |   test('acepta credenciales válidas y entra al dashboard', async ({ page }) => {
  19 |     await page.fill('#login-username', ADMIN_USER);
  20 |     await page.fill('#login-password', ADMIN_PASS);
  21 |     await page.click('button[type="submit"]');
  22 | 
  23 |     // El login navega a "/" (DashboardPage), no a /patients: ver login.rs:59.
> 24 |     await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 });
     |                ^ TimeoutError: page.waitForURL: Timeout 15000ms exceeded.
  25 |     await expect(page).not.toHaveURL(/\/login/);
  26 |     await expect(page.locator('[role="alert"]')).toHaveCount(0);
  27 |   });
  28 | 
  29 |   test('muestra error con credenciales inválidas', async ({ page }) => {
  30 |     await page.fill('#login-username', ADMIN_USER);
  31 |     await page.fill('#login-password', 'definitivamente-incorrecta');
  32 |     await page.click('button[type="submit"]');
  33 | 
  34 |     // El servidor devuelve 401 y la app pinta el alert con `login-invalid`.
  35 |     await expect(page.locator('[role="alert"]')).toBeVisible({ timeout: 15000 });
  36 |     await expect(page).toHaveURL(/\/login/);
  37 |   });
  38 | 
  39 |   test('requiere usuario: el submit no pasa el HTML si falta el campo', async ({ page }) => {
  40 |     await page.fill('#login-password', ADMIN_PASS);
  41 |     await page.click('button[type="submit"]');
  42 |     // `required` en el input bloquea el submit nativo: seguimos en /login.
  43 |     await expect(page).toHaveURL(/\/login/);
  44 |     await expect(page.locator('#login-username')).toBeVisible();
  45 |   });
  46 | 
  47 |   test('una ruta protegida sin sesión redirige a /login', async ({ page }) => {
  48 |     await page.goto('/patients');
  49 |     await page.waitForURL(/\/login/, { timeout: 15000 });
  50 |     await expect(page.locator('#login-username')).toBeVisible();
  51 |   });
  52 | });
  53 | 
```