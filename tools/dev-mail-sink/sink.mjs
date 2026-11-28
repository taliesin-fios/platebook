// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * A local SMTP mail sink for development and rehearsals. Nothing is relayed:
 * every message is stored as an .eml file and listed on a small web page.
 *
 *   node tools/dev-mail-sink/sink.mjs [--smtp 1025] [--http 1080] [--dir <tmp>/platebook-mail-sink]
 *
 * Stored messages include live sign-in and reset links: keep --dir outside the
 * repository (the default is the system temp directory).
 *
 * Point Platebook at it with EMAIL_HOST=127.0.0.1 EMAIL_PORT=1025 EMAIL_TLS=none
 * (and no EMAIL_USER/EMAIL_PASS).
 *
 * Failure modes, to rehearse an unreliable mail server (POST /mode?set=…):
 *   accept    – 250 to everything (default)
 *   reject    – 550 at RCPT TO (a permanent rejection)
 *   tempfail  – 451 at RCPT TO (a transient failure; senders may retry)
 *   down      – the SMTP port is closed (connection refused)
 *
 * HTTP: GET /            message list (HTML)
 *       GET /api/messages  JSON list
 *       GET /messages/:n   raw message      GET /messages/:n/text  decoded text part
 *       GET /mode          current mode     POST /mode?set=accept|reject|tempfail|down
 *       POST /clear        delete stored messages
 *
 * Zero dependencies; Node 20+. Binds to 127.0.0.1 only.
 */

import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const arg = (name, dflt) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : dflt;
};
const SMTP_PORT = Number(arg('smtp', 1025));
const HTTP_PORT = Number(arg('http', 1080));
const DIR = path.resolve(arg('dir', path.join(os.tmpdir(), 'platebook-mail-sink')));
const MODES = ['accept', 'reject', 'tempfail', 'down'];

fs.mkdirSync(DIR, { recursive: true });
let mode = 'accept';
let smtp = null;

const indexFile = path.join(DIR, 'index.json');
const loadIndex = () => { try { return JSON.parse(fs.readFileSync(indexFile, 'utf8')); } catch { return []; } };
const saveIndex = (idx) => fs.writeFileSync(indexFile, JSON.stringify(idx, null, 2));
const log = (...a) => console.log(new Date().toISOString(), '[mail-sink]', ...a);

/** Unfolded top-level headers of a raw message. */
function headers(raw) {
  const head = raw.split(/\r?\n\r?\n/)[0].replace(/\r?\n[ \t]+/g, ' ');
  const out = {};
  for (const line of head.split(/\r?\n/)) {
    const m = line.match(/^([\w-]+):\s*(.*)$/);
    if (m) out[m[1].toLowerCase()] = m[2];
  }
  return out;
}

/** Decodes RFC 2047 encoded words (=?utf-8?b?…?= / ?q?). */
function decodeWords(s = '') {
  return s.replace(/=\?([^?]+)\?([bqBQ])\?([^?]*)\?=/g, (_, _cs, enc, text) => {
    if (enc.toLowerCase() === 'b') return Buffer.from(text, 'base64').toString('utf8');
    return Buffer.from(text.replace(/_/g, ' ').replace(/=([0-9A-F]{2})/gi, (_m, h) => String.fromCharCode(parseInt(h, 16))), 'latin1').toString('utf8');
  });
}

function decodeBody(body, encoding = '') {
  const e = encoding.toLowerCase();
  if (e.includes('base64')) return Buffer.from(body.replace(/\s+/g, ''), 'base64').toString('utf8');
  if (e.includes('quoted-printable')) {
    const bytes = body.replace(/=\r?\n/g, '').replace(/=([0-9A-F]{2})/gi, (_m, h) => String.fromCharCode(parseInt(h, 16)));
    return Buffer.from(bytes, 'latin1').toString('utf8');
  }
  return body;
}

/** The decoded text/plain part and attachment file names. */
function parts(raw) {
  const text = [];
  const attachments = [];
  const walk = (chunk) => {
    const [head, ...rest] = chunk.split(/\r?\n\r?\n/);
    const h = headers(head + '\n\n');
    const body = rest.join('\n\n');
    const ct = h['content-type'] || 'text/plain';
    const boundary = ct.match(/boundary="?([^";]+)"?/i)?.[1];
    if (boundary) {
      for (const p of body.split(`--${boundary}`).slice(1)) {
        if (!p.startsWith('--')) walk(p.replace(/^\r?\n/, ''));
      }
      return;
    }
    const filename = (h['content-disposition'] || '').match(/filename="?([^";]+)"?/i)?.[1];
    if (filename) attachments.push({ filename: decodeWords(filename), type: ct.split(';')[0], bytes: body.replace(/\s+/g, '').length * 3 / 4 | 0 });
    else if (ct.startsWith('text/plain')) text.push(decodeBody(body, h['content-transfer-encoding']));
  };
  walk(raw);
  return { text: text.join('\n').trim(), attachments };
}

function store(raw, envelope) {
  const idx = loadIndex();
  const n = (idx.at(-1)?.n || 0) + 1;
  const file = `${String(n).padStart(4, '0')}.eml`;
  fs.writeFileSync(path.join(DIR, file), raw);
  const h = headers(raw);
  const { attachments } = parts(raw);
  const entry = {
    n, file, received_at: new Date().toISOString(),
    envelope_from: envelope.from, envelope_to: envelope.to,
    from: decodeWords(h.from), to: decodeWords(h.to), reply_to: decodeWords(h['reply-to'] || ''),
    subject: decodeWords(h.subject), attachments,
  };
  idx.push(entry);
  saveIndex(idx);
  log(`stored #${n} to=${entry.to} subject="${entry.subject}"`);
}

