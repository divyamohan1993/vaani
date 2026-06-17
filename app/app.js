/* Vaani — voice → text into any app. Compact pill UI.
 * Recognition runs in Chrome's Web Speech engine (only robust Google path).
 * Finalized phrases are POSTed to the local Rust helper, which types them into
 * whatever window is focused. No helper → clipboard fallback. */
'use strict';

const HELPER_PORT = 17653;
const LOCAL = ['127.0.0.1', 'localhost'].includes(location.hostname);
const HELPER = LOCAL ? '' : `http://127.0.0.1:${HELPER_PORT}`;
const AUTO_PUNCT = true;

const $ = (s) => document.querySelector(s);
const els = {
  body: document.body, mic: $('#mic'), dot: $('#dot'),
  needchrome: $('#needchrome'), thisurl: $('#thisurl'),
};

const store = {
  get: (k, d) => { try { const v = localStorage.getItem('vaani.' + k); return v === null ? d : JSON.parse(v); } catch { return d; } },
  set: (k, v) => { try { localStorage.setItem('vaani.' + k, JSON.stringify(v)); } catch {} },
};

const state = {
  listening: false, wantListening: false, helper: false,
  lang: store.get('lang', 'en-IN'), sentenceStart: true, clip: '',
};

function note(msg) {
  try { if (state.helper) fetch(HELPER + '/log', { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: String(msg) }).catch(() => {}); } catch {}
}

/* ---------------- Speech recognition ---------------- */
const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
let rec = null;

if (!SR) {
  els.needchrome.hidden = false;
  els.thisurl.textContent = location.href;
} else {
  buildRecognizer();
}

function buildRecognizer() {
  rec = new SR();
  rec.continuous = true;
  rec.interimResults = true;
  rec.maxAlternatives = 1;
  rec.lang = state.lang;

  rec.onstart = () => { setListening(true); note('onstart lang=' + rec.lang); };

  rec.onresult = (e) => {
    for (let i = e.resultIndex; i < e.results.length; i++) {
      if (e.results[i].isFinal) deliverFinal(e.results[i][0].transcript);
    }
  };

  rec.onerror = (e) => {
    note('error:' + e.error);
    if (e.error === 'not-allowed' || e.error === 'service-not-allowed') {
      stopListening();
      els.dot.title = 'Microphone blocked — allow mic for this page in Chrome';
    }
  };

  // Chrome ends on silence/~60s; restart while the user still wants it.
  rec.onend = () => {
    setListening(false);
    if (state.wantListening) {
      try { rec.start(); } catch { setTimeout(() => { if (state.wantListening) { try { rec.start(); } catch {} } }, 250); }
    }
  };
}

function startListening() { if (rec) { state.wantListening = true; note('start()'); try { rec.start(); } catch (e) { note('start-threw:' + e); } } }
function stopListening() { state.wantListening = false; try { rec.stop(); } catch {} setListening(false); }
function toggleListening() { state.wantListening ? stopListening() : startListening(); }

function setListening(on) {
  state.listening = on;
  els.body.dataset.listening = String(on);
  els.mic.setAttribute('aria-pressed', String(on));
  // Tell the helper so the native mic dot can recolour (idle ↔ live).
  try { if (state.helper) fetch(HELPER + '/state', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ listening: on }) }).catch(() => {}); } catch {}
}

/* ---------------- Deliver finalized text ---------------- */
let queue = Promise.resolve();

function deliverFinal(raw) {
  let text = raw.trim();
  if (!text) return;
  if (AUTO_PUNCT && state.lang.startsWith('en')) {
    if (state.sentenceStart) text = text.charAt(0).toUpperCase() + text.slice(1);
    state.sentenceStart = /[.!?]\s*$/.test(text);
  }
  text += ' ';
  queue = queue.then(() => send(text)).catch(() => {});
}

async function send(text) {
  if (!state.helper) { copyToClipboard(text); return; }
  try {
    const r = await fetch(HELPER + '/type', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text, mode: 'type' }),
    });
    if (!r.ok) throw new Error('helper ' + r.status);
  } catch { setHelper(false); copyToClipboard(text); }
}

async function copyToClipboard(text) {
  state.clip += text;
  try { await navigator.clipboard.writeText(state.clip.trim()); } catch {}
}

/* ---------------- Helper bridge ---------------- */
async function checkHelper() {
  try { const r = await fetch(HELPER + '/health', { cache: 'no-store' }); setHelper(r.ok); }
  catch { setHelper(false); }
}

function setHelper(on) {
  if (on === state.helper) return;
  state.helper = on;
  els.dot.className = 'dot ' + (on ? 'ok' : 'warn');
  els.dot.title = on ? 'Helper connected — typing into your apps' : 'Helper offline — copying to clipboard (Ctrl+V)';
}

// Hotkey/tray commands flow helper → page via a short poll (HTTP-only design).
async function poll() {
  if (!state.helper) return;
  try {
    const r = await fetch(HELPER + '/poll', { cache: 'no-store' });
    if (!r.ok) return;
    const { action } = await r.json();
    if (action === 'toggle') toggleListening();
    else if (action === 'start') startListening();
    else if (action === 'stop') stopListening();
    else if (action && action.startsWith('lang:')) {
      const pill = document.querySelector('.lang-pill[data-lang="' + action.slice(5) + '"]');
      if (pill) pill.click();
    }
  } catch { setHelper(false); }
}

/* ---------------- UI wiring ---------------- */
els.mic.addEventListener('click', toggleListening);
document.addEventListener('keydown', (e) => {
  if (e.code === 'Space' && e.target === document.body) { e.preventDefault(); toggleListening(); }
});

document.querySelectorAll('.lang-pill').forEach((p) => {
  p.addEventListener('click', () => {
    document.querySelectorAll('.lang-pill').forEach((x) => { x.classList.remove('is-active'); x.setAttribute('aria-checked', 'false'); });
    p.classList.add('is-active'); p.setAttribute('aria-checked', 'true');
    state.lang = p.dataset.lang; store.set('lang', state.lang); state.sentenceStart = true;
    if (rec) { rec.lang = state.lang; if (state.wantListening) { try { rec.stop(); } catch {} } } // onend restarts in new lang
  });
  if (p.dataset.lang === state.lang) { p.classList.add('is-active'); p.setAttribute('aria-checked', 'true'); }
  else { p.classList.remove('is-active'); p.setAttribute('aria-checked', 'false'); }
});

/* ---------------- Boot ---------------- */
checkHelper();
setInterval(checkHelper, 3000);
setInterval(poll, 300);
