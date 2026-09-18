#!/usr/bin/env python3
"""Seed de datos demo — dMart UCI / SAHUAPA.
Crea: institución, 10 camas, personal (3 médicos + 4 enfermeros),
10 pacientes con diagnóstico propio y medición completa de escalas
(APACHE II, GCS, NEWS2, SOFA, SAPS III) vía la API REST.
"""

import json
import os
import re
import sys
import time
import uuid
import urllib.request
import urllib.error

BASE = "http://localhost:3000/api"
ENV = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".env")


def env_val(key):
    try:
        with open(ENV, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line.startswith(key + "="):
                    return line.split("=", 1)[1].strip()
    except FileNotFoundError:
        pass
    return ""


def api(method, path, body=None, token=None):
    url = BASE + path
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        try:
            payload = e.read().decode()
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {"error": str(e)}


def main():
    pw = env_val("DMART_ADMIN_PASSWORD")
    if not pw:
        sys.exit("DMART_ADMIN_PASSWORD vacío en .env")
    code, resp = api("POST", "/auth/login", {"username": "admin", "password": pw})
    if code != 200 or not resp.get("data", {}).get("token"):
        sys.exit(f"Login admin falló ({code}): {resp}")
    token = resp["data"]["token"]
    print("✓ Login admin OK")

    # ── Institución ──────────────────────────────────────────────
    code, cfg = api("GET", "/admin/institucion", token=token)
    nombre = "SAHUAPA HOSPITAL ANTONIO PATRICIO DE ALCALA"
    if code == 200 and cfg.get("data", {}).get("nombre") != nombre:
        c = cfg["data"]
        c.update({
            "nombre": nombre,
            "direccion": "Av. Universidad, Sector Los Bloques, Cumaná",
            "telefono": "+58 293 000 0000",
            "email": "contacto@sahuapa.gob.ve",
            "ciudad": "Cumaná",
            "estado": "Sucre",
            "pais": "Venezuela",
            "zona_horaria": "America/Caracas",
        })
        c.pop("estado", None)
        code, upd = api("PUT", "/admin/institucion", c, token)
        if code != 200:
            sys.exit(f"Fallo al actualizar institución ({code}): {upd}")
        print(f"✓ Institución -> {nombre}")
    else:
        print(f"✓ Institución ya configurada ({cfg.get('data',{}).get('nombre')})")

    # ── Camas: asegurar 10 ───────────────────────────────────────
    def data_items(r):
        return r.get("data", {}).get("items", []) if isinstance(r.get("data"), dict) else r.get("data", [])

    code, camas_r = api("GET", "/admin/camas", token=token)
    camas = [c for c in data_items(camas_r) if c.get("cama_id")]
    if len(camas) < 10:
        falta = 10 - len(camas)
        code, _ = api("POST", "/admin/camas/init", {"cantidad": falta}, token)
    code, camas_r = api("GET", "/admin/camas", token=token)
    camas = sorted([c for c in data_items(camas_r) if c.get("cama_id")], key=lambda c: c["numero"])[:10]
    camas_por_num = {c["numero"]: c for c in camas}
    print(f"✓ Camas: {len(camas_por_num)} válidas (1..{max(camas_por_num) if camas_por_num else 0})")

    # ── Personal ─────────────────────────────────────────────────
    code, staff_r = api("GET", "/admin/staff", token=token)
    existentes = {s.get("username") for s in data_items(staff_r)} if code == 200 else set()
    staff_clave = "Clinica2026!"
    personal = [
        ("med.jperez", "Médico", "Dra. Julia Pérez Mota"),
        ("med.rgarcia", "Médico", "Dr. Ricardo García López"),
        ("med.lmedina", "Médico", "Dra. Luisa Medina Silva"),
        ("enf.atorres", "Enfermero", "Enf. Ana Torres Gil"),
        ("enf.jpulgar", "Enfermero", "Enf. Jorge Pulgar Barrios"),
        ("enf.mquintero", "Enfermero", "Enf. María Quintero Rivas"),
        ("enf.oscar", "Enfermero", "Enf. Oscar Silva Duarte"),
    ]
    creados = 0
    for username, rol, nombre in personal:
        if username in existentes:
            print(f"  · {username} ya existe")
            continue
        code, r = api("POST", "/admin/staff", {
            "username": username, "nombre": nombre,
            "rol": rol, "password": staff_clave,
        }, token)
        if code in (200, 201):
            creados += 1
            print(f"  · {username} ({rol}) creado")
        else:
            print(f"  ✗ {username} error ({code}): {r}")
    print(f"✓ Personal: {creados} nuevos / {len(personal)} total")

    # ── Pacientes ────────────────────────────────────────────────
    pacientes = [
        # (nombre, apellido, sexo, edad, cedula, diagnostico_uci, diagnostico_hospital)
        ("José Alejandro", "Márquez", "Masculino", 68, "V-8501123",
         "Sepsis de origen pulmonar", "Neumonía adquirida en la comunidad (J18.9)"),
        ("María del Carmen", "Rojas", "Femenino", 74, "V-5112345",
         "ACV isquémico extenso", "Enfermedad cerebrovascular isquémica (I63.9)"),
        ("Pedro Luis", "Guzmán", "Masculino", 55, "V-9145567",
         "Infarto agudo de miocardio transmural", "Síndrome coronario agudo (I21.9)"),
        ("Ana Teresa", "Blanco", "Femenino", 61, "V-1223489",
         "Shock séptico de origen abdominal", "Peritonitis aguda (K65.9)"),
        ("Carlos Andrés", "Pereira", "Masculino", 42, "V-1834560",
         "Politraumatismo y TEC severo", "Traumatismo múltiple (T07)"),
        ("Rosa Elena", "Fuentes", "Femenino", 79, "V-4211787",
         "EPOC reagudizada con insuficiencia respiratoria", "EPOC descompensada (J44.1)"),
        ("Luis Alberto", "Cedeño", "Masculino", 48, "V-1550398",
         "Cetoacidosis diabética severa", "Diabetes mellitus tipo 1 descompensada (E10.1)"),
        ("Yolanda Josefina", "Salazar", "Femenino", 66, "V-6339214",
         "Insuficiencia renal aguda oligúrica", "IRA inducida por contraste (N17.9)"),
        ("Ramón Emilio", "Cabrera", "Masculino", 71, "V-4228810",
         "Encefalopatía hepática grado III", "Hepatitis aguda fulminante (K72.0)"),
        ("Carmen Luisa", "Rangel", "Femenino", 35, "V-1776452",
         "Síndrome de Guillain-Barré con falla ventilatoria", "Polirradiculoneuropatía aguda (G61.0)"),
    ]

    # Estado Apache II objetivo por paciente: Bajo/Moderado/Severo/Crítico
    def apache_data(i, edad):
        # Perfiles de gravedad distintos para los 10.
        perfiles = [
            {  # 1 crítico — sepsis
                "temperatura": 38.6, "presion_arterial_media": 58, "presion_sistolica": 88,
                "frecuencia_cardiaca": 138, "frecuencia_respiratoria": 34, "fio2": 0.55,
                "pao2": None, "a_ado2": 395, "spo2": 89, "ph_arterial": 7.27, "sodio_serico": 131,
                "potasio_serico": 3.0, "creatinina": 2.5, "falla_renal_aguda": True, "bilirrubina": 2.4,
                "hematocrito": 26, "leucocitos": 21, "plaquetas": 95, "gcs_ojos": 2, "gcs_verbal": 3,
                "gcs_motor": 5, "gcs_total": 10, "ventilacion_mecanica": True, "vasopresores": True,
                "dosis_vasopresor": 0.25, "diuresis_diaria": 280, "alerta": False, "o2_suplementario": True,
                "bicarbonate": 18, "insuficiencia_respiratoria": True, "cirugia_no_operado": True,
            },
            {  # 2 severo — ACV
                "temperatura": 38.3, "presion_arterial_media": 132, "presion_sistolica": 172,
                "frecuencia_cardiaca": 116, "frecuencia_respiratoria": 26, "fio2": 0.35,
                "pao2": 62, "a_ado2": None, "spo2": 92, "ph_arterial": 7.32, "sodio_serico": 156,
                "potasio_serico": 4.6, "creatinina": 1.4, "falla_renal_aguda": False, "bilirrubina": 1.1,
                "hematocrito": 25, "leucocitos": 16, "plaquetas": 210, "gcs_ojos": 3, "gcs_verbal": 4,
                "gcs_motor": 5, "gcs_total": 12, "ventilacion_mecanica": True, "o2_suplementario": True,
                "bicarbonate": 22,
            },
            {  # 3 severo — IAM
                "temperatura": 37.8, "presion_arterial_media": 70, "presion_sistolica": 95,
                "frecuencia_cardiaca": 122, "frecuencia_respiratoria": 28, "fio2": 0.4,
                "pao2": 70, "a_ado2": None, "spo2": 91, "ph_arterial": 7.30, "sodio_serico": 133,
                "potasio_serico": 3.6, "creatinina": 1.8, "falla_renal_aguda": False, "bilirrubina": 1.2,
                "hematocrito": 29, "leucocitos": 18, "plaquetas": 180, "gcs_ojos": 3, "gcs_verbal": 5,
                "gcs_motor": 5, "gcs_total": 13, "ventilacion_mecanica": False, "cardiovascular_severa": True,
                "citabular": True,
            },
            {  # 4 severo — shock abdominal
                "temperatura": 39.1, "presion_arterial_media": 62, "presion_sistolica": 82,
                "frecuencia_cardiaca": 128, "frecuencia_respiratoria": 30, "fio2": 0.5,
                "pao2": None, "a_ado2": 330, "spo2": 90, "ph_arterial": 7.22, "sodio_serico": 130,
                "potasio_serico": 3.2, "creatinina": 2.2, "falla_renal_aguda": True, "bilirrubina": 3.5,
                "hematocrito": 27, "leucocitos": 23, "plaquetas": 120, "gcs_ojos": 3, "gcs_verbal": 4,
                "gcs_motor": 5, "gcs_total": 12, "ventilacion_mecanica": True, "vasopresores": True,
                "dosis_vasopresor": 0.3, "diuresis_diaria": 200, "alerta": False, "o2_suplementario": True,
                "bicarbonate": 16, "insuficiencia_respiratoria": True, "cirugia_no_operado": True,
            },
            {  # 5 moderado — politrauma
                "temperatura": 37.5, "presion_arterial_media": 84, "presion_sistolica": 110,
                "frecuencia_cardiaca": 104, "frecuencia_respiratoria": 22, "fio2": 0.4,
                "pao2": 78, "a_ado2": None, "spo2": 94, "ph_arterial": 7.35, "sodio_serico": 138,
                "potasio_serico": 4.2, "creatinina": 1.0, "falla_renal_aguda": False, "bilirrubina": 1.0,
                "hematocrito": 32, "leucocitos": 14, "plaquetas": 160, "gcs_ojos": 3, "gcs_verbal": 4,
                "gcs_motor": 5, "gcs_total": 12, "ventilacion_mecanica": False, "vasopresores": True,
                "dosis_vasopresor": 0.12, "bicarbonate": 23,
            },
            {  # 6 moderado — EPOC
                "temperatura": 37.2, "presion_arterial_media": 95, "presion_sistolica": 128,
                "frecuencia_cardiaca": 108, "frecuencia_respiratoria": 25, "fio2": 0.28,
                "pao2": 55, "a_ado2": None, "spo2": 89, "ph_arterial": 7.34, "sodio_serico": 140,
                "potasio_serico": 4.0, "creatinina": 1.1, "falla_renal_aguda": False, "bilirrubina": 0.8,
                "hematocrito": 44, "leucocitos": 12, "plaquetas": 220, "gcs_ojos": 4, "gcs_verbal": 5,
                "gcs_motor": 6, "gcs_total": 15, "ventilacion_mecanica": False, "o2_suplementario": True,
                "insuficiencia_respiratoria": True, "bicarbonate": 26,
            },
            {  # 7 moderado — CAD
                "temperatura": 36.9, "presion_arterial_media": 88, "presion_sistolica": 118,
                "frecuencia_cardiaca": 98, "frecuencia_respiratoria": 24, "fio2": 0.21,
                "pao2": 85, "a_ado2": None, "spo2": 96, "ph_arterial": 7.20, "sodio_serico": 132,
                "potasio_serico": 4.8, "creatinina": 1.3, "falla_renal_aguda": True, "bilirrubina": 0.9,
                "hematocrito": 40, "leucocitos": 13, "plaquetas": 200, "gcs_ojos": 4, "gcs_verbal": 4,
                "gcs_motor": 6, "gcs_total": 14, "ventilacion_mecanica": False, "bicarbonate": 14,
            },
            {  # 8 severo — IRA
                "temperatura": 37.0, "presion_arterial_media": 82, "presion_sistolica": 115,
                "frecuencia_cardiaca": 92, "frecuencia_respiratoria": 20, "fio2": 0.21,
                "pao2": 88, "a_ado2": None, "spo2": 97, "ph_arterial": 7.26, "sodio_serico": 148,
                "potasio_serico": 6.1, "creatinina": 4.2, "falla_renal_aguda": True, "bilirrubina": 1.5,
                "hematocrito": 30, "leucocitos": 11, "plaquetas": 150, "gcs_ojos": 3, "gcs_verbal": 4,
                "gcs_motor": 5, "gcs_total": 12, "ventilacion_mecanica": False, "insuficiencia_renal": True,
                "bicarbonate": 15,
            },
            {  # 9 crítico — hepatitis fulminante
                "temperatura": 37.9, "presion_arterial_media": 64, "presion_sistolica": 90,
                "frecuencia_cardiaca": 130, "frecuencia_respiratoria": 32, "fio2": 0.5,
                "pao2": None, "a_ado2": 350, "spo2": 88, "ph_arterial": 7.29, "sodio_serico": 146,
                "potasio_serico": 3.4, "creatinina": 2.0, "falla_renal_aguda": True, "bilirrubina": 9.2,
                "hematocrito": 24, "leucocitos": 17, "plaquetas": 55, "gcs_ojos": 2, "gcs_verbal": 3,
                "gcs_motor": 4, "gcs_total": 9, "ventilacion_mecanica": True, "insuficiencia_hepatica": True,
                "cirugia_no_operado": True, "alerta": False, "o2_suplementario": True, "bicarbonate": 17,
            },
            {  # 10 moderado — Guillain-Barré
                "temperatura": 36.8, "presion_arterial_media": 92, "presion_sistolica": 124,
                "frecuencia_cardiaca": 88, "frecuencia_respiratoria": 14, "fio2": 0.4,
                "pao2": 72, "a_ado2": None, "spo2": 93, "ph_arterial": 7.37, "sodio_serico": 139,
                "potasio_serico": 4.1, "creatinina": 0.9, "falla_renal_aguda": False, "bilirrubina": 0.7,
                "hematocrito": 38, "leucocitos": 9, "plaquetas": 240, "gcs_ojos": 4, "gcs_verbal": 5,
                "gcs_motor": 6, "gcs_total": 15, "ventilacion_mecanica": True, "o2_suplementario": True,
                "bicarbonate": 24,
            },
        ]
        p = perfiles[i]
        p["edad"] = edad
        all_keys = {
            "temperatura": 37.0, "presion_arterial_media": 93.0, "presion_sistolica": 120.0,
            "frecuencia_cardiaca": 80.0, "frecuencia_respiratoria": 16.0, "fio2": 0.21,
            "pao2": 80.0, "a_ado2": None, "spo2": 98.0, "ph_arterial": 7.40, "sodio_serico": 140.0,
            "potasio_serico": 4.0, "creatinina": 1.0, "falla_renal_aguda": False, "bilirrubina": 0.8,
            "hematocrito": 42.0, "leucocitos": 8.0, "plaquetas": 250.0, "gcs_ojos": 4, "gcs_verbal": 5,
            "gcs_motor": 6, "gcs_total": 15, "edad": edad, "insuficiencia_hepatica": False,
            "cardiovascular_severa": False, "insuficiencia_respiratoria": False, "insuficiencia_renal": False,
            "inmunocomprometido": False, "cirugia_no_operado": False, "ventilacion_mecanica": False,
            "vasopresores": False, "dosis_vasopresor": 0.0, "diuresis_diaria": 1500, "alerta": True,
            "o2_suplementario": False, "nivel_conciencia": "", "bicarbonate": 24.0,
            "tipo_admision": "medical", "fuente_admision": "emergency_room",
            "dias_pre_uci": 1, "infeccion_admision": "none", "sistema_anatomico": "respiratory",
        }
        # corregir 3: quitar clave inválida añadida por error
        p.pop("citabular", None)
        data = dict(all_keys)
        data.update({k: v for k, v in p.items() if k != "edad"})
        data["edad"] = edad
        return data

    creados_p = 0
    for idx, (nombre, apellido, sexo, edad, cedula, diag_uci, diag_hosp) in enumerate(pacientes, start=1):
        cama = camas_por_num.get(idx)
        if not cama:
            print(f"  ✗ No hay cama {idx}")
            continue
        hist_clinica = f"HC-{10000 + idx * 37}"
        paciente = {
            "patient_id": str(uuid.uuid4()),
            "nombre": nombre, "apellido": apellido, "sexo": sexo, "cedula": cedula,
            "color_piel": "Tipo4" if idx % 2 else "Tipo3", "historia_clinica": hist_clinica,
            "nacionalidad": "Venezolano", "pais": "Venezuela", "estado": "Sucre", "ciudad": "Cumaná",
            "lugar_nacimiento": "Cumaná", "direccion": f"Calle {idx}, Cumaná, estado Sucre",
            "fecha_nacimiento": f"{2026 - edad}-01-15", "edad": edad,
            "familiar_encargado": "Familiar de referencia",
            "fecha_ingreso_hospital": "2026-09-16T08:00:00Z",
            "fecha_ingreso_uci": "2026-09-17T10:00:00Z",
            "descripcion_ingreso": f"Ingreso por {diag_hosp.lower()}.",
            "antecedentes": "Antecedentes crónicos controlados.",
            "resumen_ingreso": f"Paciente {sexo.lower()}, {edad} años, con {diag_uci.lower()}.",
            "diagnostico_hospital": diag_hosp, "diagnostico_uci": diag_uci,
            "examen_fisico_hospital": "Signos vitales al ingreso registrados en expediente.",
            "examen_fisico_uci": "Valoración inicial de enfermería y médico de guardia.",
            "tipo_admision": "Urgente", "migracion_otro_centro": False,
            "ventilacion_mecanica": idx in (1, 6, 9, 10),
            "procesos_invasivos": ["CVC"] if idx % 3 == 0 else [],
            "cama_id": cama["cama_id"], "cama_numero": idx,
        }
        code, r = api("POST", "/patients", {**paciente, "equipos_ids": []}, token)
        if code not in (200, 201):
            print(f"  ✗ {nombre} {apellido}: error ({code}): {str(r)[:200]}")
            continue
        pid = r["data"]["patient_id"]
        creados_p += 1
        print(f"  ✓ Paciente {idx}: {nombre} {apellido} (cama {idx})")

        # Medición completa de escalas
        data = apache_data(idx - 1, edad)
        code, res = api("POST", f"/patients/{pid}/scales/apache",
                        {"data": data, "notas": f"Ingreso UCI — perfíl de gravedad {idx}"}, token)
        if code not in (200, 201):
            print(f"  ✗ escala {pid}: ({code}) {str(res)[:200]}")
        else:
            d = res.get("data", {})
            print(f"     APACHE={d.get('apache_score')} GCS={d.get('gcs_score')} "
                  f"NEWS2={d.get('news2_score')} SOFA={d.get('sofa_score')} SAPS3={d.get('saps3_score')} "
                  f"Mort.{round(d.get('mortality_risk', 0), 1)}%")

    print(f"\n✓ {creados_p}/10 pacientes con mediciones")
    print("Credenciales personal: usuario / Clinica2026!")


if __name__ == "__main__":
    main()