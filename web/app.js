const form = document.querySelector("#command-form");
const input = document.querySelector("#command-input");
const button = document.querySelector("#run-button");
const output = document.querySelector("#output");
const responseMeta = document.querySelector("#response-meta");
const password = document.querySelector("#password-input");
const statusText = document.querySelector("#status-text");
const statusDot = document.querySelector("#status-dot");
const healthDot = document.querySelector("#health-dot");
const healthLabel = document.querySelector("#health-label");
const healthDetail = document.querySelector("#health-detail");
let busy = false;

function setStatus(connected, detail) {
  statusDot.className = `dot ${connected ? "good" : "bad"}`;
  statusText.textContent = connected ? "Connected" : "Disconnected";
  healthDot.className = `dot ${connected ? "good" : "bad"}`;
  healthLabel.textContent = connected ? "River is online" : "Connection unavailable";
  healthDetail.textContent = detail;
}

async function runCommand(command) {
  if (busy || !command.trim()) return;
  busy = true;
  button.disabled = true;
  button.firstElementChild.textContent = "Running…";
  responseMeta.textContent = "Working";
  output.className = "output";
  output.textContent = "Waiting for River…";
  try {
    const response = await fetch("/api/command", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command, password: password.value }),
    });
    const data = await response.json();
    if (!response.ok && response.status !== 422) throw new Error(data.error || `Request failed (${response.status})`);
    setStatus(true, "The embedded API reached the active River store.");
    output.className = `output ${data.ok ? "success" : "error"}`;
    output.textContent = data.ok ? (data.kind === "null" ? "(nil)" : data.response ?? "(nil)") : data.error;
    responseMeta.textContent = data.ok ? "Success" : "Command error";
  } catch (error) {
    setStatus(false, error.message || "Could not reach the River server.");
    output.className = "output error";
    output.textContent = error.message || "Request failed. Check the server connection.";
    responseMeta.textContent = "Request failed";
  } finally {
    busy = false;
    button.disabled = false;
    button.firstElementChild.textContent = "Run command";
  }
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  runCommand(input.value);
});
document.querySelectorAll("[data-command]").forEach((item) => item.addEventListener("click", () => {
  input.value = item.dataset.command;
  input.focus();
  runCommand(input.value);
}));
document.querySelector("#clear-button").addEventListener("click", () => {
  output.className = "output empty";
  output.innerHTML = '<span class="output-glyph">↳</span><span>Run a command to see its response here.</span>';
  responseMeta.textContent = "Ready";
});
runCommand("HEALTH");

