# APACHE II - Sistema de Clasificación de Gravedad de Enfermedades

## Descripción

El **APACHE II** (Acute Physiology and Chronic Health Evaluation II) es un sistema de puntuación de gravedad ampliamente utilizado en Unidades de Cuidados Intensivos (UCI).

**Referencia original:** Knaus WA, Draper EA, Wagner DP, Zimmerman JE (1985). APACHE II: a severity of disease classification system. Crit Care Med. 13(10):818-29.

---

## Componentes del Score

El score APACHE II tiene un máximo de **71 puntos** distribuidos en:

| Componente | Puntos |
|------------|--------|
| Acute Physiology Score (APS) | 0-60 |
| Puntuación por edad | 0-6 |
| Enf. crónicas severas | 0-5 |

---

## Variables Fisiológicas (12 variables, 0-60 puntos)

### 1. Temperatura Rectal (°C)

| Puntos | Valor |
|--------|-------|
| +4 | ≥41.0 |
| +3 | 39.0-40.9 |
| +1 | 38.5-38.9 |
| 0 | 36.0-38.4 |
| +1 | 34.0-35.9 |
| +2 | 32.0-33.9 |
| +3 | 30.0-31.9 |
| +4 | <30.0 |

### 2. Presión Arterial Media (mmHg)

| Puntos | Valor |
|--------|-------|
| +4 | ≥160 |
| +3 | 130-159 |
| +2 | 110-129 |
| 0 | 70-109 |
| +2 | 50-69 |
| +4 | <50 |

### 3. Frecuencia Cardíaca (lpm)

| Puntos | Valor |
|--------|-------|
| +4 | ≥180 |
| +3 | 140-179 |
| +2 | 110-139 |
| 0 | 70-109 |
| +2 | 55-69 |
| +3 | 40-54 |
| +4 | <40 |

### 4. Frecuencia Respiratoria (rpm)

| Puntos | Valor |
|--------|-------|
| +4 | ≥50 |
| +3 | 35-49 |
| +1 | 25-34 |
| 0 | 12-24 |
| +1 | 10-11 |
| +2 | 6-9 |
| +4 | <6 |

### 5. Oxigenación

**Si FiO2 ≥ 0.5: usar A-aDO2 (mmHg)**

| Puntos | Valor |
|--------|-------|
| +4 | ≥500 |
| +3 | 350-499 |
| +2 | 200-349 |
| 0 | <200 |

**Si FiO2 < 0.5: usar PaO2 (mmHg)**

| Puntos | Valor |
|--------|-------|
| +4 | <55 |
| +3 | 55-60 |
| +1 | 61-70 |
| 0 | >70 |

### 6. pH Arterial

| Puntos | Valor |
|--------|-------|
| +4 | ≥7.70 |
| +3 | 7.60-7.69 |
| +1 | 7.50-7.59 |
| 0 | 7.33-7.49 |
| +2 | 7.25-7.32 |
| +3 | 7.15-7.24 |
| +4 | <7.15 |

### 7. Sodio Sérico (mEq/L)

| Puntos | Valor |
|--------|-------|
| +4 | ≥180 |
| +3 | 160-179 |
| +2 | 155-159 |
| +1 | 150-154 |
| 0 | 130-149 |
| +2 | 120-129 |
| +3 | 111-119 |
| +4 | <111 |

### 8. Potasio Sérico (mEq/L)

| Puntos | Valor |
|--------|-------|
| +4 | ≥7.0 |
| +3 | 6.0-6.9 |
| +1 | 5.5-5.9 |
| 0 | 3.5-5.4 |
| +1 | 3.0-3.4 |
| +2 | 2.5-2.9 |
| +4 | <2.5 |

### 9. Creatinina Sérica (mg/dL)

| Puntos | Valor |
|--------|-------|
| +4 | ≥3.5 |
| +3 | 2.0-3.4 |
| +2 | 1.5-1.9 |
| 0 | 0.6-1.4 |
| +2 | <0.6 |

**Nota:** Si hay falla renal aguda, duplicar los puntos de creatinina.

### 10. Hematocrito (%)

| Puntos | Valor |
|--------|-------|
| +4 | ≥60 |
| +2 | 50-59.9 |
| +1 | 46-49.9 |
| 0 | 30-45.9 |
| +2 | 20-29.9 |
| +4 | <20 |

### 11. Leucocitos (x10³/mm³)

| Puntos | Valor |
|--------|-------|
| +4 | ≥40 |
| +2 | 20-39.9 |
| +1 | 15-19.9 |
| 0 | 3-14.9 |
| +2 | 1-2.9 |
| +4 | <1 |

### 12. Glasgow Coma Scale (GCS)

Puntos = 15 - GCS real

| GCS | Puntos APS |
|-----|------------|
| 15 | 0 |
| 14 | 1 |
| 13 | 2 |
| 12 | 3 |
| 11 | 4 |
| 10 | 5 |
| 9 | 6 |
| 8 | 7 |
| 7 | 8 |
| 6 | 9 |
| 5 | 10 |
| 4 | 11 |
| 3 | 12 |

---

## Puntuación por Edad

| Puntos | Edad |
|--------|------|
| 0 | ≤44 |
| 2 | 45-54 |
| 3 | 55-64 |
| 5 | 65-74 |
| 6 | ≥75 |

---

## Enfermedades Crónicas Severas (5 puntos)

Si el paciente tiene alguna de las siguientes:

1. **Insuficiencia hepática:** Cirrosis documentada, hipertensión portal, antecedentes de sangrado variceal
2. **Enfermedad cardiovascular severa:** Insuficiencia cardíaca clase IV NYHA, angina inestable
3. **Insuficiencia respiratoria severa:** EPOC restrictivo crónico, fibrosis quística, dependencia de VM
4. **Insuficiencia renal crónica:** Diálisis crónica
5. **Inmunocomprometido:** Quimioterapia, radioterapia, transplante, SIDA

**Nota:** Agregar 5 puntos si hay cirugía no operatoria o de emergencia.

---

## Clasificación de Severidad

| Score APACHE II | Mortalidad Estimada | Clasificación |
|-----------------|---------------------|---------------|
| 0-9 | <10% | Bajo |
| 10-19 | 10-25% | Moderado |
| 20-29 | 25-50% | Severo |
| 30+ | >50% | Crítico |

---

## Fórmula de Mortalidad

Según Knaus et al. (1985):

```
ln(R / (1-R)) = -3.517 + (0.083 × APACHE II) + 0.379 × (si cirugía de emergencia)
```

Donde R es el riesgo de mortalidad hospitalaria.

---

## Implementación en dMart

El sistema dMart implementa automáticamente:

- ✅ Cálculo de las 12 variables fisiológicas
- ✅ Puntuación por edad
- ✅ Puntuación por enfermedades crónicas
- ✅ Cálculo de GCS
- ✅ Estimación de riesgo de mortalidad
- ✅ Validación de rangos clínicos
- ✅ Tests de verificación

---

## Limitaciones

1. El APACHE II fue desarrollado en los años 80 y puede no reflejar las prácticas actuales
2. No debe usarse como único criterio para decisiones de triage
3. La validez varía según el tipo de UCI y población
4. Debe complementarse con otros indicadores clínicos
