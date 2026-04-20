// scan-path.js — 스캔 경로 행 UI (+ 추가 / 📂 다이얼로그 / ✕ 삭제)

function initScanPath() {
  const container = document.getElementById('path-rows');
  if (!container) return;
  if (container.children.length === 0) addPathRow('');
}

function addPathRow(value) {
  const container = document.getElementById('path-rows');
  if (!container) return;

  const row = document.createElement('div');
  row.className = 'path-row';

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'path-input';
  input.placeholder = '/경로/입력 또는 📂 선택';
  input.value = value || '';

  const folderBtn = document.createElement('button');
  folderBtn.className = 'path-folder-btn';
  folderBtn.textContent = '📂';
  folderBtn.title = '폴더 선택';
  folderBtn.onclick = () => openFolderDialog(input);

  const removeBtn = document.createElement('button');
  removeBtn.className = 'path-remove-btn';
  removeBtn.textContent = '✕';
  removeBtn.title = '삭제';
  removeBtn.onclick = () => removePathRow(removeBtn);

  row.appendChild(input);
  row.appendChild(folderBtn);
  row.appendChild(removeBtn);
  container.appendChild(row);
}

function removePathRow(btn) {
  const container = document.getElementById('path-rows');
  if (!container) return;
  const row = btn.closest('.path-row');
  if (!row) return;
  if (container.children.length <= 1) {
    row.querySelector('.path-input').value = '';
    return;
  }
  row.remove();
}

async function openFolderDialog(input) {
  try {
    const { open } = window.__TAURI__?.dialog || {};
    if (!open) return;
    const selected = await open({ directory: true, multiple: false });
    if (selected) input.value = selected;
  } catch (e) {
    console.error('folder dialog error', e);
  }
}

function getScanPaths() {
  const inputs = document.querySelectorAll('#path-rows .path-input');
  return Array.from(inputs)
    .map(i => i.value.trim())
    .filter(v => v.length > 0);
}

function setScanPaths(paths) {
  const container = document.getElementById('path-rows');
  if (!container) return;
  container.innerHTML = '';
  const list = Array.isArray(paths) && paths.length > 0 ? paths : [''];
  list.forEach(p => addPathRow(p));
}
