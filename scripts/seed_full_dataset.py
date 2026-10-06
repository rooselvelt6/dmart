#!/usr/bin/env python3
"""Siembra masiva de datos para dMart UCI.

A diferencia de `seed_demo_data.py` (10 pacientes fijos, para demo), este
script genera un dataset grande y realista:

- camas (hasta `--camas` en total)
- personal: médicos, enfermeros, soporte y viewers
- equipos médicos (ventiladores, monitores, bombas, computadores)
- dispositivos / monitores registrados en `/devices`
- pacientes (por defecto 520) con historial de camas y las 5 escalas clínicas

Los pacientes se generan en dos bloques para que el censo final sea
realista: los primeros se ingresan y se egresan (liberando la cama, con
historial de cama. Los ultimos quedan ingresados como censo actual.

Es idempotente por claves naturales: username de personal, serial de
equipos/dispositivos e historia clínica de pacientes. Volver a ejecutarlo
 completa lo que falte sin duplicar.

Uso:
    python3 scripts/seed_full_dataset.py                 # 520 pacientes
    python3 scripts/seed_full_dataset.py --patients 1000 --camas 120
    python3 scripts/seed_full_dataset.py --solo-pacientes --patients 50
"""

import argparse
import json
import os
import random
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone

BASE = os.environ.get("DMART_API_BASE", "http://localhost:3000/api/v1")
ENV_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".env")

# ── Datos maestros ────────────────────────────────────────────────────────

NOMBRES_F = [
    "María", "Ana", "Carmen", "Rosa", "Yolanda", "Luisa", "Isabel", "Pilar",
    "Gabriela", "Verónica", "Beatriz", "Adriana", "Mercedes", "Milagros",
    "Josefina", "Esperanza", "Lourdes", "Maritza", "Nathalie", "Oriana",
    "Roxana", "Yaira", "Zulay", "Ángela", "Betsi", "Damaris", "Solveig",
]
NOMBRES_M = [
    "José", "Pedro", "Carlos", "Luis", "Ramón", "Jorge", "Ricardo", "Oscar",
    "Jesús", "Manuel", "Rafael", "Gustavo", "Iván", "Miguel", "Rubén",
    "Eduardo", "Alfonso", "Germán", "Óscar", "Héctor", "Wilson", "Yeimy",
    "Wilker", "Domingo", "Efraín", "Argenis",
]
APELLIDOS = [
    "Márquez", "Rojas", "Guzmán", "Blanco", "Pereira", "Fuentes", "Cedeño",
    "Salazar", "Cabrera", "Rangel", "Contreras", "Villalobos", "Aponte",
    "Bermúdez", "Colmenares", "Farías", "García", "Hernández", "Jiménez",
    "López", "Medina", "Nieves", "Ortiz", "Paredes", "Quintero", "Ramos",
    "Silva", "Torres", "Urdaneta", "Valera", "Zambrano", "Figueroa", "Gonçalves",
]

# Diagnósticos con su categoría (para pasarlos a la ficha del paciente).
DIAGNOSTICOS = [
    ("Sepsis de origen pulmonar", "Neumonía adquirida en la comunidad (J18.9)", "respiratory", "respiratory"),
    ("Shock séptico abdominal", "Peritonitis aguda (K65.9)", "digestive", "septic"),
    ("Síndrome de.distress respiratorio agudo", "ARDS grave (J80)", "respiratory", "respiratory"),
    ("ACV isquémico extenso", "Enfermedad cerebrovascular isquémica (I63.9)", "neurologic", "neurologic"),
    ("Hemorragia intracraneal", "Hemorragia subaracnoidea (I60.9)", "neurologic", "neurologic"),
    ("Infarto agudo de miocardio", "Síndrome coronario agudo con ST elevado (I21.0)", "cardiovascular", "cardiac"),
    ("Falla cardiaca aguda descompensada", "Insuficiencia cardíaca congestiva (I50.9)", "cardiovascular", "cardiac"),
    ("Insuficiencia renal aguda oligúrica", "Nefritis intersticial aguda (N10)", "renal", "renal"),
    ("Lesión renal aguda por contraste", "Nefropatía por contraste (N14.1)", "renal", "renal"),
    ("Encefalopatía hepática grado III", "Hepatitis fulminante (K72.0)", "hepatic", "hepatic"),
    ("Politraumatismo y TEC severo", "Traumatismo múltiple (T07)", "trauma", "trauma"),
    ("Hemorragia digestiva alta", "Hemorragia de tubo digestivo alto (K92.2)", "digestive", "digestive"),
    ("EPOC reagudizada con insuficiencia respiratoria", "EPOC descompensada (J44.1)", "respiratory", "respiratory"),
    ("Cetoacidosis diabética severa", "Diabetes tipo 1 descompensada (E10.1)", "metabolic", "metabolic"),
    ("Síndrome de Guillain-Barré", "Polirradiculoneuropatía aguda (G61.0)", "neurologic", "neurologic"),
    ("Postoperatorio de cirugía cardíaca", "Estado post bypass aortocoronario (Z95.1)", "cardiovascular", "scheduled_surgical"),
    ("Postoperatorio de colecistectomía", "Estado postoperatorio (Z93.6)", "digestive", "scheduled_surgical"),
    ("Meningitis bacteriana", "Meningitis no pnemocócica (G03.9)", "neurologic", "infection"),
    ("Neutropenia febril", "Neutropenia inducida por quimioterapia (D70.7)", "hematologic", "infection"),
    ("Pancreatitis aguda grave", "Pancreatitis aguda necrosante (K85.9)", "digestive", "digestive"),
]