const consoleView = document.querySelector("#console-view");
const docsView = document.querySelector("#docs-view");
const navConsole = document.querySelector("#nav-console");
const navDocs = document.querySelector("#nav-docs");
const docsNav = document.querySelector("#docs-nav");
const docsContent = document.querySelector("#docs-content");
const docsSearch = document.querySelector("#docs-search");
let documentation = [];
let activeDoc = "getting-started";
const commandHistory = [];
let historyIndex = 0;

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character]);
}
function navigate(view) {
  const docs = view === "docs";
  consoleView.hidden = docs;
  docsView.hidden = !docs;
  navDocs.classList.toggle("active", docs);
  navConsole.classList.toggle("active", !docs);
  if (docs) renderDoc(activeDoc, docsSearch.value);
  else input.focus();
}
function sectionMatches(section, query) {
  if (!query) return true;
  return `${section.title} ${section.blocks.map((block) => JSON.stringify(block)).join(" ")}`.toLowerCase().includes(query.toLowerCase());
}
function renderNav(query = "") {
  const matches = documentation.filter((section) => sectionMatches(section, query));
  if (!matches.length) {
    docsNav.innerHTML = '<p class="docs-no-results">No sections match your search.</p>';
    return;
  }
  const groups = [...new Set(matches.map((section) => section.group))];
  docsNav.innerHTML = groups.map((group) => `<div class="docs-nav-group"><span>${escapeHtml(group)}</span>${matches.filter((section) => section.group === group).map((section) => `<button type="button" class="docs-nav-item ${section.id === activeDoc ? "active" : ""}" data-doc="${escapeHtml(section.id)}">${escapeHtml(section.title)}</button>`).join("")}</div>`).join("");
  docsNav.querySelectorAll("[data-doc]").forEach((button) => button.addEventListener("click", () => renderDoc(button.dataset.doc, docsSearch.value)));
}
function renderBlock(block) {
  if (block.type === "p") return `<p>${escapeHtml(block.text)}</p>`;
  if (block.type === "h2") return `<h2>${escapeHtml(block.text)}</h2>`;
  if (block.type === "code") return `<div class="doc-code"><div class="doc-code-head"><span>${escapeHtml(block.lang || "text")}</span><button type="button" class="copy-button" data-copy="${escapeHtml(block.text)}">Copy</button></div><pre><code>${escapeHtml(block.text)}</code></pre></div>`;
  if (block.type === "table") return `<div class="doc-table-wrap"><table><thead><tr>${block.headers.map((cell) => `<th>${escapeHtml(cell)}</th>`).join("")}</tr></thead><tbody>${block.rows.map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`;
  if (block.type === "command") return `<section class="command-doc"><div class="command-doc-title"><span class="section-label">COMMAND</span><h2>${escapeHtml(block.name)}</h2></div><code class="command-syntax">${escapeHtml(block.syntax)}</code><p>${escapeHtml(block.description)}</p><div class="doc-code"><div class="doc-code-head"><span>EXAMPLE</span><div><button type="button" class="copy-button" data-copy="${escapeHtml(block.example)}">Copy</button><button type="button" class="try-button" data-try="${escapeHtml(block.example)}">Try in Console</button></div></div><pre><code>${escapeHtml(block.example)}</code></pre></div><div class="command-response"><strong>Response</strong><code>${escapeHtml(block.response)}</code></div><p class="command-errors"><strong>Errors and notes</strong><br>${escapeHtml(block.errors)}</p></section>`;
  return "";
}
function renderDoc(id, query = "") {
  const section = documentation.find((item) => item.id === id);
  if (!section) return;
  activeDoc = id;
  renderNav(query);
  const blocks = section.blocks.filter((block) => !query || JSON.stringify(block).toLowerCase().includes(query.toLowerCase()) || section.title.toLowerCase().includes(query.toLowerCase()));
  if (!blocks.length) {
    docsContent.innerHTML = `<header class="doc-title"><p class="section-label">${escapeHtml(section.group)}</p><h1>${escapeHtml(section.title)}</h1></header><div class="docs-empty"><strong>No matching content</strong><p>Try another word or clear your search.</p></div>`;
    return;
  }
  docsContent.innerHTML = `<header class="doc-title"><p class="section-label">${escapeHtml(section.group)}</p><h1>${escapeHtml(section.title)}</h1></header>${blocks.map(renderBlock).join("")}`;
  docsContent.querySelectorAll("[data-copy]").forEach((button) => button.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(button.dataset.copy); button.textContent = "Copied"; setTimeout(() => { button.textContent = "Copy"; }, 1200); }
    catch { button.textContent = "Select and copy"; }
  }));
  docsContent.querySelectorAll("[data-try]").forEach((button) => button.addEventListener("click", () => {
    input.value = button.dataset.try;
    history.replaceState(null, "", "#console");
    navigate("console");
    input.focus();
  }));
}

window.addEventListener("hashchange", () => navigate(location.hash === "#docs" ? "docs" : "console"));
navConsole.addEventListener("click", () => navigate("console"));
navDocs.addEventListener("click", () => navigate("docs"));
docsSearch.addEventListener("input", () => {
  const matches = documentation.filter((section) => sectionMatches(section, docsSearch.value));
  renderNav(docsSearch.value);
  const chosen = matches.find((section) => section.id === activeDoc) || matches[0];
  if (chosen) renderDoc(chosen.id, docsSearch.value);
  else docsContent.innerHTML = '<div class="docs-empty"><strong>No documentation found</strong><p>Try a different search term.</p></div>';
});
document.addEventListener("keydown", (event) => {
  if (event.key === "/" && !["INPUT", "TEXTAREA"].includes(document.activeElement.tagName)) {
    event.preventDefault();
    navigate("docs");
    docsSearch.focus();
  }
  if (event.key === "Escape" && document.activeElement === docsSearch) {
    docsSearch.value = "";
    docsSearch.dispatchEvent(new Event("input"));
    docsSearch.blur();
  }
  if (event.key === "ArrowUp" && document.activeElement === input && !input.value && historyIndex > 0) input.value = commandHistory[--historyIndex];
  if (event.key === "ArrowDown" && document.activeElement === input && historyIndex < commandHistory.length) input.value = commandHistory[++historyIndex] || "";
});
form.addEventListener("submit", () => {
  const command = input.value.trim();
  if (command && commandHistory.at(-1) !== command) commandHistory.push(command);
  historyIndex = commandHistory.length;
});
fetch("/docs.json").then((response) => {
  if (!response.ok) throw new Error("Documentation asset unavailable");
  return response.json();
}).then((data) => {
  documentation = data.sections;
  renderDoc(activeDoc);
  if (location.hash === "#docs") navigate("docs");
}).catch(() => {
  docsContent.innerHTML = '<div class="docs-empty"><strong>Documentation could not load</strong><p>Reload the page to try again.</p></div>';
});
