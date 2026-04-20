// preset.js — 스캔 프리셋 저장/불러오기 (localStorage)

const PRESET_KEY = 'dup_scan_presets';

function loadPresets() {
  try { return JSON.parse(localStorage.getItem(PRESET_KEY)) || []; }
  catch { return []; }
}

function savePresets(list) {
  localStorage.setItem(PRESET_KEY, JSON.stringify(list));
}

function captureCurrentPreset() {
  return {
    paths:       getScanPaths(),
    noPhash:     document.getElementById('opt-no-phash')?.checked ?? false,
    noVhash:     document.getElementById('opt-no-vhash')?.checked ?? false,
    noArchive:   document.getElementById('opt-no-archive')?.checked ?? false,
    phashExact:  document.getElementById('opt-phash-exact')?.value ?? '0',
    phashSimilar:document.getElementById('opt-phash-similar')?.value ?? '10',
    vhashExact:  document.getElementById('opt-vhash-exact')?.value ?? '0',
    vhashSimilar:document.getElementById('opt-vhash-similar')?.value ?? '10',
    vhashFrames: document.getElementById('opt-vhash-frames')?.value ?? '10',
    minOverlap:  document.getElementById('opt-min-overlap')?.value ?? '5',
    minArcFiles: document.getElementById('opt-min-arc-files')?.value ?? '0',
    excludePatterns: document.getElementById('opt-exclude-patterns')?.value ?? '',
  };
}

function applyPreset(preset) {
  if (!preset) return;
  if (preset.paths) setScanPaths(preset.paths);
  const setChk = (id, val) => { const el = document.getElementById(id); if (el) el.checked = val; };
  const setVal = (id, val) => { const el = document.getElementById(id); if (el) el.value = val; };
  setChk('opt-no-phash',    preset.noPhash);
  setChk('opt-no-vhash',    preset.noVhash);
  setChk('opt-no-archive',  preset.noArchive);
  setVal('opt-phash-exact', preset.phashExact);
  setVal('opt-phash-similar', preset.phashSimilar);
  setVal('opt-vhash-exact', preset.vhashExact);
  setVal('opt-vhash-similar', preset.vhashSimilar);
  setVal('opt-vhash-frames', preset.vhashFrames);
  setVal('opt-min-overlap', preset.minOverlap);
  setVal('opt-min-arc-files', preset.minArcFiles);
  setVal('opt-exclude-patterns', preset.excludePatterns ?? '');
}

function openPresetModal() {
  const existing = document.getElementById('preset-modal');
  if (existing) existing.remove();

  const list = loadPresets();
  const modal = document.createElement('div');
  modal.id = 'preset-modal';
  modal.className = 'modal-overlay';
  modal.innerHTML = `
    <div class="modal" style="min-width:340px;max-width:480px">
      <h2>스캔 프리셋</h2>
      <div id="preset-list" style="margin-bottom:12px;display:flex;flex-direction:column;gap:6px;max-height:260px;overflow-y:auto">
        ${list.length === 0
          ? '<div style="color:var(--text2);font-size:12px">저장된 프리셋 없음</div>'
          : list.map((p, i) => `
            <div class="preset-item">
              <span class="preset-name" style="flex:1;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escHtml(p.name)}</span>
              <button class="action-btn" style="padding:3px 8px;font-size:11px" onclick="loadPreset(${i})">불러오기</button>
              <button class="action-btn danger" style="padding:3px 8px;font-size:11px" onclick="deletePreset(${i})">삭제</button>
            </div>`).join('')}
      </div>
      <div style="display:flex;gap:6px;margin-bottom:12px">
        <input id="preset-name-input" type="text" placeholder="프리셋 이름" class="path-input" style="flex:1">
        <button class="action-btn primary" style="white-space:nowrap" onclick="saveCurrentPreset()">현재 설정 저장</button>
      </div>
      <div class="modal-btns"><button class="action-btn" onclick="closePresetModal()">닫기</button></div>
    </div>`;
  document.body.appendChild(modal);
  modal.addEventListener('click', e => { if (e.target === modal) closePresetModal(); });
}

function closePresetModal() {
  document.getElementById('preset-modal')?.remove();
}

function saveCurrentPreset() {
  const nameEl = document.getElementById('preset-name-input');
  const name = nameEl?.value.trim();
  if (!name) { nameEl?.focus(); return; }
  const list = loadPresets();
  list.push({ name, ...captureCurrentPreset() });
  savePresets(list);
  closePresetModal();
  openPresetModal();
}

function loadPreset(index) {
  const list = loadPresets();
  if (list[index]) applyPreset(list[index]);
  closePresetModal();
}

function deletePreset(index) {
  const list = loadPresets();
  list.splice(index, 1);
  savePresets(list);
  closePresetModal();
  openPresetModal();
}

function escHtml(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
