const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const refreshItem = document.querySelector("#menu-item-refresh");
const updateItem = document.querySelector("#menu-item-update");
const autostartItem = document.querySelector("#menu-item-autostart");
const quitItem = document.querySelector("#menu-item-quit");

function applyMenuState(state) {
  updateItem.textContent = state.updateLabel;
  updateItem.disabled = state.updateBusy;
  autostartItem.setAttribute("aria-checked", String(state.autostartEnabled));
}

refreshItem.addEventListener("click", () => {
  invoke("menu_refresh");
});

updateItem.addEventListener("click", () => {
  invoke("menu_update_action");
});

autostartItem.addEventListener("click", async () => {
  const enabled = await invoke("menu_toggle_autostart");
  autostartItem.setAttribute("aria-checked", String(enabled));
});

quitItem.addEventListener("click", () => {
  invoke("menu_quit");
});

listen("tray-context-menu://state", (event) => applyMenuState(event.payload));

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    invoke("hide_window");
  }
});
