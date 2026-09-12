// k6 master test runner - runs all load tests sequentially
// Usage: k6 run tests/load/run_all.js

import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  scenarios: {
    auth: {
      executor: 'per-vu-iterations',
      exec: 'auth',
      vus: 1,
      iterations: 1,
      startTime: '0s',
    },
    scales: {
      executor: 'per-vu-iterations',
      exec: 'scales',
      vus: 1,
      iterations: 1,
      startTime: '10s',
    },
    fhir: {
      executor: 'per-vu-iterations',
      exec: 'fhir',
      vus: 1,
      iterations: 1,
      startTime: '20s',
    },
    hl7: {
      executor: 'per-vu-iterations',
      exec: 'hl7',
      vus: 1,
      iterations: 1,
      startTime: '30s',
    },
  },
};

const BASE_URL = 'http://127.0.0.1:3000/api';

function login() {
  const res = http.post(`${BASE_URL}/auth/login`, JSON.stringify({
    email: 'admin@uci.local',
    password: 'admin123',
  }), { headers: { 'Content-Type': 'application/json' } });
  return res.json('access_token');
}

export function auth() {
  const token = login();
  check(token, { 'got token': (t) => t !== undefined });
  sleep(1);
}

function randomData() {
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

export function scales() {
  const token = login();
  if (!token) return;

  const headers = {
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
  };

  const data = randomData();

  // APACHE II
  check(http.post(`${BASE_URL}/scales/apache`, JSON.stringify(data), headers), {
    'apache 200': (r) => r.status === 200,
    'score 0-71': (r) => {
      const s = r.json('score');
      return s !== undefined && s >= 0 && s <= 71;
    },
  });

  // SOFA
  check(http.post(`${BASE_URL}/scales/sofa`, JSON.stringify(data), headers), {
    'sofa 200': (r) => r.status === 200,
    'score 0-24': (r) => {
      const s = r.json('score');
      return s !== undefined && s >= 0 && s <= 24;
    },
  });

  // NEWS2
  check(http.post(`${BASE_URL}/scales/news2`, JSON.stringify(data), headers), {
    'news2 200': (r) => r.status === 200,
    'score 0-20': (r) => {
      const s = r.json('score');
      return s !== undefined && s >= 0 && s <= 20;
    },
  });

  // SAPS III
  check(http.post(`${BASE_URL}/scales/saps3`, JSON.stringify(data), headers), {
    'saps3 200': (r) => r.status === 200,
    'score 0-104': (r) => {
      const s = r.json('score');
      return s !== undefined && s >= 0 && s <= 104;
    },
  });

  // GCS
  const gcsData = {
    apertura_ocular: data.gcs_ojos,
    respuesta_verbal: data.gcs_verbal,
    respuesta_motora: data.gcs_motor,
    notas: 'Load test',
  };
  check(http.post(`${BASE_URL}/scales/gcs`, JSON.stringify(gcsData), headers), {
    'gcs 200': (r) => r.status === 200,
    'score 3-15': (r) => {
      const s = r.json('score');
      return s !== undefined && s >= 3 && s <= 15;
    },
  });

  sleep(0.5);
}

export function fhir() {
  const token = login();
  if (!token) return;

  const headers = {
    headers: {
      'Authorization': `Bearer ${token}`,
      'Accept': 'application/fhir+json',
    },
  };

  // Patient search
  const search = http.get(`http://127.0.0.1:3000/fhir/Patient?_count=5`, headers);
  check(search, {
    'search 200': (r) => r.status === 200,
    'is Bundle': (r) => r.json('resourceType') === 'Bundle',
  });

  const bundle = search.json();
  if (bundle.entry && bundle.entry.length > 0) {
    const pid = bundle.entry[0].resource.id;
    
    // Patient read
    check(http.get(`http://127.0.0.1:3000/fhir/Patient/${pid}`, headers), {
      'read 200': (r) => r.status === 200,
      'is Patient': (r) => r.json('resourceType') === 'Patient',
    });

    // DiagnosticReport
    check(http.get(`http://127.0.0.1:3000/fhir/Patient/${pid}/DiagnosticReport`, headers), {
      'dr 200': (r) => r.status === 200,
      'is Bundle': (r) => r.json('resourceType') === 'Bundle',
    });

    // QR
    check(http.get(`http://127.0.0.1:3000/fhir/Patient/${pid}/DiagnosticReport/QR`, headers), {
      'qr 200': (r) => r.status === 200,
      'has svg': (r) => r.json('svg_base64') !== undefined,
      'has png': (r) => r.json('png_base64') !== undefined,
    });
  }

  sleep(0.5);
}

export function hl7() {
  // Simple health check under load
  check(http.get(`${BASE_URL}/health`), {
    'health 200': (r) => r.status === 200,
    'db ok': (r) => r.json('database') === 'ok',
  });
  sleep(0.5);
}
