/* EUDI Evidence Inference — Demo Console.
 * Pure client of the existing /v1 API. No canned data: every panel is rendered
 * from a real response, and every exchange is shown as raw JSON. */

"use strict";

const SUBJECT = "user-123";
const PID_CLAIMS = ["given_name", "family_name", "birth_date", "resident_country"];
let DISCLOSABLE = PID_CLAIMS.slice();

const $ = (id) => document.getElementById(id);
const rp = $("rp-out"), wallet = $("wallet-out"), engine = $("engine-out");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---- API client -----------------------------------------------------------

async function api(method, path, body) {
  const opt = { method, headers: { "Content-Type": "application/json" } };
  if (body !== undefined) opt.body = JSON.stringify(body);
  let res, data, txt = "";
  try {
    res = await fetch(path, opt);
    txt = await res.text();
  } catch (e) {
    setOffline(true);
    throw e;
  }
  try { data = txt ? JSON.parse(txt) : null; } catch (_) { data = txt; }
  // Engine down: Java returns 503 ENGINE_UNAVAILABLE. Surface the banner, but
  // keep the real response visible — never fake a result.
  const engineDown = res.status === 503 || (data && data.error === "ENGINE_UNAVAILABLE");
  setOffline(engineDown);
  logExchange(method, path, body, res.status, data);
  if (engineDown) throw new Error("engine unreachable (503)");
  return { ok: res.ok, status: res.status, data };
}

function setOffline(on) { $("offline").classList.toggle("hidden", !on); }

function logExchange(method, path, reqBody, status, respBody) {
  const d = document.createElement("details");
  d.className = "ex";
  const s = document.createElement("summary");
  s.innerHTML = `<span class="meth">${method} ${path}</span> → ${status}`;
  d.appendChild(s);
  if (reqBody !== undefined) d.appendChild(pre("request", reqBody));
  d.appendChild(pre("response", respBody));
  $("raw-log").prepend(d);
}

function pre(label, obj) {
  const wrap = document.createElement("div");
  const cap = document.createElement("div");
  cap.style.cssText = "font-size:11px;color:#6b7280;margin-top:6px";
  cap.textContent = label;
  const p = document.createElement("pre");
  p.textContent = typeof obj === "string" ? obj : JSON.stringify(obj, null, 2);
  wrap.appendChild(cap); wrap.appendChild(p);
  return wrap;
}

// ---- small render helpers -------------------------------------------------

function active(colId) {
  ["col-rp", "col-wallet", "col-engine"].forEach((c) =>
    $(c).classList.toggle("active", c === colId));
}
function esc(s) { return String(s).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c])); }
function proves(text) { rp.innerHTML = `<div class="proves fade"><b>What this proves:</b> ${esc(text)}</div>`; }
function resetPanels() {
  wallet.innerHTML = '<p class="idle">Issuing credential…</p>';
  engine.innerHTML = '<p class="idle">Waiting for the relying party…</p>';
}
function rawDetails(title, obj) {
  return `<details class="inline"><summary>${esc(title)}</summary><pre>${esc(JSON.stringify(obj, null, 2))}</pre></details>`;
}
function badge(decision) {
  const d = (decision || "").toLowerCase();
  const label = decision === "REPLAY_DETECTED" ? "REPLAY_DETECTED" : decision;
  return `<span class="badge ${d}">${esc(label)}</span>`;
}

// ---- SD-JWT VP decoding (client-side proof of disclosure) ------------------

function b64urlDecode(s) {
  s = s.replace(/-/g, "+").replace(/_/g, "/");
  while (s.length % 4) s += "=";
  return atob(s);
}
function safeJson(s) { try { return JSON.parse(s); } catch (_) { return null; } }

// Decode one compact JWS into { header, payload, sig, raw }.
function decodeJwt(jwt) {
  const p = (jwt || "").split(".");
  if (p.length < 2) return { header: null, payload: null, sig: "", raw: jwt };
  return { header: safeJson(b64urlDecode(p[0])), payload: safeJson(b64urlDecode(p[1])), sig: p[2] || "", raw: jwt };
}

// Parse an SD-JWT — issuance form ("<issuer-jwt>~<d>*~", trailing ~) or a VP
// ("<issuer-jwt>~<d>*~<kb-jwt>"). Shared by the VC and VP renderers.
function parseSdJwt(compact) {
  const segs = (compact || "").split("~");
  const trailingEmpty = segs[segs.length - 1] === "";
  const kbRaw = trailingEmpty ? null : segs[segs.length - 1];
  const discSegs = segs.slice(1, segs.length - 1);
  const disclosures = discSegs.filter(Boolean).map((s) => {
    const a = safeJson(b64urlDecode(s));
    return a ? { raw: s, salt: a[0], claim: a[1], value: a[2] } : { raw: s, salt: null, claim: null, value: null };
  });
  return { issuer: decodeJwt(segs[0]), disclosures, kb: kbRaw ? decodeJwt(kbRaw) : null };
}

