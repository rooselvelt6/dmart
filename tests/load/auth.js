// k6 load test: Autenticación y login
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

export default function () {
  // Test login endpoint
  const loginPayload = JSON.stringify({
    email: 'admin@uci.local',
    password: 'admin123',
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const loginRes = http.post(`${BASE_URL}/auth/login`, loginPayload, params);
  
  const loginOk = check(loginRes, {
    'login status 200': (r) => r.status === 200,
    'has token': (r) => r.json('access_token') !== undefined,
    'response time < 500ms': (r) => r.timings.duration < 500,
  });

  loginFailRate.add(!loginOk);

  if (loginOk) {
    const token = loginRes.json('access_token');
    
    // Test authenticated endpoints
    const authHeaders = {
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json',
      },
    };

    // Test /patients
    const patientsRes = http.get(`${BASE_URL}/patients`, authHeaders);
    check(patientsRes, {
      'patients status 200': (r) => r.status === 200,
      'patients array': (r) => Array.isArray(r.json()),
    });

    // Test /patients/stats
    const statsRes = http.get(`${BASE_URL}/patients/stats`, authHeaders);
    check(statsRes, {
      'stats status 200': (r) => r.status === 200,
      'has kpis': (r) => r.json('egresados') !== undefined,
    });
  }

  sleep(1);
}
