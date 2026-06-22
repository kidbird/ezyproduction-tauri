// ─── Firmware Download Page ───────────────────────────────────────────────
// Drives the r26-cli sidecar via Tauri events. Uses raw `invoke` (not
// `tauriInvoke`) because progress is reported through `firmware-event` and a
// modal loading overlay would conflict with the inline progress UI.

const { listen } = window.__TAURI__.event;

const fw = {
  pacPath: null,
  report: null,
  downloading: false,
};

const RISK_LABEL = {
  Safe: ['安全', 'ok'],
  NvWrite: ['NV', 'warn'],
  RfCalibration: ['RF校准', 'err'],
  Erase: ['擦除', 'err'],
  EraseAll: ['全盘擦除', 'err'],
  PhaseCheck: ['PhaseCheck', 'info'],
};

function fwLog(line) {
  const el = document.getElementById('fwLog');
  if (el.dataset.empty !== 'false') { el.textContent = ''; el.dataset.empty = 'false'; }
  el.textContent += (el.textContent ? '\n' : '') + line;
  el.scrollTop = el.scrollHeight;
}

function fwRenderReport(report) {
  const alertEl = document.getElementById('fwPacAlert');
  alertEl.innerHTML = `<div class="alert alert-success">✓ PAC 加载完成（${report.total_files} 个文件）</div>`;

  const chips = document.getElementById('fwSafetyChips');
  const eraseOn = report.erase_files.length > 0;
  const nvOn = report.nv_files.length > 0;
  const rfBlocked = report.touches_rf_calibration || report.rf_cali_files.length > 0;
  const pcBlocked = report.phasecheck_files.length > 0;
  chips.innerHTML =
    `<span class="chip ${eraseOn ? 'warn' : 'ok'}">擦除: ${eraseOn ? '将执行' : '无'}</span>` +
    `<span class="chip ${nvOn ? 'warn' : 'ok'}">NV写入: ${nvOn ? '将执行(保留校准)' : '无'}</span>` +
    `<span class="chip ${rfBlocked ? 'err' : 'ok'}">射频校准: ${rfBlocked ? '含校准·已禁止' : '已保护'}</span>` +
    (pcBlocked ? `<span class="chip err">PhaseCheck: 含数据·已禁止</span>` : '');

  const rows = [];
  report.safe_files.forEach(id => rows.push({ id, risk: 'Safe', reason: '' }));
  [...report.nv_files, ...report.rf_cali_files, ...report.erase_files, ...report.phasecheck_files]
    .forEach(f => rows.push({ id: f.file_id, risk: f.risk_level, reason: f.reason }));
  const body = rows.map(r => {
    const [label, cls] = RISK_LABEL[r.risk] || [escHtml(r.risk) + ' ?', 'warn'];
    return `<tr><td class="id">${escHtml(r.id)}</td><td><span class="chip ${cls}" title="${escHtml(r.reason)}">${label}</span></td></tr>`;
  }).join('');
  document.getElementById('fwRiskWrap').innerHTML =
    `<details><summary style="cursor:pointer; font-size:12.5px; color:var(--text-secondary);">文件列表 (${report.total_files})</summary>` +
    `<table class="risk-table"><thead><tr><th>文件 ID</th><th>风险</th></tr></thead><tbody>${body}</tbody></table></details>`;

  const blocked = rfBlocked || pcBlocked;
  document.getElementById('fwStartBtn').disabled = blocked || fw.downloading;
  if (blocked) {
    alertEl.innerHTML += `<div class="alert alert-error" style="margin-top:8px;">此 PAC 含受保护分区（射频校准 / PhaseCheck），出于保护已禁止刷写。</div>`;
  }
}

function fwSetDownloading(on) {
  fw.downloading = on;
  document.getElementById('fwStartBtn').disabled = on || !fw.report;
  document.getElementById('fwStopBtn').disabled = !on;
  document.getElementById('fwSelectPacBtn').disabled = on;
  document.getElementById('fwStartBtn').textContent = on ? '下载中…' : '开始下载';
}

function fwSetResult(html) {
  document.getElementById('fwResult').innerHTML = html;
}

