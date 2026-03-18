# Documentación de API - dMart UCI

## Endpoints

### Health Check

```http
GET /api/health
```

**Respuesta:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "timestamp": "2026-03-18T06:13:42Z"
}
```

---

### Pacientes

#### Listar pacientes

```http
GET /api/patients
GET /api/patients?q=busqueda
```

**Respuesta:**
```json
{
  "success": true,
  "data": [
    {
      "id": "p001",
      "nombre_completo": "Juan Perez",
      "cedula": "V-12345678",
      "historia_clinica": "HC-001",
      "edad": 61,
      "sexo": "Masculino",
      "fecha_ingreso_uci": "2026-03-18T10:00:00Z",
      "estado_gravedad": "Bajo",
      "ultimo_apache_score": null,
      "ultimo_gcs_score": null
    }
  ],
  "error": null
}
```

#### Obtener paciente

```http
GET /api/patients/{id}
```

**Respuesta:**
```json
{
  "success": true,
  "data": {
    "patient_id": "p001",
    "nombre": "Juan",
    "apellido": "Perez",
    "sexo": "Masculino",
    "cedula": "V-12345678",
    "color_piel": "Tipo1",
    "historia_clinica": "HC-001",
    "nacionalidad": "Venezolano",
    "pais": "Venezuela",
    "estado": "Caracas",
    "ciudad": "Caracas",
    "lugar_nacimiento": "Caracas",
    "direccion": "Av. Principal",
    "fecha_nacimiento": "1965-03-15",
    "familiar_encargado": "Maria Perez",
    "fecha_ingreso_hospital": "2026-03-18T08:00:00Z",
    "fecha_ingreso_uci": "2026-03-18T10:00:00Z",
    "descripcion_ingreso": "Paciente con insuficiencia respiratoria",
    "antecedentes": "Hipertensión, Diabetes tipo 2",
    "resumen_ingreso": "Ingreso por emergencia",
    "diagnostico_hospital": "Neumonía",
    "diagnostico_uci": "Insuficiencia respiratoria aguda",
    "examen_fisico_hospital": "Taquipnea, febril",
    "examen_fisico_uci": "Paciente ventilado",
    "tipo_admision": "Urgente",
    "migracion_otro_centro": false,
    "centro_origen": null,
    "ventilacion_mecanica": true,
    "procesos_invasivos": ["Catéter venoso central", "Sonda nasogástrica"],
    "estado_gravedad": "Critico",
    "ultimo_apache_score": 25,
    "ultimo_gcs_score": 12,
    "created_at": "2026-03-18T10:00:00Z",
    "updated_at": "2026-03-18T12:00:00Z"
  },
  "error": null
}
```

#### Crear paciente

```http
POST /api/patients
Content-Type: application/json

{
  "nombre": "Juan",
  "apellido": "Perez",
  "sexo": "Masculino",
  "cedula": "V-12345678",
  "historia_clinica": "HC-001"
}
```

#### Actualizar paciente

```http
PUT /api/patients/{id}
Content-Type: application/json

{
  "nombre": "Juan",
  "apellido": "Perez",
  "sexo": "Masculino",
  "cedula": "V-12345678",
  "historia_clinica": "HC-001",
  ...
}
```

#### Eliminar paciente

```http
DELETE /api/patients/{id}
```

---

### Mediciones

#### Listar mediciones

```http
GET /api/patients/{id}/measurements
```

**Respuesta:**
```json
{
  "success": true,
  "data": [
    {
      "id": "m001",
      "patient_id": "p001",
      "timestamp": "2026-03-18T12:00:00Z",
      "apache_score": 18,
      "gcs_score": 12,
      "mortality_risk": 25.5,
      "severity": "Moderado",
      "apache_data": {
        "temperatura": 38.5,
        "presion_arterial_media": 75.0,
        "frecuencia_cardiaca": 95.0,
        "frecuencia_respiratoria": 22.0,
        "fio2": 0.4,
        "pao2": 70.0,
        "a_ado2": null,
        "ph_arterial": 7.38,
        "sodio_serico": 138.0,
        "potasio_serico": 4.2,
        "creatinina": 1.2,
        "falla_renal_aguda": false,
        "hematocrito": 38.0,
        "leucocitos": 12.0,
        "gcs_total": 12,
        "edad": 65,
        "insuficiencia_hepatica": false,
        "cardiovascular_severa": true,
        "insuficiencia_respiratoria": true,
        "insuficiencia_renal": false,
        "inmunocomprometido": false,
        "cirugia_no_operado": true
      },
      "gcs_data": {
        "apertura_ocular": 3,
        "respuesta_verbal": 4,
        "respuesta_motora": 5
      },
      "notas": "Paciente estable, sin cambios significativos"
    }
  ],
  "error": null
}
```

#### Obtener última medición

```http
GET /api/patients/{id}/measurements/last
```

#### Crear medición

```http
POST /api/patients/{id}/measurements
Content-Type: application/json

{
  "apache_data": {
    "temperatura": 38.5,
    "presion_arterial_media": 75.0,
    "frecuencia_cardiaca": 95.0,
    "frecuencia_respiratoria": 22.0,
    "fio2": 0.4,
    "pao2": 70.0,
    "a_ado2": null,
    "ph_arterial": 7.38,
    "sodio_serico": 138.0,
    "potasio_serico": 4.2,
    "creatinina": 1.2,
    "falla_renal_aguda": false,
    "hematocrito": 38.0,
    "leucocitos": 12.0,
    "gcs_total": 12,
    "edad": 65,
    "insuficiencia_hepatica": false,
    "cardiovascular_severa": true,
    "insuficiencia_respiratoria": true,
    "insuficiencia_renal": false,
    "inmunocomprometido": false,
    "cirugia_no_operado": true
  },
  "gcs_data": {
    "apertura_ocular": 3,
    "respuesta_verbal": 4,
    "respuesta_motora": 5
  },
  "notas": "Paciente estable"
}
```

---

### Exportación

#### Exportar a CSV

```http
GET /api/patients/{id}/export/csv
```

**Headers:**
- `Content-Type: text/csv`
- `Content-Disposition: attachment; filename="paciente_{id}_mediciones.csv"`

#### Exportar a PDF

```http
GET /api/patients/{id}/export/pdf
```

**Headers:**
- `Content-Type: application/pdf`
- `Content-Disposition: attachment; filename="paciente_{id}_reporte.pdf"`

---

## Códigos de Estado

| Código | Descripción |
|--------|-------------|
| 200 | OK |
| 201 | Creado |
| 400 | Bad Request |
| 404 | No encontrado |
| 500 | Error interno del servidor |

## Formato de Respuesta

Todas las respuestas siguen el formato:

```json
{
  "success": true,
  "data": {...},
  "error": null
}
```

O en caso de error:

```json
{
  "success": false,
  "data": null,
  "error": "Mensaje de error"
}
```
