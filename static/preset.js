// preset.js — 스캔 프리셋 저장/불러오기 (서버 파일 다이얼로그)

let presetList = [];

function captureCurrentPreset() {
  return {
    paths:        getScanPaths(),
    noPhash:      document.getElementById('opt-no-phash')?.checked ?? false,
    noVhash:      document.getElementById('opt-no-vhash')?.checked ?? false,
    noArchive:    document.getElementById('opt-no-archive')?.checked ?? false,
    phashExact:   document.getElementById('opt-phash-exact')?.value ?? '0',
    phashSimilar: document.getElementById('opt-phash-similar')?.value ?? '10',
    vhashExact:   document.getElementById('opt-vhash-exact')?.value ?? '0',
    vhashSimilar: document.getElementById('opt-vhash-similar')?.value ?? '10',
    vhashFrames:  document.getElementById('opt-vhash-frames')?.value ?? '10',
    minOverlap:   document.getElementById('opt-min-overlap')?.value ?? '5',
    minArcFiles:  document.getElementById('opt-min-arc-files')?.value ?? '0',
    excludePatterns: document.getElementById('opt-exclude-patterns')?.value ?? '',
  };
}

function applyPreset(preset) {
  if (!preset) return;
  if (preset.paths) setScanPaths(preset.paths);
  const setChk = (id, val) => { const el = document.getElementById(id); if (el) el.checked = val; };
  const setVal = (id, val) => { const el = document.getElementById(id); if (el) el.value = val; };
  setChk('opt-no-phash',     preset.noPhash);
  setChk('opt-no-vhash',     preset.noVhash);
  setChk('opt-no-archive',   preset.noArchive);
  setVal('opt-phash-exact',  preset.phashExact);
  setVal('opt-phash-similar',preset.phashSimilar);
  setVal('opt-vhash-exact',  preset.vhashExact);
  setVal('opt-vhash-similar',preset.vhashSimilar);
  setVal('opt-vhash-frames', preset.vhashFrames);
  setVal('opt-min-overlap',  preset.minOverlap);
  setVal('opt-min-arc-files',preset.minArcFiles);
  setVal('opt-exclude-patterns', preset.excludePatterns ?? '');
}

function openPresetModal() {
  document.getElementById('preset-modal')?.remove();

  const modal = document.createElement('div');
  modal.id = 'preset-modal';
  modal.className = 'modal-overlay';
  modal.innerHTML = `
    <div class="modal" style="min-width:340px;max-width:480px">
      <h2>스캔 프리셋</h2>
      <div id="preset-list" style="margin-bottom:12px;display:flex;flex-direction:column;gap:6px;max-height:260px;overflow-y:auto">
        ${renderPresetItems()}
      </div>
      <div style="display:flex;gap:6px;margin-bottom:12px">
        <input id="preset-name-input" type="text" placeholder="프리셋 이름" class="path-input" style="flex:1">
        <button class="action-btn primary" style="white-space:nowrap" onclick="saveCurrentPreset()">현재 설정 저장</button>
      </div>
      <div class="modal-btns" style="justify-content:space-between">
        <div style="display:flex;gap:6px">
          <button class="action-btn" onclick="exportPresets()">파일로 내보내기</button>
          <button class="action-btn" onclick="importPresets()">파일에서 가져오기</button>
        </div>
        <button class="action-btn" onclick="closePresetModal()">닫기</button>
      </div>
    </div>`;
  document.body.appendChild(modal);
  modal.addEventListener('click', e => { if (e.target === modal) closePresetModal(); });
}

function renderPresetItems() {
  if (presetList.length === 0)
    return '<div style="color:var(--text2);font-size:12px">저장된 프리셋 없음</div>';
  return presetList.map((p, i) => `
    <div class="preset-item">
      <span class="preset-name" style="flex:1;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escHtml(p.name)}</span>
      <button class="action-btn" style="padding:3px 8px;font-size:11px" onclick="loadPreset(${i})">불러오기</button>
      <button class="action-btn danger" style="padding:3px 8px;font-size:11px" onclick="deletePreset(${i})">삭제</button>
    </div>`).join('');
}

function refreshPresetList() {
  const el = document.getElementById('preset-list');
  if (el) el.innerHTML = renderPresetItems();
}

function closePresetModal() {
  document.getElementById('preset-modal')?.remove();
}

function saveCurrentPreset() {
  const nameEl = document.getElementById('preset-name-input');
  const name = nameEl?.value.trim();
  if (!name) { nameEl?.focus(); return; }
  presetList.push({ name, ...captureCurrentPreset() });
  refreshPresetList();
  if (nameEl) nameEl.value = '';
}

function loadPreset(index) {
  if (presetList[index]) applyPreset(presetList[index]);
  closePresetModal();
}

function deletePreset(index) {
  presetList.splice(index, 1);
  refreshPresetList();
}

async function exportPresets() {
  if (presetList.length === 0) { alert('저장된 프리셋이 없습니다.'); return; }
  try {
    const res = await fetch('/api/preset/save', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ data: presetList }),
    });
    const data = await res.json();
    if (data.ok) alert('저장 완료: ' + data.path);
  } catch (e) {
    console.error('export error', e);
  }
}

async function importPresets() {
  try {
    const res = await fetch('/api/preset/load');
    const data = await res.json();
    if (data.ok && Array.isArray(data.data)) {
      presetList = data.data;
      refreshPresetList();
    }
  } catch (e) {
    console.error('import error', e);
  }
}

function escHtml(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