function session(sock) {
  let buf = '';
  let inData = false;
  let data = [];
  let env = { from: '', to: [] };
  const send = (s) => sock.write(s + '\r\n');
  send('220 mail-sink.local ESMTP (local sink: nothing is relayed)');
  sock.on('data', (chunk) => {
    buf += chunk.toString('latin1');
    let i;
    while ((i = buf.indexOf('\r\n')) >= 0 || (inData && (i = buf.indexOf('\n')) >= 0)) {
      const line = buf.slice(0, i).replace(/\r$/, '');
      buf = buf.slice(i + (buf[i] === '\r' ? 2 : 1));
      if (inData) {
        if (line === '.') {
          inData = false;
          store(data.join('\r\n'), env);
          data = [];
          env = { from: '', to: [] };
          send('250 2.0.0 OK: stored (not relayed)');
        } else data.push(line.startsWith('..') ? line.slice(1) : line);
        continue;
      }
      const cmd = line.toUpperCase();
      if (cmd.startsWith('EHLO')) { sock.write('250-mail-sink.local\r\n250-8BITMIME\r\n250 SIZE 26214400\r\n'); }
      else if (cmd.startsWith('HELO')) send('250 mail-sink.local');
      else if (cmd.startsWith('MAIL FROM')) { env.from = line.slice(10).trim(); send('250 2.1.0 OK'); }
      else if (cmd.startsWith('RCPT TO')) {
        if (mode === 'reject') { log(`rejected RCPT ${line.slice(8)}`); send('550 5.1.1 Recipient rejected (sink in reject mode)'); }
        else if (mode === 'tempfail') { log(`tempfail RCPT ${line.slice(8)}`); send('451 4.3.0 Temporary failure (sink in tempfail mode)'); }
        else { env.to.push(line.slice(8).trim()); send('250 2.1.5 OK'); }
      }
      else if (cmd === 'DATA') { inData = true; send('354 End data with <CR><LF>.<CR><LF>'); }
      else if (cmd.startsWith('RSET')) { env = { from: '', to: [] }; send('250 OK'); }
      else if (cmd.startsWith('NOOP')) send('250 OK');
      else if (cmd.startsWith('QUIT')) { send('221 Bye'); sock.end(); return; }
      else send('502 5.5.1 Command not implemented');
    }
  });
  sock.on('error', () => {});
}

function startSmtp() {
  if (smtp) return;
  smtp = net.createServer(session);
  smtp.listen(SMTP_PORT, '127.0.0.1', () => log(`SMTP listening on 127.0.0.1:${SMTP_PORT}`));
}
function stopSmtp() {
  if (!smtp) return;
  smtp.close();
  smtp = null;
  log('SMTP port closed (mode: down)');
}

const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
function page() {
  const rows = loadIndex().slice().reverse().map((m) => `<tr><td>${m.n}</td><td>${esc(m.received_at.replace('T', ' ').slice(0, 19))}</td>
    <td>${esc(m.to)}</td><td><a href="/messages/${m.n}/text">${esc(m.subject)}</a></td>
    <td>${esc(m.reply_to)}</td><td>${m.attachments.map((a) => esc(a.filename)).join(', ')}</td></tr>`).join('');
  return `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Mail sink</title><style>body{font:14px system-ui;margin:16px}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ddd;padding:6px;text-align:left;vertical-align:top}
.mode{display:inline-block;padding:4px 10px;border-radius:12px;background:${mode === 'accept' ? '#dcfce7' : '#fee2e2'}}</style>
<h1>Local mail sink</h1><p>Nothing here was relayed to a real inbox. SMTP 127.0.0.1:${SMTP_PORT} · mode: <span class="mode">${mode}</span></p>
<table><tr><th>#</th><th>Received (UTC)</th><th>To</th><th>Subject</th><th>Reply-To</th><th>Attachments</th></tr>${rows || '<tr><td colspan=6>No messages yet</td></tr>'}</table>`;
}

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  const json = (v, code = 200) => { res.writeHead(code, { 'content-type': 'application/json' }); res.end(JSON.stringify(v, null, 2)); };
  if (req.method === 'GET' && url.pathname === '/') { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); return res.end(page()); }
  if (req.method === 'GET' && url.pathname === '/api/messages') return json(loadIndex());
  if (url.pathname === '/mode') {
    if (req.method === 'POST') {
      const next = url.searchParams.get('set');
      if (!MODES.includes(next)) return json({ error: `mode must be one of ${MODES.join(', ')}` }, 400);
      mode = next;
      if (mode === 'down') stopSmtp(); else startSmtp();
      log(`mode → ${mode}`);
    }
    return json({ mode });
  }
  if (req.method === 'POST' && url.pathname === '/clear') {
    for (const m of loadIndex()) fs.rmSync(path.join(DIR, m.file), { force: true });
    saveIndex([]);
    return json({ cleared: true });
  }
  const m = url.pathname.match(/^\/messages\/(\d+)(\/text)?$/);
  if (req.method === 'GET' && m) {
    const entry = loadIndex().find((e) => e.n === Number(m[1]));
    if (!entry) return json({ error: 'not found' }, 404);
    const raw = fs.readFileSync(path.join(DIR, entry.file), 'utf8');
    res.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' });
    return res.end(m[2] ? `To: ${entry.to}\nReply-To: ${entry.reply_to}\nSubject: ${entry.subject}\nAttachments: ${entry.attachments.map((a) => a.filename).join(', ') || '(none)'}\n\n${parts(raw).text}` : raw);
  }
  json({ error: 'not found' }, 404);
}).listen(HTTP_PORT, '127.0.0.1', () => log(`web view on http://127.0.0.1:${HTTP_PORT}`));

startSmtp();
