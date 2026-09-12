// k6 load test: Escalas clínicas (APACHE II, SOFA, etc.)
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

const scalesFailRate = new Rate('scales_failures');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 100 },
    { duration: '1m', target: 100 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<300'],
    http_req_failed: ['rate<0.05'],
    scales_failures: ['rate<0.05'],
  },
};

const BASE_URL = 'http://127.0.0.1:3000/api';

function randomApacheData() {
  return {
    temperatura: 36.0 + Math.random() * 4.0,
    presion_arterial_media: 60 + Math.random() * 60,
    presion_sistolica: 90 + Math.random() * 100,
    frecuencia_cardiaca: 50 + Math.random() * 120,
    frecuencia_respiratoria: 8 + Math.random() * 30,
    fio2: 0.21 + Math.random() * 0.79,
    pao2: 60 + Math.random() * 400,
    a_ado2: Math.random() * 300,
    spo2: 85 + Math.random() * 15,
    ph_arterial: 7.2 + Math.random() * 0.4,
    sodio_serico: 130 + Math.random() * 20,
    potasio_serico: 3.0 + Math.random() * 2.5,
    creatinina: 0.5 + Math.random() * 4.0,
    hematocrito: 20 + Math.random() * 35,
    leucocitos: 3 + Math.random() * 25,
    gcs_ojos: Math.floor(Math.random() * 4) + 1,
    gcs_verbal: Math.floor(Math.random() * 5) + 1,
    gcs_motor: Math.floor(Math.random() * 6) + 1,
    edad: Math.floor(Math.random() * 50) + 40,
    notas: 'Load test',
  };
}

export default function () {
  // Login first
  const loginPayload = JSON.stringify({
    email: 'admin@uci.local',
    password: 'admin123',
  });

  const loginParams = {
    headers: { 'Content-Type': 'application/json' },
  };

  const loginRes = http.post(`${BASE_URL}/auth/login`, loginPayload, loginParams);
  const loginOk = check(loginRes, { 'login ok': (r) => r.status === 200 });
  
  if (!loginOk) {
    scalesFailRate.add(true);
    return;
  }

  const token = loginRes.json('access_token');
  const authHeaders = {
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
  };

  // Test APACHE II
  const apacheData = randomApacheData();
  const apacheRes = http.post(`${BASE_URL}/scales/apache`, JSON.stringify(apacheData), authHeaders);
  check(apacheRes, {
    'apache status 200': (r) => r.status === 200,
    'has score': (r) => r.json('score') !== undefined && r.json('score') >= 0 && r.json('score') <= 71,
    'has mortality': (r) => r.json('mortalidad_riesgo') !== undefined,
  });

  // Test SOFA
  const sofaRes = http.post(`${BASE_URL}/scales/sofa`, JSON.stringify(apacheData), authHeaders);
  check(sofaRes, {
    'sofa status 200': (r) => r.status === 200,
    'has score': (r) => r.json('score') !== undefined && r.json('score') >= 0 && r.json('score') <= 24,
  });

  // Test NEWS2
  const news2Res = http.post(`${BASE_URL}/scales/news2`, JSON.stringify(apacheData), authHeaders);
  check(news2Res, {
    'news2 status 200': (r) => r.status === 200,
    'has score': (r) => r.json('score') !== undefined && r.json('score') >= 0 && r.json('score') <= 20,
  });

  // Test SAPS III
  const saps3Res = http.post(`${BASE_URL}/scales/saps3`, JSON.stringify(apacheData), authHeaders);
  check(saps3Res, {
    'saps3 status 200': (r) => r.status === 200,
    'has score': (r) => r.json('score') !== undefined && r.json('score') >= 0 && r.json('score') <= 104,
  });

  // Test GCS
  const gcsData = {
    apertura_ocular: apacheData.gcs_ojos,
    respuesta_verbal: apacheData.gcs_verbal,
    respuesta_motora: apacheData.gcs_motor,
    notas: 'Load test',
  };
  const gcsRes = http.post(`${BASE_URL}/scales/gcs`, JSON.stringify(gcsData), authHeaders);
  check(gcsRes, {
    'gcs status 200': (r) => r.status === 200,
    'has score': (r) => r.json('score') !== undefined && r.json('score') >= 3 && r.json('score') <= 15,
  });

  scalesFailRate.add(false);
  sleep(0.2);
}
