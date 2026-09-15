-- SPEC-016: Clinical Decision Support Rules Engine — PlanDefinition FHIR.
-- Tabla `plan_definition` almacena planes CDS versionados (FHIR PlanDefinition JSONB),
-- sembrados con 3 planes base (Sepsis-3, ARDSnet, Anticoagulación).

DEFINE TABLE IF NOT EXISTS plan_definition SCHEMAFULL;

-- Identificador lógico del plan (ej: "sepsis-3-bundle-v1")
DEFINE FIELD IF NOT EXISTS plan_id ON plan_definition TYPE string;
-- Versión semántica del plan
DEFINE FIELD IF NOT EXISTS version ON plan_definition TYPE string;
-- FHIR PlanDefinition completo (JSONB almacenado como string)
DEFINE FIELD IF NOT EXISTS definition ON plan_definition TYPE string;
-- Si el plan está activo para evaluación
DEFINE FIELD IF NOT EXISTS active ON plan_definition TYPE bool DEFAULT false;
-- Fingerprint SHA-256 de la definición canónica (SPEC-029)
DEFINE FIELD IF NOT EXISTS fingerprint ON plan_definition TYPE string;
-- Metadatos: autor, descripción, tags clínicos
DEFINE FIELD IF NOT EXISTS meta ON plan_definition TYPE string;

-- Índices
DEFINE INDEX IF NOT EXISTS idx_plan_definition_id ON plan_definition COLUMNS plan_id UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_plan_definition_active ON plan_definition COLUMNS active;
DEFINE INDEX IF NOT EXISTS idx_plan_definition_fingerprint ON plan_definition COLUMNS fingerprint UNIQUE;

-- Seed: 3 planes base (Sepsis-3, ARDSnet, Anticoagulación)
CREATE plan_definition CONTENT { plan_id: 'sepsis-3-bundle', version: '1.0.0', definition: '{"name":"Sepsis-3 Bundle (1-hour)","description":"SSC 2021 Sepsis/Septic Shock 1-hour bundle","criteria":[{"expression":"score_news2 >= 5 || (vital_2708_6 < 90 && score_news2 >= 3)"}],"action":[{"id":"sepsis_alert","kind":"alert","title":"Sepsis Alert - Activate 1-hour bundle","description":"Lactate, blood cultures, broad-spectrum ABx, 30ml/kg crystalloid, vasopressors if MAP<65","code":{"system":"http://snomed.info/sct","code":"4525004"}},{"id":"lactate_order","kind":"order","title":"Stat Lactate","description":"Order venous/arterial lactate within 15 min"},{"id":"cultures_order","kind":"order","title":"Blood Cultures x2","description":"Before antibiotics if possible"}]}', active: true, fingerprint: 'e9996b4b917fc4b3e0c9e3ead7bf972f9cbc1d250ab92ce920ca45d18f97d6dc', meta: '{"author":"SSC 2021","tags":["sepsis","bundle","critical"]}' };

CREATE plan_definition CONTENT { plan_id: 'ardsnet-ventilation', version: '1.0.0', definition: '{"name":"ARDSnet Low Tidal Volume Ventilation","description":"ARDSnet protocol: VT 6ml/kg PBW, Pplat <=30, FiO2/PEEP table","criteria":[{"expression":"vital_2708_6 != null && vital_2708_6 < 88 && score_sofa >= 2"}],"action":[{"id":"ards_alert","kind":"alert","title":"ARDS Criteria Met - Initiate Lung-Protective Ventilation","description":"VT 6ml/kg PBW, Pplat <=30 cmH2O, PEEP/FiO2 per ARDSnet table"},{"id":"vt_order","kind":"order","title":"Set Tidal Volume 6ml/kg PBW","description":"Calculate PBW: Male=50+2.3*(height_in-60), Female=45.5+2.3*(height_in-60)"}]}', active: true, fingerprint: '9369e3a41577626df501d9a49157f88c4e9ed7d7c918e2e27c8dc41cb6c07b39', meta: '{"author":"ARDSnet","tags":["ards","ventilation","lung-protective"]}' };

CREATE plan_definition CONTENT { plan_id: 'anticoagulation-icu', version: '1.0.0', definition: '{"name":"ICU Anticoagulation Prophylaxis","description":"Pharmacologic VTE prophylaxis for all ICU patients unless contraindicated","criteria":[{"expression":"true"}],"action":[{"id":"vte_alert","kind":"alert","title":"VTE Prophylaxis Indicated","description":"Enoxaparin 40mg SC q24h (adjust for CrCl) unless active bleeding/HIT/thrombocytopenia"},{"id":"heparin_order","kind":"order","title":"Enoxaparin 40mg SC daily","description":"Renal dose: 20mg if CrCl<30. Contraindicated: platelets<50k, active bleed, HIT"}]}', active: true, fingerprint: '512208cd8ac5cee9ed533ee36613f15100c220649c77e86abf7f73823c1cf109', meta: '{"author":"CHEST 2021","tags":["vte","anticoagulation","prophylaxis"]}' };