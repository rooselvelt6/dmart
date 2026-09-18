#!/usr/bin/env node
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const MD = join(ROOT, 'Informe_Final_Tecnico_dMart_UCI.md');
const TMPL = join(ROOT, 'docs/templates/plantilla_informe_tecnico.html');
const OUT = join(ROOT, 'build');

const esc = (s) =>
  String(s).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

function inline(src) {
  let s = src.replace(/\\\(\^\{([^}]*)\}\\\)/g, '⟦SUP:$1⟧');
  s = s.replace(/\\\(/g, '').replace(/\\\)/g, '');
  const codes = [];
  s = s.replace(/`([^`]+)`/g, (_, c) => {
    codes.push(c);
    return `⟦CODE${codes.length - 1}⟧`;
  });
  s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(?<!\*)\*([^*\n]+)\*(?!\*)/g, '<em>$1</em>');
  s = s.replace(/(?<!_)_([^_\n]+)_(?!_)/g, '<em>$1</em>');
  s = s.replace(/⟦SUP:([^⟧]+)⟧/g, '<sup>$1</sup>');
  s = s.replace(/⟦CODE(\d+)⟧/g, (_, i) => `<code>${esc(codes[Number(i)])}</code>`);
  return s;
}

function parseYaml(block) {
  const lines = block.split('\n');
  const out = { title: '', author: '', date: '' };
  let key = null, mode = null, acc = [];
  const flush = () => {
    if (!key) return;
    let v = acc.join('\n').replace(/^\n+|\n+$/g, '');
    v = v.split('\n').filter((l) => l.trim() === '').length === 0 ? v.replace(/\n/g, ' ') : v;
    if (key === 'title') {
      out.title = v.split('\n').join(' ');
    } else if (key === 'author') {
      out.author = v;
    } else {
      out[key] = v;
    }
  };
  for (const raw of lines) {
    const line = raw.trimEnd();
    const m = line.match(/^([\w-]+):\s*$/);
    if (m) {
      flush();
      key = m[1];
      const placeholder = out[key] ?? '';
      void placeholder;
      mode = 'next';
      acc = [];
      continue;
    }
    const m2 = line.match(/^([\w-]+):\s*(.+)$/);
    if (m2) {
      flush();
      key = m2[1];
      out[key] = m2[2].trim();
      mode = 'plain';
      acc = [];
      continue;
    }
    if (key && mode === 'next' && (line.startsWith(' ') || line === '')) {
      mode = line.startsWith(' ') ? 'collect' : mode;
      acc.push(line.replace(/^ {2,}/, ''));
    } else if (key && mode === 'collect' && line.startsWith(' ')) {
      acc.push(line.replace(/^ {2,}/, ''));
    } else if (key && mode === 'collect' && line === '') {
      acc.push('');
    } else {
      flush();
      key = null;
      mode = 'plain';
      acc = [];
    }
  }
  flush();
  return out;
}

function renderAuthor(raw) {
  let s = String(raw)
    .replace(/\\\(\^\{([^}]*)\}\\\)/g, '<sup>$1</sup>')
    .replace(/\\[()]/g, '')
    .replace(/^(\d+)(?=[A-ZÁÉÍÓÚÑ])/gm, '');
  const paras = s.split('\n').map((l) => l.trim()).filter(Boolean);
  if (!paras.length) return '';
  const names = paras[0].replace(/\s+/g, ' ');
  const affs = paras.slice(1);
  let html = `<p class="authors"><strong>${names}</strong><br>`;
  html += affs.map((a) => a.replace(/^(\d+)/, '<sup>$1</sup>')).join('<br>');
  html += '</p>';
  return html;
}

function isTableLine(l) { return l.trimStart().startsWith('|'); }
function isBullet(l) { return /^\s*- /.test(l); }
function isOrdered(l) { return /^\s*\d+\.\s+/.test(l); }
function isHeading(l) { return /^(#{1,6})\s+/.test(l); }
function isFence(l) { return /^```\w*$/.test(l.trim()); }
function isHr(l) { return /^---+$/.test(l.trim()) || /^\*\*\*+$/.test(l.trim()); }
function isBlank(l) { return l.trim() === ''; }

function splitTableLine(l) {
  let t = l.trim();
  if (t.startsWith('|')) t = t.slice(1);
  if (t.endsWith('|') && !t.endsWith('\\|')) t = t.slice(0, -1);
  return t.split('|').map((c) => c.trim());
}

function isSepRow(cells) {
  return cells.every((c) => /^:?-{2,}:?$/.test(c.replace(/\s/g, '')));
}