TIPOS_CAMA = ["General"] * 8 + ["Aislamiento"] * 2 + ["Coronaria"] + ["Pediatrica"]

MARCAS_VENTILADOR = [
    ("Servo U", "Ventilador Mecánico", "Mindray", "Ventilador U"),
    ("Hamilton C6", "Ventilador Mecánico", "Hamilton Medical", "C6"),
    ("PB980", "Ventilador Mecánico", "Nihon Kohden", "PB980"),
    ("LTV 1200", "Ventilador Mecánico", "CareFusion", "LTV 1200"),
]
MARCAS_MONITOR = [
    ("iPM12", "Monitor", "Mindray", "iPM12"),
    ("IntelliVue MX750", "Monitor", "Philips", "MX750"),
    (" BeneHeart H8", "Monitor", "Zhongvu", "H8"),
    ("Dash 4000", "Monitor", "GE Healthcare", "Dash 4000"),
]
MARCAS_BOMBA = [
    ("Alaris 8100", "Bomba de Infusión", "ICU Medical", "Alaris 8100"),
    ("Infuspace L200", "Bomba de Infusión", "B. Braun", "Infuspace L200"),
    ("Omni RF", "Bomba de Infusión", "ICU Medical", "Omni RF"),
]
MARCAS_PC = [
    ("OptiPlex 7010", "Computador", "Dell", "OptiPlex 7010"),
    ("ProDesk 400", "Computador", "HP", "ProDesk 400"),
]
PROVEEDORES = ["Medtronic", "Philips", "Mindray", "Dräger", "GE Healthcare", "B. Braun"]

# ── Utilidades HTTP ───────────────────────────────────────────────────────

_env_cache = {}


def env_val(key):
    if key not in _env_cache:
        val = ""
        try:
            with open(ENV_PATH, encoding="utf-8") as fh:
                for line in fh:
                    line = line.strip()
                    if line.startswith(key + "="):
                        val = line.split("=", 1)[1].strip()
        except FileNotFoundError:
            pass
        _env_cache[key] = val
    return _env_cache[key]


_print_lock = threading.Lock()


def say(msg):
    with _print_lock:
        print(msg, flush=True)


def api(method, path, body=None, token=None, retries=3):
    """Llama a la API reintentando ante 5xx (transient en SurrealKV)."""
    url = BASE + path
    data = json.dumps(body).encode() if body is not None else None
    last = (0, "")
    for attempt in range(retries):
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("Content-Type", "application/json")
        if token:
            req.add_header("Authorization", "Bearer " + token)
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                raw = resp.read().decode()
                try:
                    return resp.status, json.loads(raw)
                except json.JSONDecodeError:
                    return resp.status, {}
        except urllib.error.HTTPError as e:
            raw = e.read().decode()
            try:
                parsed = json.loads(raw)
            except json.JSONDecodeError:
                parsed = raw
            last = (e.code, parsed)
            if e.code < 500:
                return e.code, parsed
            time.sleep(0.4 * (attempt + 1))
        except (urllib.error.URLError, TimeoutError) as e:
            last = (0, str(e))
            time.sleep(0.5 * (attempt + 1))
    return last


