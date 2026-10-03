// k6 load test: Autenticación y endpoints autenticados de lectura
//
// El login lleva anti-brute-force por diseño (429), así que autenticar en cada
// iteración mediría el throttle en vez del servidor. Se entra una vez en
// `setup` y la carga se mide sobre los endpoints que consumen el token.
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

const loginFailRate = new Rate('login_failures');

export const options = {
  stages: [
    { duration: '30s', target: 10 },   // Ramp up
    { duration: '1m', target: 50 },    // Stay at 50 users
    { duration: '30s', target: 100 },  // Ramp up to 100
    { duration: '1m', target: 100 },   // Stay at 100
    { duration: '30s', target: 0 },    // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],
    http_req_failed: ['rate<0.05'],
    login_failures: ['rate<0.05'],
  },
};

const BASE_URL = 'http://127.0.0.1:3000/api';

// El endpoint /auth/login espera `username` (no `email`); el admin que
// siembra `seed_default_admin` se llama `admin`.
const ADMIN_USERNAME = __ENV.ADMIN_USERNAME || 'admin';
const ADMIN_PASSWORD = __ENV.ADMIN_PASSWORD || 'admin123';

export function setup() {
  const res = http.post(
    `${BASE_URL}/auth/login`,
    JSON.stringify({ username: ADMIN_USERNAME, password: ADMIN_PASSWORD }),
    { headers: { 'Content-Type': 'application/json' } },
  );

  const ok = check(res, {
    'login status 200': (r) => r.status === 200,
    'has token': (r) => r.json('data.access_token') !== undefined,
  });
  loginFailRate.add(!ok);

  const token = res.json('data.access_token');
  if (!token) {
    throw new Error(`setup: login falló (HTTP ${res.status}): ${res.body}`);
  }
  return { token };
}

export default function (data) {
  const authHeaders = {
    headers: {
      Authorization: `Bearer ${data.token}`,
      'Content-Type': 'application/json',
    },
  };

  const patientsRes = http.get(`${BASE_URL}/patients`, authHeaders);
  check(patientsRes, {
    'patients status 200': (r) => r.status === 200,
    'patients devuelve items': (r) => Array.isArray(r.json('data.items')),
  });

  const statsRes = http.get(`${BASE_URL}/stats`, authHeaders);
  check(statsRes, {
    'stats status 200': (r) => r.status === 200,
    'stats con totales': (r) => r.json('data.total_pacientes') !== undefined,
    'stats con promedios': (r) => r.json('data.promedios') !== undefined,
  });

  sleep(1);
}
