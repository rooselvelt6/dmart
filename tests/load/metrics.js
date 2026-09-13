// k6 load test - endpoint Prometheus /obs/metrics (SPEC-005)
// Objetivo del spec: 1000 req/s -> /obs/metrics responde < 100ms (p95).
// Ejecutar: cargo run --release --bin dmart-server  (en otra terminal)
//           k6 run tests/load/metrics.js
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  scenarios: {
    metrics: {
      executor: 'ramping-vus',
      exec: 'metrics',
      startVUs: 0,
      stages: [
        { duration: '10s', target: 800 },   // ramp-up hacia 800 VUs (~800-1000 rps)
        { duration: '30s', target: 1000 },  // sostiene ~1000 req/s
        { duration: '10s', target: 0 },     // ramp-down
      ],
      gracefulRampDown: '5s',
    },
  },
  thresholds: {
    // SPEC-005: /metrics < 100ms bajo carga
    http_req_duration: ['p(95)<100', 'p(99)<250'],
    http_reqs: ['rate>=900'],               // mantiene el flujo pedido por el spec
    http_req_failed: ['rate<0.01'],
  },
};

const METRICS_URL = 'http://127.0.0.1:3000/obs/metrics';

export function metrics() {
  const res = http.get(METRICS_URL);
  check(res, {
    'metrics 200': (r) => r.status === 200,
    'content-type prometheus': (r) => (r.headers['Content-Type'] || '').includes('text/plain'),
    'contiene http_requests_total': (r) => r.body.includes('http_requests_total'),
    'contiene patients_total': (r) => r.body.includes('patients_total'),
    'contiene ingest_gap_total': (r) => r.body.includes('ingest_gap_total'),
  });
}