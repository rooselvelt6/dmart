// k6 load test: Escalas clínicas (APACHE II, SOFA, NEWS2, SAPS III, GCS)
//
// NOTA: cada escala tiene su propio struct de petición (`api::scales`), así
// que los payloads no son intercambiables. Los endpoints son por paciente:
// `POST /api/patients/{id}/scales/{escala}`.
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

const BASE_URL = 'http://127.0.0.1:3000/api';

// El endpoint /auth/login espera `username` (no `email`); el admin que
// siembra `seed_default_admin` se llama `admin`.
const ADMIN_USERNAME = __ENV.ADMIN_USERNAME || 'admin';
const ADMIN_PASSWORD = __ENV.ADMIN_PASSWORD || 'admin123';

const scaleFailRate = new Rate('scales_failures');

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
    scales_failures: ['rate<0.05'],
  },
};

const JSON_HEADERS = { 'Content-Type': 'application/json' };

function authHeaders(token) {
  return {
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
  };
}

function randomApacheData() {
  return {
    temperatura: 36.0 + Math.random() * 5.0,
    presion_arterial_media: 60 + Math.random() * 60,
    presion_sistolica: 90 + Math.random() * 80,
    frecuencia_cardiaca: 60 + Math.random() * 80,
    frecuencia_respiratoria: 12 + Math.random() * 20,
    fio2: 0.21 + Math.random() * 0.5,
    pao2: 60 + Math.random() * 60,
    a_ado2: null,
    spo2: 88 + Math.random() * 12,
    ph_arterial: 7.2 + Math.random() * 0.3,
    sodio_serico: 125 + Math.random() * 20,
    potasio_serico: 3.0 + Math.random() * 2.0,
    creatinina: 0.5 + Math.random() * 3.0,
    falla_renal_aguda: false,
    bilirrubina: 0.5 + Math.random() * 5.0,
    hematocrito: 25 + Math.random() * 20,
    leucocitos: 4 + Math.random() * 20,
    plaquetas: 100 + Math.random() * 250,
    gcs_ojos: 3 + Math.floor(Math.random() * 2),
    gcs_verbal: 4 + Math.floor(Math.random() * 2),
    gcs_motor: 5 + Math.floor(Math.random() * 2),
    gcs_total: 14 + Math.floor(Math.random() * 2),
    edad: 40 + Math.floor(Math.random() * 50),
    insuficiencia_hepatica: false,
    cardiovascular_severa: false,
    insuficiencia_respiratoria: false,
    insuficiencia_renal: false,
    inmunocomprometido: false,
    cirugia_no_operado: false,
    ventilacion_mecanica: false,
    vasopresores: false,
    dosis_vasopresor: 0.0,
    diuresis_diaria: 800 + Math.floor(Math.random() * 1500),
    alerta: false,
    o2_suplementario: false,
    nivel_conciencia: '',
    bicarbonate: 20 + Math.random() * 8,
    tipo_admision: null,
    fuente_admision: null,
    dias_pre_uci: 0,
    infeccion_admision: null,
    sistema_anatomico: null,
  };
}

function sofaData(a) {
  return {
    pao2: a.pao2,
    fio2: a.fio2,
    plaquetas: a.plaquetas,
    bilirrubina: a.bilirrubina,
    presion_arterial_media: a.presion_arterial_media,
    vasopresores: a.vasopresores,
    dosis_vasopresor: a.dosis_vasopresor,
    gcs_total: a.gcs_total,
    creatinina: a.creatinina,
    diuresis_diaria: a.diuresis_diaria,
    notas: 'Load test',
  };
}

function news2Data(a) {
  return {
    frecuencia_respiratoria: a.frecuencia_respiratoria,
    spo2: a.spo2,
    o2_suplementario: a.o2_suplementario,
    presion_sistolica: a.presion_sistolica,
    frecuencia_cardiaca: a.frecuencia_cardiaca,
    temperatura: a.temperatura,
    alerta: a.alerta,
    notas: 'Load test',
  };
}

