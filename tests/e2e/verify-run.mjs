#!/usr/bin/env node
// Gate Playwright: falla si hay specs skipped o sin tests ejecutados.
// Uso: node tests/e2e/verify-run.mjs playwright-results.json

import fs from 'fs';
import path from 'path';

const reportPath = process.argv[2];
if (!reportPath) {
  console.error('Uso: node verify-run.mjs <playwright-results.json>');
  process.exit(1);
}

if (!fs.existsSync(reportPath)) {
  console.error(`❌ No existe el reporte: ${reportPath}`);
  process.exit(1);
}

const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));

/**
 * El reporte JSON anida los specs según el `describe`: el suite de nivel
 * fichero tiene `specs: []` y los specs reales cuelgan de `suites[]`
 * (los `describe`). Recorremos en profundidad, si no el gate cuenta 0 tests
 * y daba por bueno un suite entero sin ejecutar (o lo marcaba fallido siempre).
 */
function collectSpecs(suite) {
  const specs = [...(suite.specs || [])];
  for (const child of suite.suites || []) {
    specs.push(...collectSpecs(child));
  }
  return specs;
}

const suites = report.suites || [];
let totalTests = 0;
let totalPassed = 0;
let totalFailed = 0;
let totalSkipped = 0;
let hasSkippedSpecs = false;
let specsWithoutTests = 0;

console.log('─── Resumen Playwright ───\n');

for (const suite of suites) {
  const file = path.relative(process.cwd(), suite.file);
  const tests = collectSpecs(suite);
  let specPassed = 0;
  let specFailed = 0;
  let specSkipped = 0;

  for (const spec of tests) {
    for (const test of spec.tests || []) {
      totalTests++;
      if (test.results.some(r => r.status === 'passed')) {
        totalPassed++; specPassed++;
      } else if (test.results.some(r => r.status === 'failed')) {
        totalFailed++; specFailed++;
      } else if (test.results.some(r => r.status === 'skipped')) {
        totalSkipped++; specSkipped++;
      }
    }
  }

  if (tests.length === 0) {
    console.log(`⚠️  ${file}: SIN tests definidos`);
    specsWithoutTests++;
  } else if (specSkipped > 0 || specFailed > 0 || specPassed === 0) {
    console.log(`❌ ${file}: ${specPassed} ✓  ${specFailed} ✗  ${specSkipped} ⊘ (SKIPPED/FAIL/NO-RUN)`);
    if (specSkipped > 0) hasSkippedSpecs = true;
    if (specPassed === 0 && specFailed === 0 && specSkipped > 0) hasSkippedSpecs = true;
  } else {
    console.log(`✅ ${file}: ${specPassed} tests OK`);
  }
}

console.log('\n─── Totales ───');
console.log(`Tests: ${totalTests}  |  Passed: ${totalPassed}  |  Failed: ${totalFailed}  |  Skipped: ${totalSkipped}`);
console.log(`Specs sin tests: ${specsWithoutTests}`);

if (hasSkippedSpecs) {
  console.error('\n❌ GATE FALLIDO: hay specs con tests skipped o sin ejecutarse.');
  console.error('   Un "skipped" silencioso es peor que un rojo (PLAN39 P0.1).');
  process.exit(1);
}
if (specsWithoutTests > 0) {
  console.error('\n❌ GATE FALLIDO: hay ficheros .spec.ts sin ningún test.');
  process.exit(1);
}
if (totalTests === 0) {
  console.error('\n❌ GATE FALLIDO: no se ejecutó ningún test.');
  process.exit(1);
}
if (totalFailed > 0) {
  console.error('\n❌ GATE FALLIDO: hay tests fallados.');
  process.exit(1);
}

console.log('\n✅ GATE OK: todos los specs ejecutaron ≥1 test, 0 skipped, 0 failed.');
process.exit(0);