// Back-compat wrapper for the wallet column (names + values only).
function decodeDisclosures(vp) {
  return parseSdJwt(vp).disclosures.filter((d) => d.claim != null).map((d) => ({ name: d.claim, value: d.value }));
}

function tamperVp(vp) {
  const p = vp.split("~");
  if (p.length < 3) return vp;
  const d = p[1], last = d.slice(-1);
  p[1] = d.slice(0, -1) + (last === "A" ? "B" : "A");
  return p.join("~");
}

// ---- column renderers -----------------------------------------------------

function renderWalletIssued() {
  wallet.innerHTML =
    `<div class="fade"><div class="kv"><span class="k">VC held:</span> <span class="v">PID · SD-JWT VC</span></div>
     <div class="kv"><span class="k">Holder key:</span> <span class="v">ES256 (P-256), key-bound via cnf</span></div>
     <div class="kv"><span class="k">Disclosable claims:</span> <span class="v">${DISCLOSABLE.length}</span></div>
     <p class="idle">Awaiting an evidence request to build a minimal presentation…</p></div>`;
}

function renderDisclosure(disclosed) {
  const names = new Set(disclosed.map((d) => d.name));
  const valOf = Object.fromEntries(disclosed.map((d) => [d.name, d.value]));
  const items = DISCLOSABLE.map((c) => {
    if (names.has(c)) {
      return `<li class="claim disclosed"><span class="mark">✓</span>
        <span class="name">${esc(c)}</span>
        <span class="val">= ${esc(valOf[c])}</span>
        <span class="lbl">disclosed</span></li>`;
    }
    return `<li class="claim withheld"><span class="mark">✗</span>
      <span class="name">${esc(c)}</span>
      <span class="lbl">withheld</span></li>`;
  }).join("");
  wallet.innerHTML =
    `<div class="fade"><div class="kv"><span class="k">Selective disclosure (building the VP)</span></div>
     <ul class="claims">${items}</ul>
     <p class="hint">The wallet reveals only what the request needs; withheld claims never leave the device.</p></div>`;
}

function renderResolution(q) {
  const kind = q.resolvedBy === "LLM_VALIDATED" ? "llm" : "registry";
  engine.innerHTML =
    `<div class="fade"><div class="kv"><span class="k">Resolved by:</span>
       <span class="pill ${kind}">${esc(q.resolvedBy)}</span></div>
     <div class="kv"><span class="k">Canonical:</span> <span class="v">${esc(q.canonical)}</span>
       <span class="k">(${esc(q.kind)})</span></div>
     <div class="kv"><span class="k">Required evidence:</span>
       <span class="v">${esc((q.requiredEvidence || []).join(", "))}</span></div>
     <div id="verify-slot"></div></div>`;
}

function checksList(checks) {
  if (!Array.isArray(checks) || !checks.length) return "";
  const lis = checks.map((c) =>
    `<li data-ok="${c.ok}"><span class="ico">${c.ok ? "✓" : "✗"}</span>
       <span>${esc(c.name)}</span><span class="detail">${esc(c.detail || "")}</span></li>`).join("");
  return `<div class="kv" style="margin-top:12px"><span class="k">Verification checks</span></div>
          <ul class="checks">${lis}</ul>`;
}
async function animateChecks(container) {
  const lis = container.querySelectorAll(".checks li");
  for (const li of lis) {
    await sleep(180);
    li.classList.add("shown", li.dataset.ok === "true" ? "pass" : "fail");
  }
}

async function renderDecision(data, opts = {}) {
  const slot = $("verify-slot") || engine;
  let html = badge(data.decision);
  if (data.reason) html += ` <span class="k">${esc(data.reason)}</span>`;
  if (data.verifiedClaims)
    html += `<div class="kv"><span class="k">Verified claims:</span> <span class="v">${esc(JSON.stringify(data.verifiedClaims))}</span></div>`;
  if (typeof data.disclosureCount === "number")
    html += `<div class="kv"><span class="k">Disclosures used:</span> <span class="v">${data.disclosureCount}</span></div>`;
  html += checksList(data.verificationChecks);
  if (data.failedCheck)
    html += `<div class="note red">Failed check: <b>${esc(data.failedCheck)}</b></div>`;
  if (opts.moneyShot && data.decision === "ALLOW")
    html += `<div class="note">Response contains the derived boolean only — <b>no birth date anywhere</b>.</div>`;
  html += rawDetails("Raw decision JSON", data);
  slot.insertAdjacentHTML("beforeend", `<div class="fade">${html}</div>`);
  await animateChecks(slot);
}

