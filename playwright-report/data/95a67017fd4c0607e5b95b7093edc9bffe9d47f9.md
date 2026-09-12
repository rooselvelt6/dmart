# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: login.spec.ts >> Login >> should show error with invalid credentials
- Location: tests/e2e/login.spec.ts:24:7

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: page.fill: Test timeout of 30000ms exceeded.
Call log:
  - waiting for locator('input[type="email"]')

```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - heading "Directory listing for /" [level=1] [ref=e2]
  - separator [ref=e3]
  - list [ref=e4]:
    - listitem [ref=e5]:
      - link ".dockerignore" [ref=e6] [cursor=pointer]:
        - /url: .dockerignore
    - listitem [ref=e7]:
      - link ".env.example" [ref=e8] [cursor=pointer]:
        - /url: .env.example
    - listitem [ref=e9]:
      - link ".git/" [ref=e10] [cursor=pointer]:
        - /url: .git/
    - listitem [ref=e11]:
      - link ".github/" [ref=e12] [cursor=pointer]:
        - /url: .github/
    - listitem [ref=e13]:
      - link ".gitignore" [ref=e14] [cursor=pointer]:
        - /url: .gitignore
    - listitem [ref=e15]:
      - link "Caddyfile" [ref=e16] [cursor=pointer]:
        - /url: Caddyfile
    - listitem [ref=e17]:
      - link "Cargo.lock" [ref=e18] [cursor=pointer]:
        - /url: Cargo.lock
    - listitem [ref=e19]:
      - link "Cargo.toml" [ref=e20] [cursor=pointer]:
        - /url: Cargo.toml
    - listitem [ref=e21]:
      - link "CHECKPOINT_FASE5.md" [ref=e22] [cursor=pointer]:
        - /url: CHECKPOINT_FASE5.md
    - listitem [ref=e23]:
      - link "dist/" [ref=e24] [cursor=pointer]:
        - /url: dist/
    - listitem [ref=e25]:
      - link "dmart-app/" [ref=e26] [cursor=pointer]:
        - /url: dmart-app/
    - listitem [ref=e27]:
      - link "dmart-server/" [ref=e28] [cursor=pointer]:
        - /url: dmart-server/
    - listitem [ref=e29]:
      - link "dmart-shared/" [ref=e30] [cursor=pointer]:
        - /url: dmart-shared/
    - listitem [ref=e31]:
      - link "docker-compose.prod.yml" [ref=e32] [cursor=pointer]:
        - /url: docker-compose.prod.yml
    - listitem [ref=e33]:
      - link "docker-compose.yml" [ref=e34] [cursor=pointer]:
        - /url: docker-compose.yml
    - listitem [ref=e35]:
      - link "Dockerfile" [ref=e36] [cursor=pointer]:
        - /url: Dockerfile
    - listitem [ref=e37]:
      - link "Dockerfile.dev" [ref=e38] [cursor=pointer]:
        - /url: Dockerfile.dev
    - listitem [ref=e39]:
      - link "docs/" [ref=e40] [cursor=pointer]:
        - /url: docs/
    - listitem [ref=e41]:
      - link "keep-alive.sh" [ref=e42] [cursor=pointer]:
        - /url: keep-alive.sh
    - listitem [ref=e43]:
      - link "node_modules/" [ref=e44] [cursor=pointer]:
        - /url: node_modules/
    - listitem [ref=e45]:
      - link "package-lock.json" [ref=e46] [cursor=pointer]:
        - /url: package-lock.json
    - listitem [ref=e47]:
      - link "package.json" [ref=e48] [cursor=pointer]:
        - /url: package.json
    - listitem [ref=e49]:
      - link "playwright-report/" [ref=e50] [cursor=pointer]:
        - /url: playwright-report/
    - listitem [ref=e51]:
      - link "playwright.config.ts" [ref=e52] [cursor=pointer]:
        - /url: playwright.config.ts
    - listitem [ref=e53]:
      - link "README.md" [ref=e54] [cursor=pointer]:
        - /url: README.md
    - listitem [ref=e55]:
      - link "ROADMAP.md" [ref=e56] [cursor=pointer]:
        - /url: ROADMAP.md
    - listitem [ref=e57]:
      - link "rust-toolchain.toml" [ref=e58] [cursor=pointer]:
        - /url: rust-toolchain.toml
    - listitem [ref=e59]:
      - link "scripts/" [ref=e60] [cursor=pointer]:
        - /url: scripts/
    - listitem [ref=e61]:
      - link "start-server.sh" [ref=e62] [cursor=pointer]:
        - /url: start-server.sh
    - listitem [ref=e63]:
      - link "target/" [ref=e64] [cursor=pointer]:
        - /url: target/
    - listitem [ref=e65]:
      - link "test-results/" [ref=e66] [cursor=pointer]:
        - /url: test-results/
    - listitem [ref=e67]:
      - link "tests/" [ref=e68] [cursor=pointer]:
        - /url: tests/
  - separator [ref=e69]
```

# Test source

```ts
  1  | import { test, expect } from '@playwright/test';
  2  | 
  3  | test.describe('Login', () => {
  4  |   test.beforeEach(async ({ page }) => {
  5  |     await page.goto('/');
  6  |   });
  7  | 
  8  |   test('should show login page', async ({ page }) => {
  9  |     await expect(page.locator('h1')).toContainText('dMart UCI');
  10 |     await expect(page.locator('input[type="email"]')).toBeVisible();
  11 |     await expect(page.locator('input[type="password"]')).toBeVisible();
  12 |     await expect(page.locator('button[type="submit"]')).toBeVisible();
  13 |   });
  14 | 
  15 |   test('should login with valid credentials', async ({ page }) => {
  16 |     await page.fill('input[type="email"]', 'admin@uci.local');
  17 |     await page.fill('input[type="password"]', 'admin123');
  18 |     await page.click('button[type="submit"]');
  19 |     
  20 |     // Should redirect to dashboard/patients
  21 |     await expect(page.locator('h1')).toContainText(/Pacientes|Dashboard/i);
  22 |   });
  23 | 
  24 |   test('should show error with invalid credentials', async ({ page }) => {
> 25 |     await page.fill('input[type="email"]', 'wrong@uci.local');
     |                ^ Error: page.fill: Test timeout of 30000ms exceeded.
  26 |     await page.fill('input[type="password"]', 'wrong');
  27 |     await page.click('button[type="submit"]');
  28 |     
  29 |     await expect(page.locator('.toast, .alert, [role="alert"]')).toBeVisible();
  30 |   });
  31 | });
  32 | 
```