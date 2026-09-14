#!/usr/bin/env bash
# SPEC-027 — Coverage thresholds per-module gate
#
# Parsea el reporte LCOV generado por cargo llvm-cov y falla si algún
# módulo protegido cae por debajo de su umbral.
#
# Uso:
#   cargo llvm-cov -p dmart-shared -p dmart-server \
#     --lib --test api_tests --test hl7_integration \
#     --lcov --output-path coverage.lcov
#   bash scripts/check-coverage-thresholds.sh coverage.lcov
#
# Todas las tolerancias están en esta función (punto único de configuración).

set -euo pipefail

# ─── Tabla de umbrales ───────────────────────────────────────────────
# Formato: "patrón_ruta|umbral_porcentaje"
# El patrón debe terminar con la sub-ruta del archivo en el repo.
declare -a THRESHOLDS=(
  "dmart-server/src/hl7/parser.rs|90"
  "dmart-server/src/hl7/mllp.rs|90"
  "dmart-server/src/hl7/ingest.rs|90"
  "dmart-shared/src/scales.rs|85"
  "dmart-shared/src/validation.rs|85"
  "dmart-shared/src/ml.rs|80"
)

LCOV_FILE="${1:-coverage.lcov}"

if [ ! -f "$LCOV_FILE" ]; then
  echo "ERROR: LCOV file not found: $LCOV_FILE" >&2
  exit 1
fi

# Extrae SF/LF/LH triples del LCOV
declare -A FILE_LF FILE_LH
current_sf=""
while IFS= read -r line; do
  case "$line" in
    SF:*) current_sf="${line#SF:}" ;;
    LF:*) FILE_LF["$current_sf"]="${line#LF:}" ;;
    LH:*) FILE_LH["$current_sf"]="${line#LH:}" ;;
  esac
done < "$LCOV_FILE"

failures=0
printf "%-45s %8s  %6s  %6s  %s\n" "MODULE" "COV%" "LH" "LF" "STATUS"
printf "%-45s %8s  %6s  %6s  %s\n" "$(printf '%0.s─' {1..45})" "────────" "──────" "──────" "──────"

for entry in "${THRESHOLDS[@]}"; do
  pat="${entry%%|*}"
  thr="${entry##*|}"

  found=0
  for sf in "${!FILE_LF[@]}"; do
    if [[ "$sf" == *"$pat" ]]; then
      lf="${FILE_LF[$sf]}"
      lh="${FILE_LH[$sf]}"
      found=1

      if [ "$lf" -eq 0 ]; then
        pct="0.0"
      else
        pct=$(awk "BEGIN {printf \"%.1f\", 100*$lh/$lf}")
      fi
      met=$(awk "BEGIN {p=$pct+0; t=$thr+0; print (p >= t) ? \"PASS\" : \"FAIL\"}")

      if [ "$met" = "PASS" ]; then
        printf "%-45s %7s%%  %6s  %6s  ✅ PASS\n" "$pat" "$pct" "$lh" "$lf"
      else
        printf "%-45s %7s%%  %6s  %6s  ❌ FAIL (threshold %s%%)\n" "$pat" "$pct" "$lh" "$lf" "$thr"
        failures=$((failures + 1))
      fi
      break
    fi
  done

  if [ "$found" -eq 0 ]; then
    printf "%-45s %8s  %6s  %6s  ⚠️  NOT FOUND\n" "$pat" "-" "-" "-"
  fi
done

echo ""
if [ "$failures" -gt 0 ]; then
  echo "❌ $failures module(s) below threshold. Coverage gate FAILED."
  exit 1
else
  echo "✅ All modules meet coverage thresholds."
fi
