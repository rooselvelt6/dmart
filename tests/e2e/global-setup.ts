import { request, FullConfig } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './helpers';

/**
 * Estado mínimo que la UI exige para poder registrar un paciente:
 * `register.rs` bloquea el envío si `sin_camas` es true, así que sin camas
 * libres los specs de pacientes y de escalas fallan al pulsar "Registrar".
 *
 * Además, los pacientes que crea la suite NO se egresan nunca, con lo que una
 * segunda ejecución local se queda sin camas (bug intermitente). Aquí se
 * garantiza un mínimo de camas libres antes de correr nada, de forma que la
 * suite es repetible tanto en local como en CI (BD limpia).
 */

const MIN_CAMAS_LIBRES = 12;

function apiBase(config: FullConfig): string {
  const base = process.env.PLAYWRIGHT_BASE_URL || config.projects[0]?.use?.baseURL;
  if (!base) throw new Error('No hay baseURL configurado para Playwright');
  // El servidor estático de `playwright.config.ts` (python http.server :8081/dist)
  // no expone la API: en ese modo el backend seassume en :3000.
  return base.includes('/dist') ? 'http://127.0.0.1:3000' : base;
}

export default async function globalSetup(config: FullConfig) {
  const baseURL = apiBase(config);
  const ctx = await request.newContext({ baseURL });

  try {
    const login = await ctx.post('/api/auth/login', {
      data: { username: ADMIN_USER, password: ADMIN_PASS },
    });
    if (!login.ok()) {
      throw new Error(`login respondió ${login.status()}: ${await login.text()}`);
    }
    const body = await login.json();
    const token = body?.data?.token;
    if (!token) throw new Error('la respuesta de login no trae data.token');

    const headers = { Authorization: `Bearer ${token}` };

    const libres = await ctx.get('/api/admin/camas/disponibles', { headers });
    if (!libres.ok()) {
      throw new Error(`camas/disponibles respondió ${libres.status()}: ${await libres.text()}`);
    }
    const libresData = (await libres.json())?.data ?? [];

    const faltan = MIN_CAMAS_LIBRES - libresData.length;
    if (faltan <= 0) {
      console.log(`\u2705 E2E setup: ${libresData.length} camas libres (mínimo ${MIN_CAMAS_LIBRES})`);
      return;
    }

    const init = await ctx.post('/api/admin/camas/init', {
      headers,
      data: { cantidad: faltan, tipo: 'General' },
    });
    if (!init.ok()) {
      throw new Error(`camas/init respondió ${init.status()}: ${await init.text()}`);
    }
    const creadas = (await init.json())?.data?.length ?? 0;
    console.log(
      `\u2705 E2E setup: ${libresData.length} camas libres + ${creadas} creadas (objetivo ${MIN_CAMAS_LIBRES})`,
    );
  } finally {
    await ctx.dispose();
  }
}