// ---- protocol exchange panel ----------------------------------------------

let ARTSEQ = 0;
const pbody = (n) => document.querySelector(`#pstep-${n} .pbody`);
function pstepActive(n) {
  [1, 2, 3].forEach((i) => document.getElementById(`pstep-${i}`).classList.toggle("active", i === n));
}
function protoReset() {
  pstepActive(0);
  pbody(1).className = "pbody idle"; pbody(1).textContent = "Run Scenario 1 or 6 to mint a fresh SD-JWT VC.";
  pbody(2).className = "pbody idle"; pbody(2).textContent = "The holder's VP — fewer disclosures than the VC.";
  pbody(3).className = "pbody idle"; pbody(3).textContent = "Artifacts checked vs issuer JWKS & consumed against the replay store.";
}
function protoNA(text) {
  protoReset();
  pbody(1).className = "pbody"; pbody(1).innerHTML = `<div class="pnote">${esc(text)}</div>`;
}

function artifactBlock(label, full, note) {
  const id = "art" + ARTSEQ++;
  const preview = esc(full.length > 90 ? full.slice(0, 90) + "…" : full);
  return `<div class="artifact"><div class="art-head"><span class="art-label">${esc(label)}</span>
      <button class="mini" data-toggle="${id}">show full</button>
      <button class="mini" data-copy="${id}">copy</button></div>
    <code class="art-preview">${preview}</code>
    <pre id="${id}" class="art-full hidden">${esc(full)}</pre>
    ${note ? `<div class="art-note">${note}</div>` : ""}</div>`;
}
function jsonBlock(label, obj) {
  return `<div class="decoded"><span class="art-label">${esc(label)}</span><pre>${esc(JSON.stringify(obj, null, 2))}</pre></div>`;
}
function disclosuresBlock(title, disclosures) {
  const rows = disclosures.map((d) =>
    `<li><code>[${esc(d.salt || "?")}, "${esc(d.claim || "?")}", ${esc(JSON.stringify(d.value))}]</code></li>`).join("");
  return `<div class="decoded"><span class="art-label">${esc(title)} — ${disclosures.length} disclosure(s)</span><ul class="disc">${rows}</ul></div>`;
}

// Step 1 — issuance. mode "full" (scenario 1/6) or "reuse" (scenario 3/4).
function protoIssuance(vc, mode) {
  pstepActive(1);
  const b = pbody(1); b.className = "pbody";
  const parsed = parseSdJwt(vc.combined);
  let html = "";
  if (mode === "reuse")
    html += `<div class="pnote reuse">Reusing this subject's already-issued VC — run Scenario 1 or 6 to watch issuance.</div>`;
  html += artifactBlock("SD-JWT VC (combined, as issued)", vc.combined,
    "Format: &lt;issuer-jwt&gt;~&lt;disclosure&gt;*~ (trailing ~, no KB-JWT at issuance).");
  html += jsonBlock("Issuer JWT · header", parsed.issuer.header);
  html += jsonBlock("Issuer JWT · payload (note the _sd digests — the hidden claims)", parsed.issuer.payload);
  html += disclosuresBlock("Disclosures (issued — ALL claims)", parsed.disclosures);
  html += `<div class="art-note">Paste the first segment (before the first ~) into jwt.io to verify the issuer signature.</div>`;
  b.innerHTML = html;
}

// Step 2 — presentation. vcCount = disclosures in the issued VC (for the drop).
function protoPresentation(vp, vcCount) {
  pstepActive(2);
  const b = pbody(2); b.className = "pbody";
  const parsed = parseSdJwt(vp);
  const vpCount = parsed.disclosures.length;
  let html = `<div class="mini-compare"><div class="box vc"><span class="n">${vcCount}</span>VC disclosures</div>
      <div class="drop">→ minimised →</div><div class="box vp"><span class="n">${vpCount}</span>VP disclosures</div></div>`;
  html += artifactBlock("Verifiable Presentation (VP, minimised)", vp,
    "Format: &lt;issuer-jwt&gt;~&lt;selected-disclosure&gt;*~&lt;kb-jwt&gt;.");
  html += disclosuresBlock("Disclosures included this time", parsed.disclosures);
  if (parsed.kb) {
    html += jsonBlock("KB-JWT · header (typ: kb+jwt)", parsed.kb.header);
    html += jsonBlock("KB-JWT · payload (aud, nonce, sd_hash, iat)", parsed.kb.payload);
  } else {
    html += `<div class="pnote">No KB-JWT present (malformed VP).</div>`;
  }
  b.innerHTML = html;
}

