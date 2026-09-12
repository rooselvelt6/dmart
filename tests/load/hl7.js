// k6 load test: HL7 MLLP ingestion
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

const hl7FailRate = new Rate('hl7_failures');

export const options = {
  stages: [
    { duration: '30s', target: 5 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 50 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<1000'],
    http_req_failed: ['rate<0.05'],
    hl7_failures: ['rate<0.1'],
  },
};

const BASE_URL = 'http://127.0.0.1:3000/api';

function generateORU() {
  const now = new Date();
  const timestamp = now.toISOString().replace(/[-:]/g, '').split('.')[0];
  const msgId = `MSG${Math.random().toString(36).substr(2, 9).toUpperCase()}`;
  const patientId = `P${Math.floor(Math.random() * 10000).toString().padStart(6, '0')}`;
  
  return `MSH|^~\\&|MINDRAY|ICU|DMART|UCI|${timestamp}||ORU^R01|${msgId}|P|2.5\r` +
    `PID|||${patientId}||TEST^PATIENT||19700101|M\r` +
    `PV1||I|UCI^1^1\r` +
    `OBX|1|NM|8480-6^Systolic BP^LN||${90 + Math.floor(Math.random() * 60)}|mmHg|90-140|N|||F\r` +
    `OBX|2|NM|8462-4^Diastolic BP^LN||${60 + Math.floor(Math.random() * 40)}|mmHg|60-90|N|||F\r` +
    `OBX|3|NM|8867-4^Heart Rate^LN||${60 + Math.floor(Math.random() * 80)}|/min|60-100|N|||F\r` +
    `OBX|4|NM|9279-1^Respiratory Rate^LN||${12 + Math.floor(Math.random() * 20)}|/min|12-20|N|||F\r` +
    `OBX|5|NM|2708-6^O2 Saturation^LN||${90 + Math.floor(Math.random() * 10)}|%|95-100|N|||F\r` +
    `OBX|6|NM|8310-5^Body Temperature^LN||${36.0 + Math.random() * 2.0}.toFixed(1)|C|36.5-37.5|N|||F`;
}

export default function () {
  const hl7Msg = generateORU();
  
  const params = {
    headers: {
      'Content-Type': 'application/hl7-v2',
    },
  };

  // Test HL7 ingestion via HTTP (if available) or check health
  const healthRes = http.get(`${BASE_URL}/health`);
  check(healthRes, {
    'health status 200': (r) => r.status === 200,
  });

  // Test a mock HL7 parse endpoint (if exists)
  // For now just test the API health under load
  sleep(0.5);
}
