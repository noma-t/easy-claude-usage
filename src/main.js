const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { check } = window.__TAURI__.updater;
const { relaunch } = window.__TAURI__.process;

const appRoot = document.querySelector("#app");
const usageContent = document.querySelector("#usage-content");
const errorMessage = document.querySelector("#error-message");
const headerStatusIndicator = document.querySelector("#header-status-indicator");
const lastUpdatedElement = document.querySelector("#last-updated");
const updateButton = document.querySelector("#update-button");

// 「Current session: 34% used · resets Jul 11, 3:30am (Asia/Tokyo)」のような行を拾う。
// ラベル部分(session / week (all models) / week (Fable) など)を固定リストで持たず、
// 「Current 〜: n% used」パターンに一致する行はすべて動的にメーター化する。
const CURRENT_USAGE_LINE_RE = /^Current\s+(.+?):\s*(\d+(?:\.\d+)?)%\s*used\b\s*(?:[·•\-–—]\s*(.*))?$/;

const METER_COLOR_BY_SEVERITY = {
  normal: "var(--color-accent)",
  warning: "var(--status-warning)",
  critical: "var(--status-critical)",
};

let hasContent = false;
let errorFlashTimer = null;

function severityForPercent(percent) {
  if (percent >= 90) return "critical";
  if (percent >= 70) return "warning";
  return "normal";
}

// 「Current ... % used」行だけを拾い、それ以外の行(見出し文や「What's contributing...」
// セクションなど)は破棄する。
function parseUsageText(text) {
  const bars = [];

  for (const line of text.split(/\r?\n/)) {
    const match = line.match(CURRENT_USAGE_LINE_RE);
    if (match) {
      bars.push({
        label: match[1].trim(),
        percent: Math.max(0, Math.min(100, Number(match[2]))),
        resets: (match[3] || "").trim(),
      });
    }
  }

  return bars;
}

function buildMeterRow(bar) {
  const severity = severityForPercent(bar.percent);

  const row = document.createElement("div");
  row.className = "usage-meter";
  row.style.setProperty("--meter-color", METER_COLOR_BY_SEVERITY[severity]);

  const head = document.createElement("div");
  head.className = "usage-meter-head";

  const label = document.createElement("span");
  label.className = "usage-meter-label";
  label.textContent = bar.label;

  const value = document.createElement("span");
  value.className = "usage-meter-value";
  value.textContent = `${bar.percent}%`;

  head.append(label, value);

  const track = document.createElement("div");
  track.className = "usage-meter-track";
  track.setAttribute("role", "progressbar");
  track.setAttribute("aria-valuenow", String(bar.percent));
  track.setAttribute("aria-valuemin", "0");
  track.setAttribute("aria-valuemax", "100");
  track.setAttribute("aria-label", bar.label);

  const fill = document.createElement("div");
  fill.className = "usage-meter-fill";
  fill.style.width = `${bar.percent}%`;
  track.appendChild(fill);

  row.append(head, track);

  if (bar.resets) {
    const resets = document.createElement("div");
    resets.className = "usage-meter-resets";
    resets.textContent = bar.resets;
    row.appendChild(resets);
  }

  return row;
}

function renderUsage(text) {
  const bars = parseUsageText(text);
  usageContent.innerHTML = "";

  const container = document.createElement("div");
  container.id = "usage-meters";
  for (const bar of bars) {
    container.appendChild(buildMeterRow(bar));
  }
  usageContent.appendChild(container);
}

function renderLastUpdated(date) {
  const time = date.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  lastUpdatedElement.textContent = `最終更新: ${time}`;
}

function setRefreshing(isRefreshing) {
  if (isRefreshing) {
    appRoot.dataset.refreshing = "true";
  } else {
    delete appRoot.dataset.refreshing;
  }
}

function flashHeaderError() {
  headerStatusIndicator.classList.add("has-error");
  clearTimeout(errorFlashTimer);
  errorFlashTimer = setTimeout(() => headerStatusIndicator.classList.remove("has-error"), 4000);
}

async function refreshUsage() {
  const isInitialLoad = !hasContent;

  if (isInitialLoad) {
    // まだ一度も表示できていない場合のみ、全体をローディング表示にする。
    appRoot.dataset.state = "loading";
  } else {
    // 既に表示済みの内容はそのまま残し、端のスピナーだけで取得中を示す。
    setRefreshing(true);
  }

  try {
    const text = await invoke("get_usage");
    renderUsage(text);
    renderLastUpdated(new Date());
    hasContent = true;
    appRoot.dataset.state = "ready";
  } catch (err) {
    if (isInitialLoad) {
      errorMessage.textContent = String(err);
      appRoot.dataset.state = "error";
    } else {
      console.error(err);
      flashHeaderError();
    }
  } finally {
    setRefreshing(false);
  }
}

listen("usage://refresh", refreshUsage);

let pendingUpdate = null;

async function checkForUpdate({ silent } = {}) {
  if (appRoot.dataset.updateState === "downloading") return;

  try {
    const update = await check();
    if (update) {
      pendingUpdate = update;
      updateButton.title = `新しいバージョン ${update.version} が利用可能です`;
      appRoot.dataset.updateState = "available";
      return;
    }

    pendingUpdate = null;
    delete appRoot.dataset.updateState;
    if (!silent) {
      updateButton.title = "最新版です";
      setTimeout(() => {
        if (!pendingUpdate) updateButton.removeAttribute("title");
      }, 3000);
    }
  } catch (err) {
    console.error(err);
    flashHeaderError();
  }
}

updateButton.addEventListener("click", async () => {
  if (!pendingUpdate) return;

  appRoot.dataset.updateState = "downloading";
  let downloaded = 0;
  let total = 0;

  try {
    await pendingUpdate.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        updateButton.title =
          total > 0 ? `ダウンロード中... ${Math.round((downloaded / total) * 100)}%` : "ダウンロード中...";
      } else if (event.event === "Finished") {
        updateButton.title = "再起動しています...";
      }
    });
    await relaunch();
  } catch (err) {
    console.error(err);
    delete appRoot.dataset.updateState;
    flashHeaderError();
  }
});

listen("update://check", () => checkForUpdate({ silent: false }));
checkForUpdate({ silent: true });

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    invoke("hide_window");
  }
});