// Step 3 — verification, tied back to the artifacts from steps 1–2.
async function protoVerification(checks, failedCheck) {
  pstepActive(3);
  const b = pbody(3); b.className = "pbody";
  let html = `<div class="art-note">The issuer JWT (step 1) is verified against <code>/engine/issuer/jwks</code>;
    each disclosure is re-hashed to an <code>_sd</code> digest; the KB-JWT (step 2) is checked against the
    credential's <code>cnf</code> key; the nonce is consumed once.</div>`;
  html += checksList(checks);
  if (failedCheck) html += `<div class="note red">Failed check: <b>${esc(failedCheck)}</b></div>`;
  b.innerHTML = html;
  await animateChecks(b);
}

// ---- scenario orchestration ----------------------------------------------

// ---- resolution pipeline panel --------------------------------------------

let CATALOGUE = [];
let CURRENT_PIPE = null;
const pipeEl = () => document.getElementById("pipe");
const CAT_HINT = { age_over_18: "age predicate", BANK_ACCOUNT_OPENING_V1: "bank-account policy" };

// One real /v1 call: a deliberately out-of-scope question returns the closed set.
async function fetchCatalogue() {
  try {
    const r = await api("POST", "/v1/verification/questions", { subjectId: SUBJECT, question: "enumerate the supported questions" });
    if (!r.ok && r.data && Array.isArray(r.data.supportedQuestions)) CATALOGUE = r.data.supportedQuestions;
  } catch (_) { /* offline handled by api() */ }
}

function pipeFromQuestionSuccess(question, d) {
  const tier1 = d.tier1 || (d.resolvedBy === "REGISTRY" ? "HIT" : "MISS");
  if (tier1 === "HIT") return { question, tier1: "HIT", canonical: d.canonical, kind: d.kind };
  return { question, tier1: "MISS", canonical: d.canonical, kind: d.kind,
    tier2: { catalogue: CATALOGUE, proposal: null, gate1: "PASS", gate2: "PASS", outcome: "ACCEPT", stage: null } };
}
function pipeFromDecision(question, d) {
  const tier1 = d.tier1 || (d.resolvedBy === "REGISTRY" ? "HIT" : "MISS");
  if (tier1 === "HIT") return { question, tier1: "HIT", canonical: d.canonical, kind: "policy" };
  return { question, tier1: "MISS", canonical: d.canonical, kind: "policy",
    tier2: { catalogue: CATALOGUE, proposal: null, gate1: "PASS", gate2: "PASS", outcome: "ACCEPT", stage: null } };
}
function pipeFromRefusal(question, d) {
  if (d && Array.isArray(d.supportedQuestions) && d.supportedQuestions.length) CATALOGUE = d.supportedQuestions;
  const stage = (d && d.stage) || null;
  const t2 = { catalogue: CATALOGUE, proposal: null, outcome: "REFUSE", stage };
  if (stage === "ENGINE_MEMBERSHIP_REJECTED") { t2.gate1 = "PASS"; t2.gate2 = "FAIL"; }
  else if (stage === "TIER2_LOW_CONFIDENCE") { t2.gate1 = "FAIL"; t2.gate2 = "NOT_REACHED"; }
  else { t2.gate1 = "NOT_REACHED"; t2.gate2 = "NOT_REACHED"; } // ABSTAIN / TIMEOUT
  return { question, tier1: "MISS", canonical: null, kind: null, tier2: t2 };
}

function pipelineSetProposal(proposalJson) {
  if (CURRENT_PIPE && CURRENT_PIPE.tier2) { CURRENT_PIPE.tier2.proposal = proposalJson; renderPipeline(CURRENT_PIPE); }
}

function prow(n, state, title, detail) {
  return `<li class="pr ${state} fade"><div class="pnum">${n}</div><div class="pc">
    <div class="pt">${title}</div>${detail ? `<div class="pd">${detail}</div>` : ""}</div></li>`;
}
function gateRow(n, title, state, detail) {
  const st = state === "PASS" ? "pass" : state === "FAIL" ? "fail" : "skip";
  const tag = state === "PASS" ? `<span class="ptag pass">PASS</span>`
    : state === "FAIL" ? `<span class="ptag fail">FAIL</span>` : `<span class="ptag wait">not reached</span>`;
  return prow(n, st, `${title} ${tag}`, detail);
}

