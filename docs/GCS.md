# Escala de Coma de Glasgow (GCS)

## Descripción

La **Escala de Coma de Glasgow** es una herramienta clínica utilizada para medir el nivel de conciencia de un paciente. Fue desarrollada por Graham Teasdale y Bryan Jennett en 1974 en la Universidad de Glasgow, Escocia.

**Referencia original:** Teasdale GM, Jennett B (1974). Assessment of coma and impaired consciousness. Lancet. 2(7872):81-4.

---

## Componentes

La escala evalúa tres aspectos:

| Variable | Puntuación | Descripción |
|----------|-------------|-------------|
| Apertura Ocular | 1-4 | Respuesta a estímulos visuales |
| Respuesta Verbal | 1-5 | Orientación y comunicación |
| Respuesta Motora | 1-6 | Respuesta a órdenes verbales |

### Apertura Ocular (AO)

| Puntos | Respuesta |
|--------|-----------|
| 4 | Espontánea |
| 3 | A la voz |
| 2 | Al dolor |
| 1 | Ninguna |

### Respuesta Verbal (RV)

| Puntos | Respuesta |
|--------|-----------|
| 5 | Orientado |
| 4 | Confuso |
| 3 | Palabras inapropiadas |
| 2 | Sonidos incomprensibles |
| 1 | Ninguna |

**Nota para pacientes intubados:** Usar "1T" (tubo) = 1 punto

### Respuesta Motora (RM)

| Puntos | Respuesta |
|--------|-----------|
| 6 | Obedece órdenes |
| 5 | Localiza el dolor |
| 4 | Retira al dolor |
| 3 | Flexión anormal (decorticación) |
| 2 | Extensión anormal (descerebración) |
| 1 | Ninguna |

---

## Cálculo del Score

```
GCS Total = AO + RV + RM
```

**Rango:** 3 - 15 puntos

---

## Interpretación

| GCS | Clasificación | Descripción |
|-----|---------------|-------------|
| 15 | Normal | Consciente y orientado |
| 13-14 | Lesión leve | Confusión, desorientación |
| 9-12 | Lesión moderada | Paciente responds to voice |
| 3-8 | Lesión grave / Coma | No despierta con estímulos |

---

## Clasificación por Gravedad

```
GCS 13-15: Lesión LEVE
GCS 9-12: Lesión MODERADA  
GCS 3-8: Lesión GRAVE / COMA
```

---

## Importancia Clínica

### En el contexto APACHE II:

El GCS contribute al **Acute Physiology Score (APS)** con:

```
Puntos GCS = 15 - GCS real
```

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

## Ejemplos Prácticos

### Ejemplo 1: Paciente Normal
- Apertura ocular: Espontánea (4)
- Respuesta verbal: Orientado (5)
- Respuesta motora: Obedece (6)
- **Total: 15** → Normal

### Ejemplo 2: Trauma Craneoencefálico Moderado
- Apertura ocular: A la voz (3)
- Respuesta verbal: Confuso (4)
- Respuesta motora: Localiza (5)
- **Total: 12** → Lesión moderada

### Ejemplo 3: Coma
- Apertura ocular: Ninguna (1)
- Respuesta verbal: Ninguna (1)
- Respuesta motora: Extensión (2)
- **Total: 4** → Lesión grave

---

## Limitaciones

1. **Pacientes intubados:** No se puede evaluar RV → usar "1T"
2. **Pacientes sedados:** No se puede evaluar adecuadamente
3. **Lesiones faciales:** Puede afectar AO y RV
4. **No evalúa:** Reflejos pupilares, funciones autonómicas

---

## Implementación en dMart

El sistema dMart permite:

- ✅ Registro de los 3 componentes del GCS
- ✅ Cálculo automático del total
- ✅ Interpretación clínica automática
- ✅ Integración con APACHE II
- ✅ Tests de validación

---

## Tabla de Referencia Rápida

| GCS | Interpretación | Puntos APACHE II |
|-----|----------------|------------------|
| 15 | Normal | 0 |
| 13-14 | Leve | 1-2 |
| 9-12 | Moderada | 3-6 |
| 3-8 | Grave/Coma | 7-12 |