function fwSetProgress(label, pct) {
  const wrap = document.getElementById('fwProgressWrap');
  if (pct == null) { wrap.style.display = 'none'; return; }
  wrap.style.display = 'block';
  document.getElementById('fwProgressLabel').textContent = label;
  document.getElementById('fwProgressPct').textContent = `${Math.round(pct)}%`;
  document.getElementById('fwProgressFill').style.width = `${Math.round(pct)}%`;
}

// Select + analyze a PAC.
document.getElementById('fwSelectPacBtn').addEventListener('click', async () => {
  const selBtn = document.getElementById('fwSelectPacBtn');
  let path;
  try {
    path = await invoke('pick_pac_file');
  } catch (e) {
    document.getElementById('fwPacAlert').innerHTML = `<div class="alert alert-error">分析失败: ${escHtml(e)}</div>`;
    return;
  }
  if (!path) return;
  selBtn.disabled = true;
  try {
    fw.pacPath = path;
    fw.report = null;
    document.getElementById('fwPacPath').textContent = path;
    document.getElementById('fwPacAlert').innerHTML = '<div class="alert alert-info">正在分析 PAC…</div>';
    document.getElementById('fwRiskWrap').innerHTML = '';
    document.getElementById('fwStartBtn').disabled = true;
    const report = await invoke('pac_info', { path });
    fw.report = report;
    fwRenderReport(report);
  } catch (e) {
    document.getElementById('fwPacAlert').innerHTML = `<div class="alert alert-error">分析失败: ${escHtml(e)}</div>`;
  } finally {
    selBtn.disabled = fw.downloading;
  }
});

// Start download.
document.getElementById('fwStartBtn').addEventListener('click', async () => {
  if (!fw.pacPath) { showToast('请先选择 PAC 文件', 'error'); return; }
  try {
    document.getElementById('fwLog').textContent = '';
    document.getElementById('fwLog').dataset.empty = 'false';
    fwSetResult('');
    fwSetDownloading(true);
    fwSetProgress('等待设备…', 0);
    await invoke('start_firmware_download', { path: fw.pacPath });
  } catch (e) {
    fwSetDownloading(false);
    fwSetProgress(null);
    fwSetResult(`<div class="alert alert-error">${e}</div>`);
  }
});

// Stop download.
document.getElementById('fwStopBtn').addEventListener('click', async () => {
  try {
    await invoke('stop_firmware_download');
    fwSetDownloading(false);
    fwSetProgress(null);
    fwSetResult('<div class="alert alert-info">下载已手动停止</div>');
  } catch (e) {
    showToast(`停止失败: ${e}`, 'error');
  }
});

// Forwarded sidecar events.
listen('firmware-event', (ev) => {
  const p = ev.payload || {};
  if (p.Log) {
    fwLog(`[${p.Log.level}] ${p.Log.message}`);
  } else if (p.Progress) {
    fwSetDownloading(true);
    fwSetProgress(p.Progress.file_id, p.Progress.percent);
  } else if (p.PacLoadProgress) {
    fwSetProgress('加载 PAC…', p.PacLoadProgress.percent);
  } else if (p.StateChange) {
    fwLog(`[状态] ${p.StateChange.from} → ${p.StateChange.to}`);
  } else if (p.Completed) {
    const r = p.Completed.result;
    fwSetDownloading(false);
    fwSetProgress(null);
    fwSetResult(r.success
      ? `<div class="alert alert-success">✅ 下载完成！耗时 ${Math.round((r.duration_ms || 0) / 1000)} 秒</div>`
      : `<div class="alert alert-error">❌ 下载失败: ${escHtml(r.error || '未知错误')}</div>`);
  } else if (p.Error) {
    fwSetDownloading(false);
    fwSetProgress(null);
    fwSetResult(`<div class="alert alert-error">❌ 错误 [${p.Error.code}]: ${escHtml(p.Error.message)}</div>`);
  } else if (p.Terminated) {
    if (fw.downloading) {
      fwSetDownloading(false);
      fwSetProgress(null);
      if (p.Terminated.code && p.Terminated.code !== 0) {
        fwSetResult(`<div class="alert alert-error">刷机进程异常退出 (code ${p.Terminated.code})</div>`);
      }
    }
  }
});