function renderPipeline(m) {
  CURRENT_PIPE = m;
  const out = [];
  out.push(prow(1, "info", "Question in", `<code>${esc(m.question)}</code>`));

  if (m.tier1 === "HIT") {
    out.push(prow(2, "pass",
      `Tier 1 — deterministic registry (Rust) <span class="ptag hit">HIT</span> <span class="ptag">fast path — no AI called</span>`,
      `normalize (lowercase · trim · strip terminal <code>?.!</code>) → exact alias match → <code>${esc(m.canonical)}</code> (${esc(m.kind || "")})`));
    for (const [n, t] of [[3, "Tier 2 — LLM proposer"], [4, "LLM raw proposal"], [5, "Gate 1 — Java confidence"], [6, "Gate 2 — engine membership"], [7, "Outcome"]])
      out.push(prow(n, "skip", t, "not run — Tier 1 resolved it deterministically (the AI only runs on a miss)"));
    pipeEl().innerHTML = out.join("");
    return;
  }

  out.push(prow(2, "info", `Tier 1 — deterministic registry (Rust) <span class="ptag miss">MISS</span>`,
    "no exact alias match → falls through to Tier 2"));
  const t2 = m.tier2 || {};
  const catList = (t2.catalogue && t2.catalogue.length)
    ? `<ul class="cat">${t2.catalogue.map((id) => `<li><code>${esc(id)}</code>${CAT_HINT[id] ? " — " + esc(CAT_HINT[id]) : ""}</li>`).join("")}</ul>`
    : `<span>(closed catalogue)</span>`;
  out.push(prow(3, "info", `Tier 2 — LLM proposer <span class="ptag ai">untrusted</span>`,
    `Sent to the model: <b>only the question + this closed catalogue</b>:${catList}
     <div class="notsent">NOT sent: subjectId, credentials, evidence, PII (G3 — structural: the resolver interface has no such field).</div>`));

  if (t2.proposal) {
    out.push(prow(4, "info", `LLM raw proposal <span class="ptag">verbatim from audit (G5)</span>`,
      `<pre>${esc(JSON.stringify(t2.proposal, null, 2))}</pre>`));
  } else if (t2.outcome === "ACCEPT") {
    out.push(prow(4, "info", `LLM raw proposal <span class="ptag wait">recorded in audit</span>`,
      "The verbatim <code>{canonical, confidence, reason}</code> is fetched from the audit record after presentation."));
  } else {
    let d;
    if (t2.stage === "TIER2_ABSTAIN") d = "Model ABSTAINED — returned <code>canonical: null</code>. Nothing to validate.";
    else if (t2.stage === "TIER2_TIMEOUT") d = "Model unavailable / timed out — fail-closed (G6). No proposal.";
    else if (t2.stage === "TIER2_LOW_CONFIDENCE") d = "Model proposed a canonical but below <code>HIGH</code> confidence (proposal withheld from the refusal response).";
    else d = "Model returned no usable proposal.";
    out.push(prow(4, "info", "LLM raw proposal", d));
  }

  out.push(gateRow(5, "Gate 1 — Java: confidence == HIGH?", t2.gate1,
    t2.gate1 === "PASS" ? "Java accepts only HIGH-confidence proposals."
      : t2.gate1 === "FAIL" ? "Rejected: confidence below HIGH."
      : "Not reached (no usable proposal)."));
  out.push(gateRow(6, "Gate 2 — Rust engine: independent membership re-check", t2.gate2,
    t2.gate2 === "PASS" ? "The engine does NOT trust Java's decision or the proposed string — it re-checks membership against its own registry, and accepts."
      : t2.gate2 === "FAIL" ? "The engine independently rejected the proposed id: not a member of its registry (Java's gate is not the only line of defence)."
      : "Not reached."));

  if (t2.outcome === "ACCEPT") {
    out.push(prow(7, "pass", `Outcome <span class="ptag accept">ACCEPT</span>`,
      `<code>resolvedBy: LLM_VALIDATED</code> → <code>${esc(m.canonical)}</code>. Both independent gates passed.`));
  } else {
    const where = t2.stage === "ENGINE_MEMBERSHIP_REJECTED" ? "stopped at Gate 2"
      : t2.stage === "TIER2_LOW_CONFIDENCE" ? "stopped at Gate 1" : "stopped before any gate";
    out.push(prow(7, "fail", `Outcome <span class="ptag refuse">REFUSE</span>`,
      `${where}: <code>${esc(t2.stage || "UNRESOLVED_QUESTION")}</code>. 422 returned — the AI cannot introduce a new predicate.`));
  }
  pipeEl().innerHTML = out.join("");
}

