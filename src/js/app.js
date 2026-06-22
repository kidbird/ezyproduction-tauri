// ─── Initialization ───────────────────────────────────────────────────────
async function initApp() {
  try {
    await invoke('init_app');

    let baseData;
    try {
      baseData = await invoke('get_base_data');
    } catch (e) {
      console.error('get_base_data failed:', e);
      showToast('加载配置失败: ' + e, 'error');
      return;
    }
    if (!baseData || !baseData.brands) {
      showToast('配置数据格式错误', 'error');
      return;
    }
    state.baseData = baseData;
    populateSelects(baseData);
    renderConfigTables(baseData);

    let product;
    try {
      product = await invoke('get_current_product');
    } catch (e) {
      console.error('get_current_product failed:', e);
      product = { brand: '', product_type: '', fac: '' };
    }
    state.currentProduct = product;

    const brandSelect = document.getElementById('brandSelect');
    const typeSelect = document.getElementById('typeSelect');
    const factorySelect = document.getElementById('factorySelect');
    if (product.Brand ?? product.brand) brandSelect.value = product.Brand ?? product.brand;
    if (product.Type ?? product.product_type) typeSelect.value = product.Type ?? product.product_type;
    if (product.Fac ?? product.fac) factorySelect.value = product.Fac ?? product.fac;

    let sn;
    try {
      sn = await invoke('get_current_sn');
    } catch (e) {
      sn = '------------';
    }
    updateSnDisplay(sn);

    showToast('应用初始化成功', 'success');
  } catch (error) {
    console.error('Init failed:', error);
    showToast('初始化失败: ' + error, 'error');
  }
}

function populateSelects(baseData) {
  const brandSelect = document.getElementById('brandSelect');
  const typeSelect = document.getElementById('typeSelect');
  const factorySelect = document.getElementById('factorySelect');

  const curBrand = brandSelect.value;
  const curType = typeSelect.value;
  const curFac = factorySelect.value;

  brandSelect.innerHTML = '';
  typeSelect.innerHTML = '';
  factorySelect.innerHTML = '';

  (baseData.brands || []).forEach(b => {
    const opt = document.createElement('option');
    opt.value = b.name;
    opt.textContent = b.name;
    brandSelect.appendChild(opt);
  });
  (baseData.types || []).forEach(t => {
    const opt = document.createElement('option');
    opt.value = t.name;
    opt.textContent = t.name;
    typeSelect.appendChild(opt);
  });
  (baseData.factories || []).forEach(f => {
    const opt = document.createElement('option');
    opt.value = f.name;
    opt.textContent = f.name;
    factorySelect.appendChild(opt);
  });

  if (curBrand && baseData.brands?.some(b => b.name === curBrand)) brandSelect.value = curBrand;
  if (curType && baseData.types?.some(t => t.name === curType)) typeSelect.value = curType;
  if (curFac && baseData.factories?.some(f => f.name === curFac)) factorySelect.value = curFac;
}

// ─── Product Config Tables ────────────────────────────────────────────────
function renderConfigTables(baseData) {
  renderConfigTable('brandsTableBody', baseData.brands || [], 'remove_brand');
  renderConfigTable('typesTableBody', baseData.types || [], 'remove_product_type');
  renderConfigTable('factoriesTableBody', baseData.factories || [], 'remove_factory');
}

function renderConfigTable(tbodyId, items, removeCmd) {
  const tbody = document.getElementById(tbodyId);
  tbody.innerHTML = '';
  if (items.length === 0) {
    tbody.innerHTML = '<tr><td colspan="3" class="text-center text-muted">暂无数据</td></tr>';
    return;
  }
  items.forEach(item => {
    const tr = document.createElement('tr');
    tr.innerHTML =
      `<td>${escHtml(item.name)}</td>` +
      `<td>${escHtml(item.code)}</td>` +
      `<td><button class="btn btn-primary" data-cmd="${removeCmd}" data-name="${escHtml(item.name)}">删除</button></td>`;
    tbody.appendChild(tr);
  });
  tbody.querySelectorAll('[data-cmd]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const cmd = btn.dataset.cmd;
      const name = btn.dataset.name;
      try {
        showLoading();
        const newBaseData = await invoke(cmd, { name });
        state.baseData = newBaseData;
        populateSelects(newBaseData);
        renderConfigTables(newBaseData);
        showToast('删除成功', 'success');
      } catch (e) {
        showToast('删除失败: ' + e, 'error');
      } finally {
        hideLoading();
      }
    });
  });
}