def data_items(resp):
    """Normaliza respuestas paginadas o planas."""
    d = resp.get("data")
    if isinstance(d, dict):
        return d.get("items", [])
    return d if isinstance(d, list) else []


def login(user, password):
    code, r = api("POST", "/auth/login", {"username": user, "password": password})
    if code != 200 or not r.get("data", {}).get("access_token"):
        sys.exit(f"No se pudo iniciar sesión como {user} ({code}): {r}")
    return r["data"]["access_token"]


# ── Generadores ───────────────────────────────────────────────────────────

def rand_nombre(rng):
    sexo = "Femenino" if rng.random() < 0.5 else "Masculino"
    nombres = NOMBRES_F if sexo == "Femenino" else NOMBRES_M
    return sexo, rng.choice(nombres), rng.choice(APELLIDOS)


def apache_payload(rng, edad):
    """Perfil fisiológico coherente con la gravedad (Apache II)."""
    # banda de gravedad: (0=Bajo, 1=Moderado, 2=Severo, 3=Critico)
    banda = rng.choices([0, 1, 2, 3], weights=[18, 34, 30, 18])[0]
    v = {
        0: dict(temp=(36.4, 37.4), pam=(80, 100), psis=(110, 140), fc=(62, 88), fr=(12, 19),
                fio2=0.21, pao2=(75, 95), spo2=(95, 99), ph=(7.35, 7.45)),
        1: dict(temp=(37.5, 38.3), pam=(70, 90), psis=(95, 125), fc=(88, 112), fr=(19, 26),
                fio2=0.28, pao2=(62, 82), spo2=(92, 96), ph=(7.30, 7.40)),
        2: dict(temp=(38.0, 39.0), pam=(58, 78), psis=(85, 108), fc=(108, 136), fr=(25, 34),
                fio2=0.40, pao2=(52, 74), spo2=(89, 94), ph=(7.25, 7.36)),
        3: dict(temp=(38.6, 40.0), pam=(42, 62), psis=(68, 90), fc=(130, 168), fr=(32, 44),
                fio2=0.55, pao2=(42, 62), spo2=(85, 91), ph=(7.15, 7.30)),
    }[banda]

    r = lambda lo, hi: round(rng.uniform(lo, hi), 2)
    gcs_total = rng.choice([15, 14, 13, 12, 11, 10, 9, 8, 7])
    gcs_ojos = rng.randint(1, 4)
    gcs_verbal = rng.randint(1, 5)
    gcs_motor = rng.randint(1, 6)
    # El total lo recalcula el servidor; se manda coherente con las partes.
    gcs_total = min(15, max(3, gcs_ojos + gcs_verbal + gcs_motor))
    vm = rng.random() < (0.15 + 0.2 * banda)
    vasopresores = vm and rng.random() < 0.45

    data = {
        "temperatura": r(*v["temp"]),
        "presion_arterial_media": r(*v["pam"]),
        "presion_sistolica": r(*v["psis"]),
        "frecuencia_cardiaca": r(*v["fc"]),
        "frecuencia_respiratoria": r(*v["fr"]),
        "fio2": v["fio2"],
        "pao2": None,
        "a_ado2": None,
        "spo2": r(*v["spo2"]),
        "ph_arterial": r(*v["ph"]),
        "sodio_serico": r(128, 152),
        "potasio_serico": r(2.8, 5.4),
        "creatinina": r(0.6, 3.4) if banda < 3 else r(2.0, 5.0),
        "falla_renal_aguda": rng.random() < 0.12 * banda,
        "bilirrubina": r(0.4, 4.2),
        "hematocrito": r(24, 46),
        "leucocitos": r(5, 26),
        "plaquetas": r(70, 330),
        "gcs_ojos": gcs_ojos,
        "gcs_verbal": gcs_verbal,
        "gcs_motor": gcs_motor,
        "gcs_total": gcs_total,
        "edad": edad,
        "insuficiencia_hepatica": rng.random() < 0.04,
        "cardiovascular_severa": rng.random() < 0.18,
        "insuficiencia_respiratoria": rng.random() < 0.30 + 0.1 * banda,
        "insuficiencia_renal": rng.random() < 0.12,
        "inmunocomprometido": rng.random() < 0.08,
        "cirugia_no_operado": rng.random() < 0.55,
        "ventilacion_mecanica": vm,
        "vasopresores": vasopresores,
        "dosis_vasopresor": r(0.05, 0.45) if vasopresores else 0.0,
        "diuresis_diaria": int(r(80, 1900)),
        "alerta": rng.random() < 0.2,
        "o2_suplementario": rng.random() < 0.45 + 0.12 * banda,
        "nivel_conciencia": rng.choice(["", "Sedado", "Desorientado", "Confuso", "Alerta"]),
        "bicarbonate": r(11, 28),
        "dias_pre_uci": rng.randint(0, 6),
        "tipo_admision": rng.choice(["medical", "medical", "unscheduled_surgical"]),
        "fuente_admision": rng.choice(["emergency_room", "emergency_room", "ward", "other_icu"]),
        "infeccion_admision": rng.choice(["none", "none", "respiratory", "nosocomial"]),
        "sistema_anatomico": "respiratory",
    }
    # SiO2 alto se documenta con a-a DO2, si no con PaO2.
    if data["fio2"] >= 0.5:
        data["a_ado2"] = round(rng.uniform(180, 460), 1)
        data["pao2"] = None
    else:
        data["pao2"] = r(*v["pao2"])
        data["a_ado2"] = None
    return data, gcs_total, banda