async function ensureIssued() {
  const r = await api("POST", "/v1/wallet/issue", { subjectId: SUBJECT });
  if (r.ok && r.data && Array.isArray(r.data.disclosableClaims))
    DISCLOSABLE = r.data.disclosableClaims;
  return r;
}
async function ask(question) {
  return api("POST", "/v1/verification/questions", { subjectId: SUBJECT, question });
}
async function present(q) {
  return api("POST", "/v1/wallet/present", {
    subjectId: SUBJECT, requestId: q.requestId, nonce: q.nonce, requiredEvidence: q.requiredEvidence,
  });
}
async function verify(requestId, presentation) {
  // Forward the wallet's COMPLETE presentation (RP is a pass-through). Sending a
  // partial object would serialize sibling fields as null, which the engine's
  // strict deserializer rejects.
  return api("POST", "/v1/verification/presentations", { requestId, presentation });
}

async function runAge() {
  proves("An age question discloses only birth_date; the decision returns the derived boolean, never the date.");
  resetPanels(); protoReset(); active("col-rp");
  const issue = await ensureIssued(); renderWalletIssued();
  const vc = issue.data && issue.data.vc;
  const q = await ask("Is this user over 18?");
  if (!q.ok) return renderRefusal(q.data);
  renderResolution(q.data);
  renderPipeline(pipeFromQuestionSuccess("Is this user over 18?", q.data));
  if (vc) protoIssuance(vc, "full");
  await sleep(300); active("col-wallet");
  const p = await present(q.data);
  renderDisclosure(decodeDisclosures(p.data.sdJwtVp));
  if (vc) protoPresentation(p.data.sdJwtVp, vc.disclosures.length);
  await sleep(300); active("col-engine");
  const v = await verify(q.data.requestId, p.data);
  await renderDecision(v.data, { moneyShot: true });
  await protoVerification(v.data.verificationChecks, v.data.failedCheck);
}

async function runBank() {
  proves("UNKNOWN is a first-class decision: the engine states exactly what must still be proven, then you supply it.");
  resetPanels(); active("col-rp");
  protoNA("Policy-decision path — evaluated from the subject's held evidence. No VC/VP is exchanged here (see Scenario 1 or 6).");
  wallet.innerHTML = '<p class="idle">Policy path uses the subject\'s held evidence.</p>';
  const d1 = await api("POST", "/v1/verification/decisions", { subjectId: SUBJECT, policy: "Can this user open a bank account?" });
  renderPipeline(pipeFromDecision("Can this user open a bank account?", d1.data));
  active("col-engine");
  renderPolicyDecision(d1.data, true);
  renderHeld(d1.data);
}

function renderHeld(d) {
  const used = (d.evidenceUsed || []).map((e) => `${e.type}`);
  wallet.innerHTML = `<div class="fade"><div class="kv"><span class="k">Evidence held & used</span></div>
    <ul class="claims">${used.map((t) => `<li class="claim disclosed"><span class="mark">✓</span><span class="name">${esc(t)}</span></li>`).join("")}</ul></div>`;
}

function renderPolicyDecision(d, allowGrant) {
  let html = badge(d.decision);
  html += `<div class="kv"><span class="k">Policy:</span> <span class="v">${esc(d.canonical)}</span> <span class="k">v${d.policyVersion}</span></div>`;
  if (d.satisfiedPredicates && d.satisfiedPredicates.length)
    html += `<div class="kv"><span class="k">Satisfied:</span> <span class="v">${esc(d.satisfiedPredicates.join(", "))}</span></div>`;
  if (d.missingPredicates && d.missingPredicates.length)
    html += `<div class="kv"><span class="k">Still missing:</span> <span class="v">${esc(d.missingPredicates.join(", "))}</span></div>`;
  if (d.decision === "UNKNOWN")
    html += `<div class="note amber">UNKNOWN is not an error — it is an honest "not yet proven".</div>`;
  const plan = d.evidenceRequestPlan;
  if (plan && plan.requiredEvidence && plan.requiredEvidence.length) {
    html += `<div class="plan"><div class="planhead">What would need to be proven:</div>
      <ul>${plan.requiredEvidence.map((e) => `<li>${esc(e)}</li>`).join("")}</ul></div>`;
  }
  if (d.decision === "ALLOW")
    html += `<div class="note">All required predicates satisfied.</div>`;
  html += rawDetails("Raw decision JSON", d);
  if (allowGrant && d.decision === "UNKNOWN")
    html += `<button id="grant-btn" class="scn" style="margin-top:12px">Grant residence credential → re-evaluate</button>`;
  engine.innerHTML = `<div class="fade">${html}</div>`;
  const g = $("grant-btn");
  if (g) g.onclick = grantAndReeval;
}

async function grantAndReeval() {
  $("grant-btn").disabled = true;
  await api("POST", "/v1/wallet/grant", { subjectId: SUBJECT, evidenceType: "RESIDENCE_CREDENTIAL" });
  active("col-wallet");
  await sleep(250); active("col-engine");
  const d2 = await api("POST", "/v1/verification/decisions", { subjectId: SUBJECT, policy: "Can this user open a bank account?" });
  renderPolicyDecision(d2.data, false);
  renderHeld(d2.data);
}