function saps3Data(a) {
  return {
    edad: a.edad,
    dias_pre_uci: a.dias_pre_uci,
    tipo_admision: a.tipo_admision,
    fuente_admision: a.fuente_admision,
    infeccion_admision: a.infeccion_admision,
    sistema_anatomico: a.sistema_anatomico,
    presion_sistolica: a.presion_sistolica,
    frecuencia_cardiaca: a.frecuencia_cardiaca,
    gcs_total: a.gcs_total,
    bilirrubina: a.bilirrubina,
    creatinina: a.creatinina,
    plaquetas: a.plaquetas,
    ph_arterial: a.ph_arterial,
    ventilacion_mecanica: a.ventilacion_mecanica,
    vasopresores: a.vasopresores,
    notas: 'Load test',
  };
}

function gcsData(a) {
  return {
    apertura_ocular: a.gcs_ojos,
    respuesta_verbal: a.gcs_verbal,
    respuesta_motora: a.gcs_motor,
    notas: 'Load test',
  };
}

// El login lleva anti-brute-force por diseño (429): entrar en cada iteración
// mediría el throttle, no el servidor. Se autentica una vez en `setup` y la
// carga se aplica sobre las escalas.
export function setup() {
  const res = http.post(
    `${BASE_URL}/auth/login`,
    JSON.stringify({ username: ADMIN_USERNAME, password: ADMIN_PASSWORD }),
    { headers: JSON_HEADERS },
  );

  const ok = check(res, {
    'login status 200': (r) => r.status === 200,
    'has token': (r) => r.json('data.access_token') !== undefined,
  });
  scaleFailRate.add(!ok);

  const token = res.json('data.access_token');
  if (!token) {
    throw new Error(`setup: login falló (HTTP ${res.status}): ${res.body}`);
  }

  // Las escalas son por paciente, así que la carga necesita uno.
  const patientId = `LOAD-${Date.now()}`;
  const created = http.post(
    `${BASE_URL}/patients`,
    JSON.stringify({ patient_id: patientId, nombre: 'Carga', apellido: 'Test', sexo: 'Masculino' }),
    { headers: { ...JSON_HEADERS, Authorization: `Bearer ${token}` } },
  );
  check(created, { 'paciente creado 2xx': (r) => r.status === 200 || r.status === 201 });
  if (created.status !== 200 && created.status !== 201) {
    throw new Error(`setup: no se pudo crear el paciente (HTTP ${created.status}): ${created.body}`);
  }

  return { token, patientId };
}

export default function (data) {
  const headers = authHeaders(data.token);
  const base = `${BASE_URL}/patients/${data.patientId}/scales`;
  const a = randomApacheData();

  const apacheRes = http.post(`${base}/apache`, JSON.stringify({ data: a, notas: 'Load test' }), headers);
  check(apacheRes, {
    'apache 2xx': (r) => r.status === 200 || r.status === 201,
    'apache score 0-71': (r) => {
      const s = r.json('data.apache_score');
      return s !== undefined && s >= 0 && s <= 71;
    },
  });

  const sofaRes = http.post(`${base}/sofa`, JSON.stringify(sofaData(a)), headers);
  check(sofaRes, { 'sofa 2xx': (r) => r.status === 200 || r.status === 201 });

  const news2Res = http.post(`${base}/news2`, JSON.stringify(news2Data(a)), headers);
  check(news2Res, { 'news2 2xx': (r) => r.status === 200 || r.status === 201 });

  const saps3Res = http.post(`${base}/saps3`, JSON.stringify(saps3Data(a)), headers);
  check(saps3Res, { 'saps3 2xx': (r) => r.status === 200 || r.status === 201 });

  const gcsRes = http.post(`${base}/gcs`, JSON.stringify(gcsData(a)), headers);
  check(gcsRes, { 'gcs 2xx': (r) => r.status === 200 || r.status === 201 });

  scaleFailRate.add(false);
  sleep(0.2);
}