def paciente_payload(rng, idx, cama, census_actual):
    """Ficha del paciente con fecha de ingreso coherente con el escenario."""
    sexo, nombre, apellido = rand_nombre(rng)
    edad = rng.randint(19, 94)
    diag_uci, diag_hosp, sistema, origen = rng.choice(DIAGNOSTICOS)
    apache, gcs, banda = apache_payload(rng, edad)

    ahora = datetime.now(timezone.utc)
    if census_actual:
        # Paciente aun en la UCI: ingreso en las ultimas horas/dias
        horas = rng.uniform(2, 96)
        fecha_ingreso = ahora - timedelta(hours=horas)
    else:
        # Paciente ya egresado: ingreso distribuido en los ultimos 6 meses
        dias = rng.uniform(2, 180)
        fecha_ingreso = ahora - timedelta(days=dias)

    ingreso_h = fecha_ingreso - timedelta(hours=rng.uniform(2, 72))
    return {
        "paciente_id": "",
        "patient_id": "",
        "nombre": nombre,
        "apellido": apellido,
        "sexo": sexo,
        "cedula": f"V-{rng.randint(10_000_000, 29_999_999)}",
        "color_piel": rng.choice(["Tipo2", "Tipo3", "Tipo4", "Tipo5"]),
        "historia_clinica": f"HC-{100000 + idx}",
        "nacionalidad": "Venezolano" if rng.random() < 0.88 else "Extranjero",
        "pais": "Venezuela",
        "estado": "Sucre",
        "ciudad": "Cumaná",
        "lugar_nacimiento": rng.choice(["Cumaná", "Caracas", "Barcelona", "Maracay", "Valencia"]),
        "direccion": f"Calle {rng.randint(1, 80)} Cumaná, estado Sucre",
        "fecha_nacimiento": (ahora - timedelta(days=edad * 365 + rng.randint(0, 364))).strftime("%Y-%m-%d"),
        "edad": edad,
        "familiar_encargado": f"{rng.choice(NOMBRES_F + NOMBRES_M)} {apellido}",
        "fecha_ingreso_hospital": ingreso_h.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "fecha_ingreso_uci": fecha_ingreso.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "descripcion_ingreso": f"Ingreso por {diag_hosp.lower()}.",
        "antecedentes": "Antecedentes crónicos controlados.",
        "resumen_ingreso": f"Paciente {sexo.lower()}, {edad} años, con {diag_uci.lower()}.",
        "diagnostico_hospital": diag_hosp,
        "diagnostico_uci": diag_uci,
        "examen_fisico_hospital": "Signos vitales al ingreso registrados en expediente.",
        "examen_fisico_uci": "Valoración inicial de enfermería y médico de guardia.",
        "tipo_admision": rng.choice(["Urgente", "Urgente", "Electiva"]),
        "migracion_otro_centro": rng.random() < 0.08,
        "centro_origen": rng.choice(["Hospital Razmini", "Hospital José María Vargas"]) if True else None,
        "ventilacion_mecanica": apache["ventilacion_mecanica"],
        "procesos_invasivos": (
            ["CVC", "VMI", "Sonda Foley"]
            if apache["ventilacion_mecanica"]
            else rng.choice([["CVC"], ["Sonda Foley"], ["CVC", "Sonda vesical"], []])
        ),
        "cama_id": cama["cama_id"] if cama else None,
        "cama_numero": cama["numero"] if cama else None,
        "equipos_ids": [],
    }, apache, gcs, banda