async function runReplay() {
  proves("A captured VP cannot be replayed — the nonce is single-use.");
  resetPanels(); protoReset(); active("col-rp");
  const issue = await ensureIssued(); renderWalletIssued();
  const vc = issue.data && issue.data.vc;
  const q = await ask("Is this user over 18?");
  renderResolution(q.data);
  renderPipeline(pipeFromQuestionSuccess("Is this user over 18?", q.data));
  if (vc) protoIssuance(vc, "reuse");
  const p = await present(q.data);
  renderDisclosure(decodeDisclosures(p.data.sdJwtVp));
  if (vc) protoPresentation(p.data.sdJwtVp, vc.disclosures.length);
  active("col-engine");
  const first = await verify(q.data.requestId, p.data);
  $("verify-slot").insertAdjacentHTML("beforeend",
    `<div class="fade"><div class="kv"><span class="k">First presentation:</span> ${badge(first.data.decision)}</div></div>`);
  await protoVerification(first.data.verificationChecks, first.data.failedCheck);
  await sleep(400);
  const again = await verify(q.data.requestId, p.data);
  $("verify-slot").insertAdjacentHTML("beforeend",
    `<div class="fade"><div class="kv"><span class="k">Same VP re-presented:</span> ${badge(again.data.decision)}
      <span class="k">${esc(again.data.reason || "")}</span></div>
      <div class="note red">The identical VP is rejected: the nonce was already consumed.</div>
      ${rawDetails("Raw replay JSON", again.data)}</div>`);
}

async function runParaphrase() {
  proves("The AI maps a paraphrase to a canonical predicate, but the engine validates it — and the model never sees credentials.");
  resetPanels(); protoReset(); active("col-rp");
  const issue = await ensureIssued(); renderWalletIssued();
  const vc = issue.data && issue.data.vc;
  const q = await ask("Is this customer an adult?");
  if (!q.ok) return renderRefusal(q.data);
  renderResolution(q.data);
  renderPipeline(pipeFromQuestionSuccess("Is this customer an adult?", q.data));
  engine.querySelector(".fade").insertAdjacentHTML("beforeend", trustDiagram());
  if (vc) protoIssuance(vc, "reuse");
  await sleep(300); active("col-wallet");
  const p = await present(q.data);
  renderDisclosure(decodeDisclosures(p.data.sdJwtVp));
  if (vc) protoPresentation(p.data.sdJwtVp, vc.disclosures.length);
  await sleep(300); active("col-engine");
  const v = await verify(q.data.requestId, p.data);
  await renderDecision(v.data, { moneyShot: true });
  await protoVerification(v.data.verificationChecks, v.data.failedCheck);
  if (v.data.auditId) {
    const a = await api("GET", "/v1/audits/" + v.data.auditId);
    if (a.ok && a.data && a.data.llmProposal) {
      pipelineSetProposal(a.data.llmProposal);
      $("verify-slot").insertAdjacentHTML("beforeend",
        `<div class="fade"><div class="kv"><span class="k">LLM proposal (from audit):</span></div>
         <pre>${esc(JSON.stringify(a.data.llmProposal, null, 2))}</pre>
         <div class="note">Proposed id was re-validated against the closed set before use.</div></div>`);
    }
  }
}

function trustDiagram() {
  return `<div class="trust">
    <div class="box llm">LLM proposer — UNTRUSTED, outside the core.<br>Sees only the question + the catalogue. No PII, ever.</div>
    <div class="arrow">↓ proposes { canonical, confidence }</div>
    <div class="box gate">VALIDATION GATE — engine checks membership + HIGH confidence</div>
    <div class="arrow">↓ validated id only</div>
    <div class="box core">Deterministic core — resolution, verification, decision. Zero LLM calls.</div>
  </div>`;
}

async function runOutOfScope() {
  proves("Out-of-scope questions are refused, not approximated. The AI cannot introduce a new predicate.");
  resetPanels(); active("col-rp");
  protoNA("Question refused before any credential exchange — no VC/VP.");
  wallet.innerHTML = '<p class="idle">No credential involved — the request never resolves.</p>';
  const q = await ask("Can this user pilot a plane?");
  active("col-engine");
  if (q.ok) { renderResolution(q.data); renderPipeline(pipeFromQuestionSuccess("Can this user pilot a plane?", q.data)); return; }
  renderRefusal(q.data);
  renderPipeline(pipeFromRefusal("Can this user pilot a plane?", q.data));
}

