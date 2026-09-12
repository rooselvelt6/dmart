// k6 load test: FHIR R4 API
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

const fhirFailRate = new Rate('fhir_failures');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 100 },
    { duration: '1m', target: 100 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],
    http_req_failed: ['rate<0.05'],
    fhir_failures: ['rate<0.05'],
  },
};

const BASE_URL = 'http://127.0.0.1:3000';

export default function () {
  // Login
  const loginPayload = JSON.stringify({
    email: 'admin@uci.local',
    password: 'admin123',
  });

  const loginParams = { headers: { 'Content-Type': 'application/json' } };
  const loginRes = http.post(`${BASE_URL}/api/auth/login`, loginPayload, loginParams);
  
  if (loginRes.status !== 200) {
    fhirFailRate.add(true);
    return;
  }

  const token = loginRes.json('access_token');
  const authHeaders = {
    headers: {
      'Authorization': `Bearer ${token}`,
      'Accept': 'application/fhir+json',
    },
  };

  // Test Patient search
  const searchRes = http.get(`${BASE_URL}/fhir/Patient?_count=10`, authHeaders);
  check(searchRes, {
    'search status 200': (r) => r.status === 200,
    'bundle type': (r) => r.json('resourceType') === 'Bundle',
    'has entries': (r) => Array.isArray(r.json('entry')),
  });

  // Test Patient read (if entries exist)
  const bundle = searchRes.json();
  if (bundle.entry && bundle.entry.length > 0) {
    const patientId = bundle.entry[0].resource.id;
    
    // Test Patient read
    const readRes = http.get(`${BASE_URL}/fhir/Patient/${patientId}`, authHeaders);
    check(readRes, {
      'read status 200': (r) => r.status === 200,
      'resource type Patient': (r) => r.json('resourceType') === 'Patient',
    });

    // Test DiagnosticReport
    const drRes = http.get(`${BASE_URL}/fhir/Patient/${patientId}/DiagnosticReport`, authHeaders);
    check(drRes, {
      'diagnosticreport status 200': (r) => r.status === 200,
      'bundle type': (r) => r.json('resourceType') === 'Bundle',
    });

    // Test QR code generation
    const qrRes = http.get(`${BASE_URL}/fhir/Patient/${patientId}/DiagnosticReport/QR`, authHeaders);
    check(qrRes, {
      'qr status 200': (r) => r.status === 200,
      'has svg': (r) => r.json('svg_base64') !== undefined,
      'has png': (r) => r.json('png_base64') !== undefined,
    });
  }

  fhirFailRate.add(false);
  sleep(0.5);
}