function parseDocument(src) {
  const lines = src.split('\n');
  const yaml = {};
  let body = lines;
  if (lines[0] && lines[0].trim() === '---') {
    let end = 1;
    while (end < lines.length && lines[end].trim() !== '---') end++;
    yaml._raw = parseYaml(lines.slice(1, end).join('\n'));
    body = lines.slice(end + 1);
  }
  return { meta: yaml._raw || {}, body };
}

function collect(blocks, out) {
  out.push(...blocks);
}

function toBlocks(bodyLines) {
  const blocks = [];
  let i = 0;
  while (i < bodyLines.length) {
    const line = bodyLines[i];
    if (isBlank(line)) { i++; continue; }

    if (isFence(line)) {
      const lang = line.trim().slice(3);
      const buf = [];
      i++;
      while (i < bodyLines.length && !isFence(bodyLines[i])) { buf.push(bodyLines[i]); i++; }
      i++;
      blocks.push({ type: 'code', lang, text: buf.join('\n') });
      continue;
    }

    if (isHeading(line)) {
      const m = line.match(/^(#{1,6})\s+(.*)$/);
      blocks.push({ type: 'h' + m[1].length, text: m[2] });
      i++;
      continue;
    }

    if (isHr(line)) {
      blocks.push({ type: 'hr' });
      i++;
      continue;
    }

    if (isTableLine(line)) {
      const rows = [];
      while (i < bodyLines.length && isTableLine(bodyLines[i])) { rows.push(splitTableLine(bodyLines[i])); i++; }
      if (rows.length >= 2 && isSepRow(rows[1])) {
        blocks.push({ type: 'table', header: rows[0], rows: rows.slice(2) });
      } else {
        blocks.push({ type: 'table', header: rows[0], rows: rows.slice(1) });
      }
      continue;
    }

    if (isBullet(line) || isOrdered(line)) {
      const ordered = isOrdered(line);
      const items = [];
      let cur = { text: '', sub: [] };
      while (i < bodyLines.length) {
        const l = bodyLines[i];
        if (isBlank(l)) break;
        if (ordered ? isOrdered(l) : isBullet(l)) {
          const m = ordered ? l.match(/^\s*\d+\.\s+(.*)$/) : l.match(/^\s*-\s+(.*)$/);
          cur = { text: m[1], sub: [] };
          items.push(cur);
          i++;
        } else if (l.startsWith('   ') || l.startsWith('\t') || l.startsWith('  ')) {
          if (items.length) items[items.length - 1].sub.push(l.trim());
          i++;
        } else {
          break;
        }
      }
      blocks.push({ type: ordered ? 'ol' : 'ul', items });
      continue;
    }

    const para = [];
    while (i < bodyLines.length) {
      const l = bodyLines[i];
      if (isBlank(l) || isHeading(l) || isFence(l) || isHr(l) || isTableLine(l) || isBullet(l) || isOrdered(l)) break;
      para.push(l.trim());
      i++;
    }
    if (para.length) blocks.push({ type: 'p', text: para.filter(Boolean).join(' ') });
  }
  return blocks;
}

function renderTableCaption(blocks, idx, capType) {
  void idx; void capType;
}

function renderHtmlPdf(blocks, meta, css, coverHtml) {
  const tocItems = [];
  let secId = 0;
  const body = [];
  let openSection = false;
  const closeSection = () => {
    if (openSection) { body.push('</section>'); openSection = false; }
  };

  for (const b of blocks) {
    if (b.type === 'h1') {
      closeSection();
      secId++;
      body.push(`<section class="page-break">`);
      body.push(`<h1 id="s${secId}">${inline(b.text)}</h1>`);
      openSection = true;
      tocItems.push({ lvl: 1, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h2') {
      secId++;
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<h2 id="s${secId}">${inline(b.text)}</h2>`);
      tocItems.push({ lvl: 2, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h3') {
      secId++;
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<h3 id="s${secId}">${inline(b.text)}</h3>`);
      tocItems.push({ lvl: 3, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h4') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<h4>${inline(b.text)}</h4>`);
    } else if (b.type === 'h5' || b.type === 'h6') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<h5>${inline(b.text)}</h5>`);
    } else if (b.type === 'p') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<p>${inline(b.text)}</p>`);
    } else if (b.type === 'hr') {
      if (openSection) closeSection();
    } else if (b.type === 'code') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      body.push(`<pre class="code-block"><code>${esc(b.text)}</code></pre>`);
    } else if (b.type === 'table') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      let t = '<table class="keep"><thead><tr>';
      t += b.header.map((c) => `<th>${inline(c)}</th>`).join('');
      t += '</tr></thead><tbody>';
      for (const r of b.rows) {
        t += '<tr>' + r.map((c) => `<td>${inline(c)}</td>`).join('') + '</tr>';
      }
      t += '</tbody></table>';
      body.push(t);
    } else if (b.type === 'ul' || b.type === 'ol') {
      if (!openSection) { body.push('<section>'); openSection = true; }
      const tag = b.type;
      let l = `<${tag}>`;
      for (const it of b.items) {
        l += `<li>${inline(it.text)}`;
        if (it.sub.length) l += '<ul><li>' + it.sub.map(inline).join('</li><li>') + '</li></ul>';
        l += '</li>';
      }
      l += `</${tag}>`;
      body.push(l);
    }
  }
  closeSection();

  let toc = '';
  if (tocItems.length) {
    toc = '<div class="toc"><ol>';
    for (const it of tocItems) {
      toc += `<li class="l${it.lvl}"><a href="#${it.id}">${inline(it.text)}</a></li>`;
    }
    toc += '</ol></div>';
  }

  const title = meta.title || 'dMart UCI — Informe Técnico Final';
  const authors = meta.author ? renderAuthor(meta.author) : '';
  const dateHtml = `<div class="meta">Informe v5.0.0 · 17 de septiembre de 2026</div>`;

  const html = `<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>dMart UCI — Informe Técnico Final</title>
<style>${css}
/* ---- overrides para el informe generado desde Markdown ---- */
section h1{
  font-size:18pt; background:none; -webkit-text-fill-color:currentColor;
  color:var(--navy); border-bottom:2px solid var(--teal-l); padding-bottom:6px;
  margin:0 0 .4em; page-break-after:avoid;
}
section{page-break-inside:auto;}
hr{border:none; border-top:1px solid var(--line); margin:1.1em 0;}
ul,ol{padding-left:1.7em; margin:.5em 0;}
li{margin:.16em 0;}
pre.code-block{
  background:var(--soft); border:1px solid var(--line); border-radius:8px;
  padding:9px 12px; font-size:8.6pt; line-height:1.5; overflow:hidden;
}
.toc-page h2{ margin-bottom:.6em; }
.cover .authors{ font-size:10pt; }
.ref p{ text-indent:-2em; padding-left:2em; }
</style></head>
<body>

${coverHtml || `<section class="cover">
  <div class="brand">dMart UCI &nbsp;·&nbsp; Informe Técnico Final</div>
  <div>
    <h1>${title.replace(/\*\*([^*]+)\*\*/g, '<span class="hl">$1</span>')}</h1>
    <p class="sub">Diseño, robustez, seguridad, análisis de sensibilidad y validación técnica de una plataforma
    integral para UCI: 152 tests verificados, 35 especificaciones SDD, MFA + RBAC + cifrado ChaCha20-Poly1305,
    compliance HIPAA/ISO&nbsp;27001 y arranque en menos de 140&nbsp;ms.</p>
    ${authors}
  </div>
  <div>
    <div class="kpis">
      <div class="kpi"><b>152</b><span>Tests verificados</span></div>
      <div class="kpi"><b>&lt;140 ms</b><span>Arranque</span></div>
      <div class="kpi"><b>69</b><span>Rutas API</span></div>
      <div class="kpi"><b>35</b><span>SPECs SDD</span></div>
      <div class="kpi"><b>18,2 MiB</b><span>Binario release</span></div>
    </div>
    ${dateHtml}
    <div class="stack">Rust 1.98 · edition 2024 · Axum 0.8 · SurrealDB/SurrealKV embebido · Leptos 0.8 → WebAssembly ·
    HL7 v2.4 + MLLP/MQTT · FHIR R4 · Prometheus · Argon2id · JWT · ChaCha20-Poly1305</div>
  </div>
</section>`}

${toc ? `<section class="toc-page page-break"><h2>Contenido</h2>${toc}</section>` : ''}

${body.join('\n')}

<div class="footer-note">dMart UCI — Informe Técnico Final v5.0.0 · 17 de septiembre de 2026<br>
${meta.author ? meta.author.split('\n')[0].replace(/^\d+/, '').replace(/\\[()^]/g, '').replace('\\','').trim() : 'Centeno-Romero, M. V. y Angulo Peña, R. A.'} · Universidad de Oriente — Núcleo de Sucre</div>

</body></html>`;
  return html;
}

function renderHtmlDocx(blocks, meta) {
  const tocItems = [];
  let secId = 0;
  const body = [];
  for (const b of blocks) {
    if (b.type === 'h1') {
      secId++;
      body.push(`<h1 id="s${secId}">${inline(b.text)}</h1>`);
      tocItems.push({ lvl: 1, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h2') {
      secId++;
      body.push(`<h2 id="s${secId}">${inline(b.text)}</h2>`);
      tocItems.push({ lvl: 2, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h3') {
      secId++;
      body.push(`<h3 id="s${secId}">${inline(b.text)}</h3>`);
      tocItems.push({ lvl: 3, id: `s${secId}`, text: b.text });
    } else if (b.type === 'h4') {
      body.push(`<h4>${inline(b.text)}</h4>`);
    } else if (b.type === 'h5' || b.type === 'h6') {
      body.push(`<h5>${inline(b.text)}</h5>`);
    } else if (b.type === 'p') {
      body.push(`<p>${inline(b.text)}</p>`);
    } else if (b.type === 'hr') {
      body.push('<hr>');
    } else if (b.type === 'code') {
      body.push(`<pre>${esc(b.text)}</pre>`);
    } else if (b.type === 'table') {
      let t = '<table><thead><tr>';
      t += b.header.map((c) => `<th>${inline(c)}</th>`).join('');
      t += '</tr></thead><tbody>';
      for (const r of b.rows) {
        t += '<tr>' + r.map((c) => `<td>${inline(c)}</td>`).join('') + '</tr>';
      }
      t += '</tbody></table>';
      body.push(t);
    } else if (b.type === 'ul' || b.type === 'ol') {
      const tag = b.type;
      let l = `<${tag}>`;
      for (const it of b.items) {
        l += `<li>${inline(it.text)}`;
        if (it.sub.length) l += '<ul><li>' + it.sub.map(inline).join('</li><li>') + '</li></ul>';
        l += '</li>';
      }
      l += `</${tag}>`;
      body.push(l);
    }
  }

  let toc = '';
  if (tocItems.length) {
    toc = '<h2>Contenido</h2><ul>';
    for (const it of tocItems) {
      toc += `<li><a href="#${it.id}">${inline(it.text)}</a></li>`;
    }
    toc += '</ul>';
  }

  const authors = meta.author ? renderAuthor(meta.author).replace(/<p class="authors">/, '<p><strong>').replace('</p>', '</strong></p>') : '';

  return `<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8"><title>dMart UCI — Informe Técnico Final</title>
<style>
body{ font-family:Calibri,Arial,sans-serif; font-size:10.5pt; }
h1{ font-size:15pt; color:#0b1f3a; }
h2{ font-size:13pt; color:#0f766e; border-bottom:1px solid #cbd5e1; padding-bottom:2px; }
h3{ font-size:11.5pt; color:#1e293b; }
table{ border-collapse:collapse; width:100%; margin:6px 0; }
th,td{ border:1px solid #8a93a2; padding:4px 6px; vertical-align:top; font-size:9pt; }
th{ background:#e2e8f0; }
pre{ font-family:'Courier New',monospace; font-size:8.5pt; background:#f1f5f9; border:1px solid #cbd5e1; padding:6px; }
li{ margin:2px 0; }
</style></head>
<body>
<h1>${(meta.title || 'dMart UCI — Informe Técnico Final').replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')}</h1>
${authors}
<p><strong>Informe v5.0.0</strong> · 17 de septiembre de 2026 · ${esc(meta.date || 'Septiembre 2026')}</p>
${toc}
${body.join('\n')}
</body></html>`;
}

const mdSrc = readFileSync(MD, 'utf8');
const { meta, body } = parseDocument(mdSrc);
const blocks = toBlocks(body);

const tmpl = readFileSync(TMPL, 'utf8');
const cssMatch = tmpl.match(/<style>([\s\S]*?)<\/style>/);
const css = cssMatch ? cssMatch[1] : '';
const coverMatch = tmpl.match(/<section class="cover">[\s\S]*?<\/section>/);
const coverHtml = coverMatch ? coverMatch[0] : '';

mkdirSync(OUT, { recursive: true });
writeFileSync(join(OUT, 'informe_pdf.html'), renderHtmlPdf(blocks, meta, css, coverHtml));
writeFileSync(join(OUT, 'informe_docx.html'), renderHtmlDocx(blocks, meta));

console.log('OK → build/informe_pdf.html, build/informe_docx.html');