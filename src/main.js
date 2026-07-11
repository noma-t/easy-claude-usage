const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const appRoot = document.querySelector("#app");
const headerElement = document.querySelector("#header");
const usagePanel = document.querySelector("#usage-panel");
const usageContent = document.querySelector("#usage-content");
const errorMessage = document.querySelector("#error-message");
const lastUpdatedElement = document.querySelector("#last-updated");

// ウィンドウの実サイズ(#app)ではなく、中身の自然な高さ(#usage-panelのscrollHeight)を
// 基準にリサイズする。#usage-panelはflex:1でウィンドウ高に引き伸ばされているため、
// scrollHeightは表示中の高さに関わらず実コンテンツの高さを返す。
const APP_BORDER_HEIGHT = 2; // #appのborder-top/bottom(各1px)

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

// ラベルごとに生成済みのメーター行を保持し、更新時はDOM要素を使い回すことで
// CSSトランジションとパーセント数値のカウントアニメーションを機能させる。
const meterRowsByLabel = new Map();

const PERCENT_ANIMATION_DURATION_MS = 500;

function formatPercent(percent) {
  return Math.round(percent * 10) / 10;
}

function animatePercentValue(entry, fromPercent, toPercent) {
  cancelAnimationFrame(entry.animationFrameId);

  if (fromPercent === toPercent) {
    entry.value.textContent = `${formatPercent(toPercent)}%`;
    return;
  }

  const startTime = performance.now();

  const step = (now) => {
    const t = Math.min(1, (now - startTime) / PERCENT_ANIMATION_DURATION_MS);
    const eased = 1 - Math.pow(1 - t, 3); // ease-out cubic
    const current = fromPercent + (toPercent - fromPercent) * eased;
    entry.value.textContent = `${formatPercent(current)}%`;

    if (t < 1) {
      entry.animationFrameId = requestAnimationFrame(step);
    } else {
      entry.value.textContent = `${formatPercent(toPercent)}%`;
    }
  };

  entry.animationFrameId = requestAnimationFrame(step);
}

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
  value.textContent = `${formatPercent(bar.percent)}%`;

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

  let resets = null;
  if (bar.resets) {
    resets = document.createElement("div");
    resets.className = "usage-meter-resets";
    resets.textContent = bar.resets;
    row.appendChild(resets);
  }

  return {
    row,
    fill,
    track,
    value,
    resets,
    percent: bar.percent,
    animationFrameId: null,
  };
}

function updateMeterRow(entry, bar) {
  const severity = severityForPercent(bar.percent);
  entry.row.style.setProperty("--meter-color", METER_COLOR_BY_SEVERITY[severity]);

  entry.fill.style.width = `${bar.percent}%`;
  entry.track.setAttribute("aria-valuenow", String(bar.percent));

  animatePercentValue(entry, entry.percent, bar.percent);
  entry.percent = bar.percent;

  if (bar.resets) {
    if (!entry.resets) {
      entry.resets = document.createElement("div");
      entry.resets.className = "usage-meter-resets";
      entry.row.appendChild(entry.resets);
    }
    entry.resets.textContent = bar.resets;
  } else if (entry.resets) {
    entry.resets.remove();
    entry.resets = null;
  }
}

function renderUsage(text) {
  const bars = parseUsageText(text);

  let container = usageContent.querySelector("#usage-meters");
  if (!container) {
    usageContent.innerHTML = "";
    container = document.createElement("div");
    container.id = "usage-meters";
    usageContent.appendChild(container);
  }

  const seenLabels = new Set();
  let previousRow = null;

  for (const bar of bars) {
    seenLabels.add(bar.label);
    let entry = meterRowsByLabel.get(bar.label);

    if (entry) {
      updateMeterRow(entry, bar);
    } else {
      entry = buildMeterRow(bar);
      meterRowsByLabel.set(bar.label, entry);
    }

    // 既に正しい位置にある行はappendChildで動かさない。再アペンドすると
    // ブラウザがノードの再挿入とみなしCSSトランジションが発火しなくなるため。
    const expectedNextNode = previousRow ? previousRow.nextSibling : container.firstChild;
    if (entry.row !== expectedNextNode) {
      container.insertBefore(entry.row, expectedNextNode);
    }
    previousRow = entry.row;
  }

  for (const [label, entry] of meterRowsByLabel) {
    if (!seenLabels.has(label)) {
      cancelAnimationFrame(entry.animationFrameId);
      entry.row.remove();
      meterRowsByLabel.delete(label);
    }
  }
}

function renderLastUpdated(date) {
  const time = date.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  lastUpdatedElement.textContent = `最終更新: ${time}`;
}

function resizeToContent() {
  const contentHeight = headerElement.offsetHeight + usagePanel.scrollHeight + APP_BORDER_HEIGHT;
  invoke("resize_window", { height: contentHeight }).catch((err) => console.error(err));
}

function setRefreshing(isRefreshing) {
  if (isRefreshing) {
    appRoot.dataset.refreshing = "true";
  } else {
    delete appRoot.dataset.refreshing;
  }
}

async function refreshUsage() {
  const isInitialLoad = !hasContent;

  if (isInitialLoad) {
    // まだ一度も表示できていない場合のみ、全体をローディング表示にする。
    appRoot.dataset.state = "loading";
    resizeToContent();
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
    resizeToContent();
  } catch (err) {
    if (isInitialLoad) {
      errorMessage.textContent = String(err);
      appRoot.dataset.state = "error";
      resizeToContent();
    } else {
      console.error(err);
    }
  } finally {
    setRefreshing(false);
  }
}

listen("usage://refresh", refreshUsage);

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    invoke("hide_window");
  }
});