function renderRefusal(data) {
  const list = (data && data.supportedQuestions) || [];
  engine.innerHTML = `<div class="fade">${badge("REFUSED")}
    <div class="note grey">The AI cannot invent a predicate. Only the closed set below is answerable.</div>
    <div class="kv"><span class="k">${esc((data && data.error) || "UNRESOLVED_QUESTION")}</span></div>
    <ul class="supported">${list.map((s) => `<li><code>${esc(s)}</code></li>`).join("")}</ul>
    ${rawDetails("Raw 422 JSON", data)}</div>`;
}

async function runTamper() {
  proves("A one-character change to a disclosure is caught by the cryptographic digest check.");
  resetPanels(); protoReset(); active("col-rp");
  const issue = await ensureIssued(); renderWalletIssued();
  const vc = issue.data && issue.data.vc;
  const q = await ask("Is this user over 18?");
  renderResolution(q.data);
  renderPipeline(pipeFromQuestionSuccess("Is this user over 18?", q.data));
  if (vc) protoIssuance(vc, "full");
  await sleep(250); active("col-wallet");
  const p = await present(q.data);
  renderDisclosure(decodeDisclosures(p.data.sdJwtVp));
  wallet.insertAdjacentHTML("beforeend", `<div class="note red">Attacker flips one character in the birth_date disclosure…</div>`);
  const tampered = Object.assign({}, p.data, { sdJwtVp: tamperVp(p.data.sdJwtVp) });
  if (vc) protoPresentation(tampered.sdJwtVp, vc.disclosures.length);
  await sleep(300); active("col-engine");
  const v = await verify(q.data.requestId, tampered);
  await renderDecision(v.data);
  await protoVerification(v.data.verificationChecks, v.data.failedCheck);
}

async function runFreeText() {
  const q = $("q").value.trim();
  if (!q) return;
  proves("Your question is routed through the same resolution API — it resolves to a canonical id, or is refused.");
  resetPanels(); active("col-rp");
  protoNA("Free-text resolution only — no VC/VP exchanged.");
  wallet.innerHTML = '<p class="idle">Free-text resolution only (no presentation).</p>';
  const r = await ask(q);
  active("col-engine");
  if (r.ok) {
    renderResolution(r.data);
    renderPipeline(pipeFromQuestionSuccess(q, r.data));
    $("verify-slot").insertAdjacentHTML("beforeend",
      `<div class="fade"><div class="note">Resolved to a member of the closed set — the flow could now request evidence.</div></div>`);
  } else {
    renderRefusal(r.data);
    renderPipeline(pipeFromRefusal(q, r.data));
  }
}

// ---- wiring ---------------------------------------------------------------

const RUNNERS = { age: runAge, bank: runBank, replay: runReplay, paraphrase: runParaphrase, oos: runOutOfScope, tamper: runTamper };

function lock(on) { document.querySelectorAll(".scn,#ask").forEach((b) => { if (b.id !== "grant-btn") b.disabled = on; }); }

async function guard(fn) {
  lock(true);
  try { await fn(); }
  catch (e) { engine.innerHTML = `<div class="note red">Engine unreachable — is the stack running? (${esc(e.message || e)})</div>`; }
  finally { lock(false); }
}

document.querySelectorAll(".scn[data-run]").forEach((btn) =>
  btn.addEventListener("click", () => guard(RUNNERS[btn.dataset.run])));
$("ask").addEventListener("click", () => guard(runFreeText));
$("q").addEventListener("keydown", (e) => { if (e.key === "Enter") guard(runFreeText); });

// protocol panel: show-full toggle + copy-to-clipboard (event delegation)
document.getElementById("protocol").addEventListener("click", (e) => {
  const t = e.target.closest("[data-toggle]");
  if (t) { const el = document.getElementById(t.dataset.toggle); const hidden = el.classList.toggle("hidden"); t.textContent = hidden ? "show full" : "hide"; return; }
  const c = e.target.closest("[data-copy]");
  if (c) { const el = document.getElementById(c.dataset.copy); copyText(el.textContent, c); }
});
function copyText(text, btn) {
  const done = () => { const o = "copy"; btn.textContent = "copied"; btn.classList.add("copied"); setTimeout(() => { btn.textContent = o; btn.classList.remove("copied"); }, 1200); };
  if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(done).catch(() => fallbackCopy(text, done));
  else fallbackCopy(text, done);
}
function fallbackCopy(text, done) {
  const ta = document.createElement("textarea"); ta.value = text; ta.style.position = "fixed"; ta.style.opacity = "0";
  document.body.appendChild(ta); ta.select();
  try { document.execCommand("copy"); } catch (_) {}
  document.body.removeChild(ta); done();
}

// initial connectivity probe (renders offline banner if the stack is down)
api("GET", "/v1/audits/__probe__").catch(() => {});
fetchCatalogue();