# ── Bloques de siembra ────────────────────────────────────────────────────

def asegurar_camas(token, objetivo, log):
    code, r = api("GET", "/admin/camas?limit=500", token=token)
    camas = data_items(r)
    faltan = objetivo - len(camas)
    if faltan > 0:
        # init_camas crea de corrido; se parte en trozos de 60 para no castear
        while faltan > 0:
            trozo = min(faltan, 60)
            tipo = rng_global.choice(TIPOS_CAMA)
            code, r = api("POST", "/admin/camas/init", {"cantidad": trozo, "tipo": tipo}, token)
            if code not in (200, 201):
                log(f"  ✗ init camas ({code}): {r}")
                break
            faltan -= trozo
    code, r = api("GET", "/admin/camas?limit=500", token=token)
    camas = sorted(data_items(r), key=lambda c: c.get("numero") or 0)
    log(f"✓ Camas: {len(camas)} (objetivo {objetivo})")
    return camas


def asegurar_personal(token, n_medicos, n_enfermeros, n_soporte, password, log):
    code, r = api("GET", "/admin/staff?limit=500", token=token)
    existentes = {s.get("username") for s in data_items(r)}
    rng = random.Random(20261005)

    plan = []
    for i in range(n_medicos):
        plan.append((f"med.{rng.choice(NOMBRES_M).lower()}{i:03d}", "Medico",
                     f"Dr. {rng.choice(NOMBRES_M)} {rng.choice(APELLIDOS)}"))
    for i in range(n_enfermeros):
        plan.append((f"enf.{rng.choice(NOMBRES_F).lower()}{i:03d}", "Enfermero",
                     f"Enf. {rng.choice(NOMBRES_F)} {rng.choice(APELLIDOS)}"))
    for i in range(n_soporte):
        plan.append((f"sop.{rng.choice(NOMBRES_M).lower()}{i:03d}", "Soporte",
                     f"Ing. {rng.choice(NOMBRES_M)} {rng.choice(APELLIDOS)}"))
    plan.append(("vis.demo", "Viewer", "Observador Demo"))

    creados = 0
    for username, rol, nombre in plan:
        if username in existentes:
            continue
        code, r = api("POST", "/admin/staff", {
            "username": username, "nombre": nombre, "rol": rol, "password": password,
        }, token)
        if code in (200, 201):
            creados += 1
        else:
            log(f"  ✗ staff {username} ({code}): {str(r)[:120]}")
    code, r = api("GET", "/admin/staff?limit=500", token=token)
    total = len(data_items(r))
    log(f"✓ Personal: {creados} nuevos / {total} total "
        f"(~{n_medicos} médicos, ~{n_enfermeros} enfermeros, ~{n_soporte} soporte)")
    return total


def asegurar_equipos(token, objetivo, camas, log):
    rng = random.Random(777)
    code, r = api("GET", "/admin/equipos?limit=1000", token=token)
    actuales = data_items(r)
    seriales = {e.get("serial") for e in actuales}
    log(f"✓ Equipos: {len(actuales)} existentes, objetivo {objetivo}")

    catalogs = MARCAS_VENTILADOR + MARCAS_MONITOR + MARCAS_BOMBA + MARCAS_PC
    tipos_equipo = {
        "Ventilador Mecánico": "VentiladorMecanico",
        "Monitor": "Monitor",
        "Bomba de Infusión": "BombaInfusion",
        "Computador": "Computador",
    }
    estados = ["Activo"] * 12 + ["Mantenimiento", "Inactivo", "Reparacion"]
    creados = 0
    desde = 0
    while len(seriales) < objetivo:
        # Las tuplas del catalogo son (nombre, tipo, marca, modelo).
        nombre_cat, tipo_cat, marca, modelo = catalogs[desde % len(catalogs)]
        desde += 1
        serial = f"SN{rng.randint(10_000_000, 99_999_999)}"
        if serial in seriales:
            continue
        # Reparto por camas: la mayoria se asigna a una cama del catalogo
        cama = rng.choice(camas) if camas and rng.random() < 0.85 else None
        fecha_compra = (datetime.now(timezone.utc) - timedelta(days=rng.randint(30, 2200))).strftime("%Y-%m-%d")
        code, r = api("POST", "/admin/equipos", {
            "equipo_id": "",
            "nombre": nombre_cat,
            "tipo": tipos_equipo[tipo_cat],
            "marca": marca,
            "modelo": modelo,
            "serial": serial,
            "estado": rng.choice(estados),
            "cama_id": cama["cama_id"] if cama else None,
            "proveedor": rng.choice(PROVEEDORES),
            "fecha_compra": fecha_compra,
            "garantia_hasta": (datetime.now(timezone.utc) + timedelta(days=rng.randint(-200, 900))).strftime("%Y-%m-%d"),
            "notas": "",
        }, token)
        if code in (200, 201):
            seriales.add(serial)
            creados += 1
        else:
            log(f"  ✗ equipo {serial} ({code}): {str(r)[:120]}")
            if len(seriales) > objetivo + 50:
                break
    log(f"✓ Equipos: {creados} creados / {len(seriales)} total")
    return len(seriales)