async function addConfigItem(addCmd, nameId, codeId) {
  const nameEl = document.getElementById(nameId);
  const codeEl = document.getElementById(codeId);
  const name = nameEl.value.trim();
  const code = codeEl.value.trim();
  if (!name || !code) {
    showToast('名称和编码不能为空', 'error');
    return;
  }
  try {
    showLoading();
    const newBaseData = await invoke(addCmd, { name, code });
    state.baseData = newBaseData;
    populateSelects(newBaseData);
    renderConfigTables(newBaseData);
    nameEl.value = '';
    codeEl.value = '';
    showToast('添加成功', 'success');
  } catch (e) {
    showToast('添加失败: ' + e, 'error');
  } finally {
    hideLoading();
  }
}

document.getElementById('addBrandBtn').addEventListener('click', () => addConfigItem('add_brand', 'newBrandName', 'newBrandCode'));
document.getElementById('addTypeBtn').addEventListener('click', () => addConfigItem('add_product_type', 'newTypeName', 'newTypeCode'));
document.getElementById('addFactoryBtn').addEventListener('click', () => addConfigItem('add_factory', 'newFactoryName', 'newFactoryCode'));

document.querySelectorAll('.config-tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.config-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.config-tab-panel').forEach(p => p.classList.remove('active'));
    tab.classList.add('active');
    document.getElementById('panel-' + tab.dataset.tab).classList.add('active');
  });
});

// ─── Production Page Event Handlers ───────────────────────────────────────
async function onProductChange() {
  const brand = document.getElementById('brandSelect').value;
  const productType = document.getElementById('typeSelect').value;
  const fac = document.getElementById('factorySelect').value;
  try {
    const sn = await tauriInvoke('set_product', { brand, productType, fac });
    updateSnDisplay(sn);
  } catch (error) {
    console.error('Failed to update product:', error);
  }
}

document.getElementById('brandSelect').addEventListener('change', onProductChange);
document.getElementById('typeSelect').addEventListener('change', onProductChange);
document.getElementById('factorySelect').addEventListener('change', onProductChange);

async function doConnect() {
  const ip = document.getElementById('deviceIpInput').value.trim();
  if (!ip) {
    showToast('请输入设备 IP 地址', 'error');
    return;
  }
  state.deviceIp = ip;
  await tauriInvoke('set_device_ip', { ip });
  updateConnectionStatus(true);
  showToast(`已连接到 ${ip}`, 'success');
  try {
    const info = await tauriInvoke('get_device_info_from_device');
    updateDeviceInfo(info);
  } catch (_) {}
}

document.getElementById('connectBtn').addEventListener('click', doConnect);
document.getElementById('deviceIpInput').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') doConnect();
});

document.getElementById('writeSnBtn').addEventListener('click', async () => {
  if (!state.connected) { showToast('请先连接设备', 'error'); return; }
  if (!state.currentSn) { showToast('序列号为空', 'error'); return; }

  try {
    const statusEl = document.getElementById('writeStatus');
    statusEl.innerHTML = '<span class="status-indicator info">写入中...</span>';

    const snOk = await tauriInvoke('write_sn_to_device', { sn: state.currentSn });
    if (!snOk) {
      statusEl.innerHTML = '<span class="status-indicator error">SN 写入失败</span>';
      showToast('SN 写入失败', 'error');
      return;
    }
    const info = await tauriInvoke('get_device_info_from_device');
    updateDeviceInfo(info);
    await tauriInvoke('save_execute_data');
    await tauriInvoke('save_device_record', { deviceInfo: info });
    const newSn = await tauriInvoke('increment_sequence');
    updateSnDisplay(newSn);

    statusEl.innerHTML = '<span class="status-indicator success">写入成功</span>';
    showToast('SN 写入成功', 'success');
  } catch (error) {
    document.getElementById('writeStatus').innerHTML =
      '<span class="status-indicator error">写入失败</span>';
    showToast('写入失败: ' + error, 'error');
  }
});

document.getElementById('getDeviceInfoBtn').addEventListener('click', async () => {
  if (!state.connected) { showToast('请先连接设备', 'error'); return; }
  try {
    const info = await tauriInvoke('get_device_info_from_device');
    updateDeviceInfo(info);
    showToast('设备信息获取成功', 'success');
  } catch (error) {
    showToast('获取失败: ' + error, 'error');
  }
});

document.getElementById('activateBtn').addEventListener('click', async () => {
  if (!state.connected) { showToast('请先连接设备', 'error'); return; }
  try {
    const ok = await tauriInvoke('activate_device');
    showToast(ok ? '设备激活成功' : '设备激活失败', ok ? 'success' : 'error');
  } catch (error) {
    showToast('激活失败: ' + error, 'error');
  }
});

// ─── Start ────────────────────────────────────────────────────────────────
window.addEventListener('DOMContentLoaded', initApp);
