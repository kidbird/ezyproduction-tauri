// ─── State ────────────────────────────────────────────────────────────────
// Single source of truth for the UI. All page scripts read/write here.
const state = {
  connected: false,
  deviceIp: '192.168.42.1',
  baseData: null,
  currentProduct: null,
  currentSn: '',
  deviceInfo: null,
  records: [],
};

// ─── Tauri Invoke Helpers ─────────────────────────────────────────────────
// Two intentional flavors (see docs/ARCHITECTURE.md §4):
//   tauriInvoke — auto loading overlay + error toast; default for event-driven calls.
//   invoke      — raw; used by long flows (firmware) that manage their own progress UI.
const { invoke } = window.__TAURI__.core;

async function tauriInvoke(cmd, args = {}) {
  try {
    showLoading();
    return await invoke(cmd, args);
  } catch (error) {
    showToast(error, 'error');
    throw error;
  } finally {
    hideLoading();
  }
}

// ─── UI Helpers ───────────────────────────────────────────────────────────
function showToast(message, type = 'info') {
  const container = document.getElementById('toastContainer');
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.textContent = message;
  container.appendChild(toast);
  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateX(100%)';
    toast.style.transition = 'all 0.2s ease';
    setTimeout(() => toast.remove(), 200);
  }, 3000);
}

function showLoading() {
  document.getElementById('loadingOverlay').classList.add('active');
}

function hideLoading() {
  document.getElementById('loadingOverlay').classList.remove('active');
}

// HTML escape for any value interpolated into innerHTML.
// Keep in sync with the same helper used in firmware.js.
function escHtml(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}

function updateConnectionStatus(connected) {
  state.connected = connected;
  const dot = document.getElementById('statusDot');
  const text = document.getElementById('statusText');
  if (connected) {
    dot.classList.add('connected');
    text.textContent = '已连接';
  } else {
    dot.classList.remove('connected');
    text.textContent = '未连接';
  }
}

function updateSnDisplay(sn) {
  state.currentSn = sn;
  document.getElementById('snDisplay').textContent = sn || '------------';
}

function updateDeviceInfo(info) {
  state.deviceInfo = info;
  document.getElementById('imeiValue').textContent = info.imei || '-';
  document.getElementById('iccidValue').textContent = info.iccid || '-';
  document.getElementById('deviceSnValue').textContent = info.sn || '-';
  document.getElementById('swVersionValue').textContent = info.sw_version || '-';
  document.getElementById('deviceNameValue').textContent = info.device_name || '-';
  document.getElementById('activateStatusValue').textContent = info.activated ? '已激活' : '未激活';
}

// ─── Navigation ───────────────────────────────────────────────────────────
document.querySelectorAll('.nav-item').forEach(item => {
  item.addEventListener('click', () => {
    if (item.classList.contains('disabled')) return;
    document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
    item.classList.add('active');
    const page = item.dataset.page;
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.getElementById(`page-${page}`).classList.add('active');
  });
});

// ─── Theme Toggle ─────────────────────────────────────────────────────────
document.getElementById('themeToggle').addEventListener('click', () => {
  const html = document.documentElement;
  const current = html.getAttribute('data-theme');
  html.setAttribute('data-theme', current === 'dark' ? 'light' : 'dark');
});