def asegurar_dispositivos(token, objetivo, camas, log):
    rng = random.Random(999)
    code, r = api("GET", "/devices?limit=1000", token=token)
    actuales = data_items(r)
    seriales = {d.get("serial") for d in actuales}
    log(f"✓ Dispositivos: {len(actuales)} existentes, objetivo {objetivo}")

    cat = [
        ("monitor_paciente", "Monitor multiparamétrico", "Mindray", "iPM12"),
        ("monitor_paciente", "Monitor multiparamétrico", "Philips", "IntelliVue MX750"),
        ("ventilador", "Ventilador de UCI", "Hamilton Medical", "C6"),
        ("bomba_infusion", "Bomba de infusión inteligente", "ICU Medical", "Alaris 8100"),
        ("bomba_seringa", "Bomba de jeringa", "B. Braun", "Perfusor"),
        ("analizador", "Analizador gasometría", "Radiometer", "ABL90 FLEX"),
        ("monitor_ecg", "Monitor ECG telemetría", "Philips", "IntelliVue MX550"),
        ("dosificador", "Dosificador de fármacos", "ICU Medical", "Alaris 8100"),
    ]
    estados = ["online"] * 9 + ["offline", "mantenimiento"]
    creados = 0
    while len(seriales) < objetivo:
        tipo, modelo_desc, marca, modelo = rng.choice(cat)
        serial = f"DV{rng.randint(100_000, 999_999)}"
        if serial in seriales:
            continue
        cama = rng.choice(camas) if camas and rng.random() < 0.9 else None
        code, r = api("POST", "/devices", {
            "device_type": tipo,
            "fabricante": marca,
            "modelo": modelo,
            "serial": serial,
            "firmware": f"v{rng.randint(1, 9)}.{rng.randint(0, 9)}.{rng.randint(0, 20)}",
            "cama_id": cama["cama_id"] if cama else None,
            "ubicacion": f"UCI - Cama {cama['numero']}" if cama else "Depósito",
            "estado": rng.choice(estados),
            "heartbeat_interval_secs": rng.choice([15, 30, 60]),
        }, token)
        if code in (200, 201):
            seriales.add(serial)
            creados += 1
        else:
            log(f"  ✗ device {serial} ({code}): {str(r)[:120]}")
            if len(seriales) > objetivo + 50:
                break
    log(f"✓ Dispositivos: {creados} creados / {len(seriales)} total")
    return len(seriales)


def sembrar_pacientes(token, n_pacientes, camas, census, workers, log):
    """Crea pacientes con historial de camas y las 5 escalas clínicas.

    Los primeros `n_pacientes - census` se egresan (liberan cama); los últimos
    `census` quedan ingresados como censo actual.
    """
    rng_global.seed(4242)
    rng = random.Random(4242)
    n_historia = max(0, n_pacientes - census)

    pool_lock = threading.Lock()
    libres = [dict(c) for c in camas if (c.get("estado") == "Libre" or not c.get("estado"))]

    def tomar_cama():
        with pool_lock:
            return libres.pop() if libres else None

    def devolver_cama(cama):
        with pool_lock:
            libres.append(cama)

    # Verificación temprana: si no hay camas libres, amplify sobre la marcha
    # creyendo Patients needs beds. Reutilizamos el endpoint init para eso.
    if not libres:
        log("! No hay camas libres; se crearán 60 camas extra para admitir pacientes")
        for _ in range(2):
            api("POST", "/admin/camas/init", {"cantidad": 60, "tipo": "General"}, token)
        code, r = api("GET", "/admin/camas?limit=800", token=token)
        for c in data_items(r):
            if c.get("estado") == "Libre" or not c.get("estado"):
                libres.append(c)

    stats = {"creados": 0, "fallos": 0, "egresados": 0, "mediciones": 0, "censo": 0, "sin_cama": 0}
    executor = ThreadPoolExecutor(max_workers=workers)
    futuros = []

    def clinica(pid, apache, gcs, idx, notas_extra):
        """Medición completa + escalas individuales para el historial."""
        gcs_data = {
            "apertura_ocular": apache["gcs_ojos"],
            "respuesta_verbal": apache["gcs_verbal"],
            "respuesta_motora": apache["gcs_motor"],
        }
        code, r = api("POST", f"/patients/{pid}/measurements", {
            "apache_data": apache, "gcs_data": gcs_data,
            "notas": notas_extra or "Ingreso UCI — valoración inicial",
        }, token)
        if code in (200, 201):
            with pool_lock:
                stats["mediciones"] += 1

        # Escalas individuales: pobla el histórico de scales del paciente.
        # No todas en todos los pacientes para no inflar el dataset.
        if idx % 2 == 0:
            api("POST", f"/patients/{pid}/scales/news2", {
                "frecuencia_respiratoria": apache["frecuencia_respiratoria"],
                "spo2": apache["spo2"],
                "o2_suplementario": apache["o2_suplementario"],
                "presion_sistolica": apache["presion_sistolica"],
                "frecuencia_cardiaca": apache["frecuencia_cardiaca"],
                "temperatura": apache["temperatura"],
                "alerta": apache["alerta"],
                "notas": "NEWS2 de control",
            }, token)
            api("POST", f"/patients/{pid}/scales/sofa", {
                "pao2": apache["pao2"] or 60,
                "fio2": apache["fio2"],
                "plaquetas": apache["plaquetas"],
                "bilirrubina": apache["bilirrubina"],
                "presion_arterial_media": apache["presion_arterial_media"],
                "vasopresores": apache["vasopresores"],
                "dosis_vasopresor": apache["dosis_vasopresor"],
                "gcs_total": gcs,
                "creatinina": apache["creatinina"],
                "diuresis_diaria": apache["diuresis_diaria"],
                "notas": "SOFA de control",
            }, token)
            api("POST", f"/patients/{pid}/scales/saps3", {
                "edad": apache["edad"],
                "dias_pre_uci": apache["dias_pre_uci"],
                "tipo_admision": apache["tipo_admision"],
                "fuente_admision": apache["fuente_admision"],
                "infeccion_admision": apache["infeccion_admision"],
                "sistema_anatomico": apache["sistema_anatomico"],
                "presion_sistolica": apache["presion_sistolica"],
                "frecuencia_cardiaca": apache["frecuencia_cardiaca"],
                "gcs_total": gcs,
                "bilirrubina": apache["bilirrubina"],
                "creatinina": apache["creatinina"],
                "plaquetas": apache["plaquetas"],
                "ph_arterial": apache["ph_arterial"],
                "ventilacion_mecanica": apache["ventilacion_mecanica"],
                "vasopresores": apache["vasopresores"],
                "notas": "SAPS III de control",
            }, token)

    for idx in range(1, n_pacientes + 1):
        cama = tomar_cama()
        payload, apache, gcs, banda = paciente_payload(rng, idx, cama, census_actual=(idx > n_historia))
        code, r = api("POST", "/patients", payload, token)
        if code not in (200, 201):
            stats["fallos"] += 1
            if cama:
                devolver_cama(cama)
            if stats["fallos"] <= 5:
                log(f"  ✗ paciente {idx} ({code}): {str(r)[:160]}")
            continue
        pid = r["data"].get("patient_id") or r["data"].get("id")
        stats["creados"] += 1

        # Paciente de historia → se egresa después de su valoración clínica
        debe_egresar = idx <= n_historia
        if cama is None:
            stats["sin_cama"] += 1

        def tarea(pid=pid, apache=apache, gcs=gcs, idx=idx, debe_egresar=debe_egresar, cama=cama, banda=banda):
            desenlace = None
            # ~18% de mortalidad, el resto Improve/traslado
            x = rng_global.random()
            if banda >= 2 and x < 0.22:
                desenlace = "Fallecido"
            elif x < 0.30:
                desenlace = "Trasladado"
            else:
                desenlace = "Mejorado"
            clinica(pid, apache, gcs, idx, f"Ingreso UCI — banda de gravedad {banda}")
            if debe_egresar:
                desenlace = desenlace or "Mejorado"
                code, _ = api("POST", f"/patients/{pid}/egreso?desenlace={desenlace}", None, token)
                if code in (200, 201):
                    with pool_lock:
                        stats["egresados"] += 1
                # El egreso libera la cama: hay que devolverla al pool o los
                # pacientes siguientes se quedan sin cama asignada.
                if cama:
                    devolver_cama(cama)
            elif cama:
                with pool_lock:
                    stats["censo"] += 1

        futuros.append(executor.submit(tarea))

        if idx % 50 == 0 or idx == n_pacientes:
            with pool_lock:
                c, f, e, m = stats["creados"], stats["fallos"], stats["egresados"], stats["mediciones"]
                sc = stats["sin_cama"]
            log(f"  · {idx}/{n_pacientes} procesados | creados={c} fallos={f} "
                f"egresados={e} mediciones={m} sin_cama={sc}")

    executor.shutdown(wait=True)
    log(f"✓ Pacientes: {stats['creados']} creados / {stats['fallos']} fallos, "
        f"{stats['egresados']} egresados, {stats['mediciones']} mediciones completas, "
        f"{stats['censo']} en cama (censo actual), {stats['sin_cama']} sin cama")
    return stats


def resumen_final(token, log):
    log("\n── Estado final ──")
    for path, label in [
        ("/admin/camas?limit=500", "camas"),
        ("/admin/equipos?limit=1000", "equipos"),
        ("/admin/staff?limit=500", "personal"),
        ("/devices?limit=1000", "dispositivos"),
        ("/patients?limit=1&estado=todos", "pacientes (todos)"),
        ("/patients?limit=1", "pacientes (activos)"),
    ]:
        code, r = api("GET", path, token=token)
        if path.startswith("/patients"):
            total = r.get("data", {}).get("total")
            log(f"  {label}: {total}")
        else:
            log(f"  {label}: {len(data_items(r))}")


def main():
    global rng_global
    parser = argparse.ArgumentParser(description="Siembra masiva de datos dMart UCI")
    parser.add_argument("--pacientes", type=int, default=520)
    parser.add_argument("--camas", type=int, default=74)
    parser.add_argument("--medicos", type=int, default=24)
    parser.add_argument("--enfermeros", type=int, default=40)
    parser.add_argument("--soporte", type=int, default=3)
    parser.add_argument("--equipos", type=int, default=160)
    parser.add_argument("--dispositivos", type=int, default=90)
    parser.add_argument("--censo", type=int, default=40, help="pacientes que quedan ingresados")
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--password", default=env_val("DMART_STAFF_PASSWORD") or "Clinica2026!")
    parser.add_argument("--user", default=env_val("DMART_ADMIN_USER") or "admin")
    parser.add_argument("--admin-password", default=env_val("DMART_ADMIN_PASSWORD"))
    parser.add_argument("--solo-pacientes", action="store_true",
                        help="solo pacientes (no crear camas/personal/equipos)")
    args = parser.parse_args()

    rng_global = random.Random(31337)

    log = say
    log(f"→ API: {BASE}")
    admin_pwd = args.admin_password or env_val("DMART_ADMIN_PASSWORD")
    if not admin_pwd:
        sys.exit("Falta DMART_ADMIN_PASSWORD (o --admin-password)")
    token = login(args.user, admin_pwd)
    log(f"✓ Sesión iniciada como {args.user}")

    camas = []
    if not args.solo_pacientes:
        camas = asegurar_camas(token, args.camas, log)
        asegurar_personal(token, args.medicos, args.enfermeros, args.soporte, args.password, log)
        asegurar_equipos(token, args.equipos, camas, log)
        asegurar_dispositivos(token, args.dispositivos, camas, log)
    else:
        code, r = api("GET", "/admin/camas?limit=800", token=token)
        camas = data_items(r)

    t0 = time.time()
    sembrar_pacientes(token, args.pacientes, camas, args.censo, args.workers, log)
    log(f"✓ Tiempo total: {time.time() - t0:.0f}s")

    resumen_final(token, log)
    log(f"\nCredenciales personal: usuario med.*/enf.*/sop.* · {args.password}")


if __name__ == "__main__":
    main()
