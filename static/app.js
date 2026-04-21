// ── 상태 ──
let state = { regular: [], image: [], video: [], archive: [] };
let currentTab = 'regular';
let filterText = '';
let sortOrder = 'none';
let typeFilter = 'all';
let pollTimer = null;
let pollActive = false;
let logSeenCount = 0;

// ── VirtualScroller ──
const ROW_H = 44;   // 그룹 헤더 높이
const FILE_H = 36;  // 파일 행 높이
const BUFFER = 10;  // 위아래 추가 렌더 그룹 수

class VirtualScroller {
  constructor(wrap) {
    this.wrap = wrap;       // 스크롤 뷰포트 엘리먼트
    this.groups = [];       // 현재 탭 그룹 배열
    this.expanded = new Set();
    this._spacer = null;
    this._items = null;
    this._onScroll = this._render.bind(this);
    wrap.addEventListener('scroll', this._onScroll, { passive: true });
  }

  setGroups(groups) {
    this.groups = groups;
    this._ensureDOM();
    this.recalc();
    this._render();
  }

  toggleExpand(gid) {
    this.expanded.has(gid) ? this.expanded.delete(gid) : this.expanded.add(gid);
    this.recalc();
    this._render();
  }

  invalidate() {
    this.recalc();
    this._render();
  }

  recalc() {
    // 각 그룹의 y오프셋과 높이를 계산
    this._offsets = [];
    let y = 8; // 상단 패딩
    for (const g of this.groups) {
      this._offsets.push(y);
      const h = ROW_H + (this.expanded.has(g.id) ? g.files.length * FILE_H : 0);
      y += h + 8; // margin-bottom
    }
    this._totalH = y;
    if (this._spacer) this._spacer.style.height = this._totalH + 'px';
  }

  _ensureDOM() {
    if (this._spacer) return;
    const wrap = this.wrap;
    wrap.innerHTML = '';
    this._spacer = document.createElement('div');
    this._spacer.className = 'vs-spacer';
    this._items = document.createElement('div');
    this._items.className = 'vs-items';
    this._spacer.appendChild(this._items);
    wrap.appendChild(this._spacer);
  }

  _render() {
    if (!this.groups.length) {
      if (this._items) this._items.innerHTML = '<div class="empty">표시할 그룹이 없습니다</div>';
      if (this._spacer) this._spacer.style.height = '80px';
      return;
    }
    const scrollTop = this.wrap.scrollTop;
    const viewH = this.wrap.clientHeight;
    const top = scrollTop;
    const bottom = scrollTop + viewH;

    // 보이는 범위 그룹 인덱스
    let start = 0, end = this.groups.length - 1;
    for (let i = 0; i < this._offsets.length; i++) {
      const groupH = ROW_H + (this.expanded.has(this.groups[i].id) ? this.groups[i].files.length * FILE_H : 0);
      if (this._offsets[i] + groupH < top) start = i + 1;
      if (this._offsets[i] > bottom && end === this.groups.length - 1) end = i;
    }
    start = Math.max(0, start - BUFFER);
    end = Math.min(this.groups.length - 1, end + BUFFER);

    const html = [];
    for (let i = start; i <= end; i++) {
      const g = this.groups[i];
      const isOpen = this.expanded.has(g.id);
      const meta = buildMeta(g);
      const originIdx = getOriginIndex(g.files);
      const filesHtml = isOpen ? g.files.map((f, fi) => renderFile(g.id, fi, f, fi === originIdx)).join('') : '';
      const showDiff = currentTab === 'image' || currentTab === 'video' ||
        (currentTab === 'regular' && g.files.some(f => isImageFile(f.path) || isVideoFile(f.path)));
      const diffBtn = showDiff
        ? `<button class="finder-btn" style="margin-right:4px" onclick="event.stopPropagation();openDiffModal(${JSON.stringify(g.files).replace(/"/g,'&quot;')})">비교</button>`
        : '';
      const bulkBtns = `<button class="bulk-btn" onclick="event.stopPropagation();bulkKeep('${g.id}')">전체 KEEP</button><button class="bulk-btn" onclick="event.stopPropagation();bulkRemove('${g.id}')">전체 REMOVE</button>`;
      html.push(`
        <div class="group" id="group-${g.id}" style="position:absolute;left:8px;right:8px;top:${this._offsets[i] - 8}px">
          <div class="group-header" onclick="vsToggleGroup('${g.id}')">
            <span class="group-id">${g.id}</span>
            <span class="group-meta">${meta}</span>
            <span class="group-savable">${g.savable_fmt}</span>
            ${diffBtn}${bulkBtns}
            <span class="group-collapse">${isOpen ? '▴' : '▾'}</span>
          </div>
          <div class="group-body" id="body-${g.id}">${filesHtml}</div>
        </div>`);
    }
    this._items.innerHTML = html.join('');
    this._spacer.style.height = this._totalH + 'px';
  }

  destroy() {
    this.wrap.removeEventListener('scroll', this._onScroll);
  }
}

// ── TabCache ──
class TabCache {
  constructor() {
    this._cache = {};  // tab → { groups, scroller }
  }

  get(tab) { return this._cache[tab]; }

  set(tab, groups, scroller) {
    this._cache[tab] = { groups, scroller };
  }

  invalidate(tab) {
    if (this._cache[tab]) delete this._cache[tab];
  }

  invalidateAll() { this._cache = {}; }
}

const tabCache = new TabCache();
let activeScroller = null;

// ── 페이지 로드 시 type-filter-bar 초기 표시 ──
document.addEventListener('DOMContentLoaded', () => {
  const bar = document.getElementById('type-filter-bar');
  if (bar) bar.style.display = 'flex';
  initScanPath();
});

// ── 플랫폼 조회 ──
let platform = 'darwin';
(async function initPlatform() {
  try {
    const r = await fetch('/api/platform');
    const data = await r.json();
    platform = data.platform || 'darwin';
    if (data.port) {
      const el = document.getElementById('header-status');
      if (el) el.textContent = `:${data.port}`;
    }
  } catch(e) {}
})();

// ── 페이지 로드 시 서버 상태 복원 ──
(async function restoreState() {
  try {
    const r = await fetch('/api/scan/status');
    const data = await r.json();
    if (data.status === 'scanning') {
      document.getElementById('scan-btn').disabled = true;
      document.getElementById('cancel-btn').style.display = '';
      setStatus('스캔 중...'); setHeaderStatus('스캔 중...');
      startPolling();
    } else if (data.result) {
      state = data.result;
      tabCache.invalidateAll();
      updateBadges();
      renderGroups();
      setStatus(data.status === 'done' ? '스캔 완료' : data.status);
      setHeaderStatus(data.status === 'done' ? '완료' : data.status);
    }
    if (data.log?.length) { updateLog(data.log); logSeenCount = data.log.length; }
    if (data.paths?.length) {
      setScanPaths(data.paths);
    }
  } catch(e) {}
})();

// ── 스캔 ──
async function startScan() {
  const paths = getScanPaths();
  if (!paths.length) { alert('경로를 입력하세요'); return; }

  const btn = document.getElementById('scan-btn');
  btn.disabled = true;
  document.getElementById('cancel-btn').style.display = '';
  document.getElementById('log-panel').innerHTML = '';
  logSeenCount = 0;
  setStatus('스캔 중...');
  setHeaderStatus('스캔 중...');

  const iv = id => parseInt(document.getElementById(id)?.value || '0', 10);
  const body = {
    paths,
    no_phash: document.getElementById('opt-no-phash').checked,
    no_vhash: document.getElementById('opt-no-vhash').checked,
    no_archive: document.getElementById('opt-no-archive').checked,
    phash_exact: iv('opt-phash-exact'),
    phash_similar: iv('opt-phash-similar'),
    vhash_exact: iv('opt-vhash-exact'),
    vhash_similar: iv('opt-vhash-similar'),
    vhash_frames: iv('opt-vhash-frames'),
    min_overlap: iv('opt-min-overlap'),
    min_arc_files: iv('opt-min-arc-files'),
    exclude_patterns: (document.getElementById('opt-exclude-patterns')?.value || '')
      .split('\n').map(s => s.trim()).filter(Boolean),
    num_threads: parseInt(document.getElementById('opt-threads')?.value || '0', 10),
    min_size_kb: iv('opt-min-size-kb'),
    check_inode: document.getElementById('opt-check-inode')?.checked ?? false,
    partial_hash_kb: iv('opt-partial-hash-kb'),
  };

  try {
    const r = await fetch('/api/scan', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify(body) });
    if (!r.ok) { const e = await r.json(); alert(e.detail); btn.disabled = false; return; }
    startPolling();
  } catch(e) { alert('요청 실패: ' + e); btn.disabled = false; }
}

function startPolling() {
  pollActive = true;
  schedulePoll();
}

function stopPolling() {
  pollActive = false;
  if (pollTimer) { clearTimeout(pollTimer); pollTimer = null; }
}

function schedulePoll() {
  if (!pollActive) return;
  pollTimer = setTimeout(async () => {
    await pollStatus();
    schedulePoll();
  }, 600);
}

async function pollStatus() {
  try {
    const r = await fetch('/api/scan/status');
    const data = await r.json();
    if (data.log?.length > logSeenCount) {
      updateLog(data.log.slice(logSeenCount));
      logSeenCount = data.log.length;
    }
    if (data.status === 'done' || data.status === 'error' || data.status === 'cancelled') {
      stopPolling();
      document.getElementById('scan-btn').disabled = false;
      document.getElementById('cancel-btn').style.display = 'none';
      if (data.status === 'done' && data.result) {
        state = data.result;
        tabCache.invalidateAll();
        updateBadges();
        renderGroups();
        buildStatsPanel(state);
        setStatus('스캔 완료');
        setHeaderStatus('완료');
      } else if (data.status === 'cancelled') {
        setStatus('중단됨');
        setHeaderStatus('중단');
      } else {
        setStatus('오류 발생');
        setHeaderStatus('오류');
      }
    }
  } catch(e) {}
}

async function cancelScan() {
  const btn = document.getElementById('cancel-btn');
  btn.disabled = true;
  btn.textContent = '중단 중...';
  try {
    await fetch('/api/scan/cancel', { method: 'POST' });
  } catch(e) {}
}

// updateLog는 progress.js에서 정의 (덮어쓰기 카운터 지원)

// ── 탭 ──
function switchTab(tab) {
  currentTab = tab;
  filterText = '';
  typeFilter = 'all';
  document.getElementById('filter-input').value = '';
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === tab));
  document.querySelectorAll('.type-filter-btn').forEach(b =>
    b.classList.toggle('active', b.dataset.type === 'all')
  );
  const typeFilterBar = document.getElementById('type-filter-bar');
  if (typeFilterBar) typeFilterBar.style.display = tab === 'regular' ? 'flex' : 'none';
  const sharedOpt = document.getElementById('sort-shared-opt');
  if (sharedOpt) sharedOpt.style.display = tab === 'archive' ? '' : 'none';
  if (tab !== 'archive' && sortOrder === 'shared') {
    sortOrder = 'none';
    const sel = document.getElementById('sort-select');
    if (sel) sel.value = 'none';
  }
  // 탭 전환: 이전 스크롤러 이벤트 제거, 새 탭 캐시 확인
  const cached = tabCache.get(tab);
  if (cached) {
    // 캐시 있으면 새 wrap에 재연결
    if (activeScroller) activeScroller.destroy();
    const wrap = getWrap();
    const scroller = new VirtualScroller(wrap);
    scroller.expanded = cached.scroller ? cached.scroller.expanded : new Set();
    activeScroller = scroller;
    tabCache.set(tab, cached.groups, scroller);
    scroller.setGroups(cached.groups);
    updateActionInfo();
  } else {
    renderGroups();
  }
}

function updateBadges() {
  for (const tab of ['regular','image','video','archive']) {
    const groups = state[tab] || [];
    const totalBytes = groups.reduce((s, g) =>
      s + g.files.reduce((gs, f) => gs + (f.size || 0), 0), 0);
    const badge = document.getElementById('badge-' + tab);
    badge.textContent = groups.length || 0;
    badge.title = groups.length ? `${groups.length}그룹 · ${fmtSize(totalBytes)}` : '';
  }
}

function fmtSize(n) {
  for (const u of ['B','KB','MB','GB']) {
    if (n < 1024) return `${n.toFixed(1)} ${u}`;
    n /= 1024;
  }
  return `${n.toFixed(1)} TB`;
}

// ── 렌더링 ──
function getWrap() {
  return document.getElementById('groups-container-wrap');
}

function renderGroups() {
  const wrap = getWrap();
  if (!wrap) return;
  const groups = filteredGroups();

  // 캐시 확인 — 같은 그룹 배열이면 scroller.invalidate()만
  const cached = tabCache.get(currentTab);
  if (cached && cached.scroller) {
    cached.groups = groups;
    activeScroller = cached.scroller;
    activeScroller.setGroups(groups);
  } else {
    if (activeScroller) activeScroller.destroy();
    const scroller = new VirtualScroller(wrap);
    scroller.setGroups(groups);
    activeScroller = scroller;
    tabCache.set(currentTab, groups, scroller);
  }
  updateActionInfo();
}

const isArchiveEntry = f => f.type === 'zip_entry' || f.path.includes('::');

function filteredGroups() {
  let groups = state[currentTab] || [];
  if (filterText) {
    const q = filterText.toLowerCase();
    groups = groups.filter(g => g.files.some(f => f.path.toLowerCase().includes(q)));
  }
  if (currentTab === 'regular' && typeFilter === 'archive') {
    groups = groups.filter(g => g.files.some(isArchiveEntry));
  } else if (currentTab === 'regular' && typeFilter === 'normal') {
    groups = groups.filter(g => g.files.every(f => !isArchiveEntry(f)));
  }
  if (sortOrder === 'savable') {
    groups = [...groups].sort((a, b) => (b.savable || 0) - (a.savable || 0));
  } else if (sortOrder === 'total') {
    groups = [...groups].sort((a, b) => {
      const maxSize = g => Math.max(...g.files.map(f => f.size || 0));
      return maxSize(b) - maxSize(a);
    });
  } else if (sortOrder === 'count') {
    groups = [...groups].sort((a, b) => b.files.length - a.files.length);
  } else if (sortOrder === 'archive_first') {
    groups = [...groups].sort((a, b) => {
      const hasArc = g => g.files.some(isArchiveEntry) ? 1 : 0;
      return hasArc(b) - hasArc(a);
    });
  } else if (sortOrder === 'shared') {
    groups = [...groups].sort((a, b) => (b.shared || 0) - (a.shared || 0));
  }
  return groups;
}

function applySort(val) {
  sortOrder = val;
  tabCache.invalidate(currentTab);
  renderGroups();
}

function applyTypeFilter(val) {
  typeFilter = val;
  document.querySelectorAll('.type-filter-btn').forEach(b =>
    b.classList.toggle('active', b.dataset.type === val)
  );
  tabCache.invalidate(currentTab);
  renderGroups();
}

function renderGroup(g) {
  const meta = buildMeta(g);
  const originIdx = getOriginIndex(g.files);
  const filesHtml = g.files.map((f, fi) => renderFile(g.id, fi, f, fi === originIdx)).join('');
  const showDiff = currentTab === 'image' || currentTab === 'video' ||
    (currentTab === 'regular' && g.files.some(f => isImageFile(f.path) || isVideoFile(f.path)));
  const diffBtn = showDiff
    ? `<button class="finder-btn" style="margin-right:4px" onclick="event.stopPropagation();openDiffModal(${JSON.stringify(g.files).replace(/"/g,'&quot;')})">비교</button>`
    : '';
  const bulkBtns = `<button class="bulk-btn" onclick="event.stopPropagation();bulkKeep('${g.id}')">전체 KEEP</button><button class="bulk-btn" onclick="event.stopPropagation();bulkRemove('${g.id}')">전체 REMOVE</button>`;
  return `
    <div class="group" id="group-${g.id}">
      <div class="group-header" onclick="toggleGroup('${g.id}')">
        <span class="group-id">${g.id}</span>
        <span class="group-meta">${meta}</span>
        <span class="group-savable">${g.savable_fmt}</span>
        ${diffBtn}${bulkBtns}
        <span class="group-collapse">▾</span>
      </div>
      <div class="group-body" id="body-${g.id}">${filesHtml}</div>
    </div>`;
}

function buildMeta(g) {
  const n = g.files.length;
  if (currentTab === 'archive') return `${n}개 · 공통 ${g.shared}개`;
  if (currentTab === 'image' || currentTab === 'video') {
    const label = g.category === 'exact' ? '완전동일' : '유사';
    return `${n}개 · ${label}`;
  }
  const hash = g.files[0]?.hash || '';
  return `${n}개${hash ? ' · ' + hash : ''}`;
}

function getOriginIndex(files) {
  if (!files || files.length === 0) return 0;
  let minScore = Infinity, minIdx = 0;
  files.forEach((f, i) => {
    const score = getOriginScore(f.path);
    if (score < minScore) { minScore = score; minIdx = i; }
  });
  return minIdx;
}

function getOriginScore(filePath) {
  const depth = filePath.split('/').length;
  const copyPattern = /copy|복사|\(\d+\)|_\d{8}|[ _-]copy\d*/i;
  const nameScore = copyPattern.test(filePath) ? 10 : 0;
  return depth + nameScore;
}

function renderFile(gid, fi, f, isOrigin) {
  const cls = f.keep === true ? 'keep' : (f.keep === false ? 'remove' : '');
  const parts = f.path.split('/');
  const name = parts.pop();
  const dir = parts.join('/') || '/';
  let extra = '';
  if (currentTab === 'archive') {
    if (f.shared) extra += ` · 공통 ${f.shared}개`;
    if (f.total_files) extra += ` · 내부 ${f.total_files}개`;
  }
  const originBadge = isOrigin ? '<span class="origin-badge">원본 추정</span>' : '';
  return `
    <div class="file-row ${cls}" id="file-${gid}-${fi}" tabindex="0" onclick="toggleKeep('${gid}',${fi})" onkeydown="handleFileKey(event,'${gid}',${fi})">
      <div class="file-status"></div>
      <div class="file-info">
        <div class="file-name" title="${escHtml(f.path)}">${escHtml(name)}${originBadge}</div>
        <div class="file-path" title="${escHtml(dir)}">${escHtml(dir)}</div>
      </div>
      <span class="file-size">${f.size_fmt}${extra}</span>
      <button class="finder-btn" data-path="${escHtml(f.path)}">열기</button>
    </div>`;
}

function toggleGroup(gid) {
  // 기존 DOM 방식 폴백 (가상 스크롤 미사용 시)
  const body = document.getElementById('body-' + gid);
  if (body) body.style.display = body.style.display === 'none' ? '' : 'none';
}

function vsToggleGroup(gid) {
  if (activeScroller) activeScroller.toggleExpand(gid);
}

// ── 키보드 네비게이션 ──
function handleFileKey(e, gid, fi) {
  if (e.key === ' ') {
    e.preventDefault();
    toggleKeep(gid, fi);
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    const rows = [...document.querySelectorAll('#groups-container-wrap .file-row')];
    const cur = document.getElementById(`file-${gid}-${fi}`);
    const idx = rows.indexOf(cur);
    const next = e.key === 'ArrowDown' ? rows[idx + 1] : rows[idx - 1];
    if (next) { next.focus(); }
  }
}

// ── Keep/Remove 토글 ──
function toggleKeep(gid, fi) {
  const groups = state[currentTab];
  const g = groups.find(x => x.id === gid);
  if (!g) return;
  g.files[fi].keep = !g.files[fi].keep;
  // 가상 스크롤: 해당 행만 클래스 교체 (리렌더 없이)
  const el = document.getElementById(`file-${gid}-${fi}`);
  if (el) el.className = 'file-row ' + (g.files[fi].keep ? 'keep' : 'remove');
  updateActionInfo();
}

function bulkKeep(gid) {
  const g = state[currentTab]?.find(x => x.id === gid);
  if (!g) return;
  g.files.forEach((f, fi) => {
    f.keep = true;
    const el = document.getElementById(`file-${gid}-${fi}`);
    if (el) el.className = 'file-row keep';
  });
  updateActionInfo();
}

function bulkRemove(gid) {
  const g = state[currentTab]?.find(x => x.id === gid);
  if (!g) return;
  g.files.forEach((f, fi) => {
    f.keep = false;
    const el = document.getElementById(`file-${gid}-${fi}`);
    if (el) el.className = 'file-row remove';
  });
  updateActionInfo();
}

// ── 필터 일괄 ──
function applyFilter() {
  filterText = document.getElementById('filter-input').value.trim();
  tabCache.invalidate(currentTab);
  renderGroups();
}

function clearFilter() {
  filterText = '';
  document.getElementById('filter-input').value = '';
  tabCache.invalidate(currentTab);
  renderGroups();
}

function bulkAction(keepValue) {
  const q = filterText.toLowerCase();
  const groups = state[currentTab] || [];
  groups.forEach(g => {
    const match = !q || g.files.some(f => f.path.toLowerCase().includes(q));
    if (!match) return;
    if (keepValue === 'keep') {
      g.files.forEach(f => { f.keep = true; });
    } else {
      g.files.forEach(f => { f.keep = false; });
    }
  });
  tabCache.invalidate(currentTab);
  renderGroups();
}

function bulkKeep() { bulkAction('keep'); }
function bulkRemove() { bulkAction('remove'); }

// ── 파인더 ──
document.addEventListener('click', async e => {
  const btn = e.target.closest('.finder-btn');
  if (!btn) return;
  e.stopPropagation();
  const path = btn.dataset.path;
  if (!path) return;
  await fetch('/api/open-finder', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({path}) });
});

// ── 세션 ZIP 불러오기 ──
async function loadCsv() {
  const picked = await fetch('/api/pick-open-zip').then(r => r.json());
  if (!picked.path) return;
  try {
    const r = await fetch('/api/load-csv', {
      method: 'POST', headers: {'Content-Type':'application/json'},
      body: JSON.stringify({ path: picked.path })
    });
    if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
    const data = await r.json();
    const status = await (await fetch('/api/scan/status')).json();
    if (status.result) {
      state = status.result;
      tabCache.invalidateAll();
    }
    updateBadges();
    renderGroups();
    const summary = data.tabs.map(t => `${t}:${data.counts[t]}그룹`).join(' / ');
    setStatus(`ZIP 불러오기 완료 — ${summary}`);
    setHeaderStatus('ZIP 로드');
    const scanBtn = document.getElementById('scan-btn');
    scanBtn.disabled = true;
    scanBtn.title = 'ZIP 결과 보기 중입니다. 파일 존재 확인 후 스캔이 활성화됩니다.';
  } catch(e) { alert('불러오기 실패: ' + e); }
}

// ── 세션 ZIP 저장 ──
async function saveCsv() {
  const hasData = Object.values(state).some(g => g.length > 0);
  if (!hasData) { alert('저장할 데이터가 없습니다'); return; }
  const picked = await fetch('/api/pick-save-zip').then(r => r.json());
  if (!picked.path) return;
  const r = await fetch('/api/save-csv', {
    method: 'POST', headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ state, path: picked.path })
  });
  if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
  const data = await r.json();
  setStatus(`ZIP 저장 완료: ${data.filename}`);
}

// ── 삭제 ──
function confirmDelete(all) {
  const groups = state[currentTab] || [];
  let targets;
  if (all) {
    targets = groups.flatMap(g => g.files.filter(f => !f.keep).map(f => f.path));
  } else {
    targets = filteredGroups().flatMap(g => g.files.filter(f => !f.keep).map(f => f.path));
  }
  if (!targets.length) { alert('삭제할 파일이 없습니다'); return; }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <h2>삭제 확인</h2>
      <p>${targets.length}개 파일을 삭제합니다. 이 작업은 되돌릴 수 없습니다.</p>
      <div style="max-height:120px;overflow-y:auto;font-size:10px;color:var(--text2);margin-bottom:12px">
        ${targets.map(p => `<div>${escHtml(p)}</div>`).join('')}
      </div>
      <div class="modal-btns">
        <button class="action-btn cancel-btn">취소</button>
        <button class="action-btn danger delete-btn">삭제</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('.delete-btn').addEventListener('click', () => {
    overlay.remove();
    executeDelete(targets);
  });
  overlay.querySelector('.cancel-btn').addEventListener('click', () => overlay.remove());
}

async function executeDelete(targets) {
  setStatus('삭제 중...');
  const r = await fetch('/api/delete', {
    method: 'POST', headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ paths: targets })
  });
  const data = await r.json();
  const deletedSet = new Set(data.deleted);
  for (const tab of ['regular', 'image', 'video', 'archive']) {
    state[tab] = (state[tab] || [])
      .map(g => ({ ...g, files: g.files.filter(f => !deletedSet.has(f.path)) }))
      .filter(g => g.files.length > 1);
  }
  tabCache.invalidateAll();
  updateBadges();
  renderGroups();
  const msg = `삭제 완료: ${data.deleted.length}개` + (data.errors.length ? ` / 오류: ${data.errors.length}개` : '');
  setStatus(msg);
}

// ── 파일 존재 확인 ──
async function checkFiles() {
  setStatus('파일 존재 확인 중...');
  const r = await fetch('/api/check-files', { method: 'POST' });
  if (!r.ok) { const e = await r.json(); alert(e.detail); return; }
  const data = await r.json();
  const status = await (await fetch('/api/scan/status')).json();
  if (status.result) { state = status.result; tabCache.invalidateAll(); }
  updateBadges();
  renderGroups();
  if (data.count === 0) {
    setStatus('확인 완료 — 모든 파일이 존재합니다');
  } else {
    setStatus(`확인 완료 — ${data.count}개 없는 파일 제거됨`);
  }
  const scanBtn = document.getElementById('scan-btn');
  scanBtn.disabled = false;
  scanBtn.title = '';
}

// ── 전체 초기화 ──
function confirmReset() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div style="margin-bottom:12px;font-weight:600">전체 초기화</div>
      <div style="margin-bottom:16px;color:var(--text2);font-size:13px">스캔 결과와 CSV 불러오기 기록이 모두 지워집니다.<br>삭제된 실제 파일은 복구되지 않습니다.</div>
      <div style="display:flex;gap:8px;justify-content:flex-end">
        <button class="action-btn cancel-btn">취소</button>
        <button class="action-btn danger reset-btn">초기화</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('.reset-btn').addEventListener('click', () => doReset(overlay));
  overlay.querySelector('.cancel-btn').addEventListener('click', () => overlay.remove());
}

async function doReset(overlay) {
  const btn = overlay.querySelector('.reset-btn');
  btn.textContent = '초기화 중...';
  btn.disabled = true;
  await fetch('/api/reset', { method: 'POST' });
  overlay.remove();
  state = { regular: [], image: [], video: [], archive: [] };
  tabCache.invalidateAll();
  if (activeScroller) { activeScroller.destroy(); activeScroller = null; }
  const wrap = getWrap();
  if (wrap) wrap.innerHTML = '';
  document.getElementById('log-panel').innerHTML = '';
  const scanBtn = document.getElementById('scan-btn');
  scanBtn.disabled = false;
  scanBtn.title = '';
  setStatus('초기화 완료');
  setHeaderStatus('대기');
  updateBadges();
}

// ── 테마 토글 ──
(function initTheme() {
  const saved = localStorage.getItem('theme') || 'dark';
  document.documentElement.dataset.theme = saved;
  const btn = document.getElementById('theme-btn');
  if (btn) btn.textContent = saved === 'dark' ? '☀' : '🌙';
})();

function toggleTheme() {
  const current = document.documentElement.dataset.theme || 'dark';
  const next = current === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = next;
  localStorage.setItem('theme', next);
  document.getElementById('theme-btn').textContent = next === 'dark' ? '☀' : '🌙';
}

// ── 유틸 ──
function updateActionInfo() {
  const groups = state[currentTab] || [];
  const removeFiles = groups.flatMap(g => g.files.filter(f => !f.keep));
  const removeCount = removeFiles.length;
  const removeBytes = removeFiles.reduce((s, f) => s + (f.size || 0), 0);
  const totalGroups = groups.length;
  const savable = removeBytes > 0 ? ` · ${fmtSize(removeBytes)} 절약 예정` : '';
  document.getElementById('action-info').textContent =
    `${totalGroups}그룹 · REMOVE ${removeCount}개${savable}`;
}

function updateThreadsLabel(val) {
  document.getElementById('opt-threads-label').textContent = val === '0' ? '자동' : `${val}개`;
}

function setStatus(msg) {
  document.getElementById('status-text').textContent = msg;
  setGauge(null);
}

function setHeaderStatus(msg) { document.getElementById('header-status').textContent = msg; }
function escHtml(s) { return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }
function escAttr(s) { return String(s).replace(/'/g,"\\'").replace(/\\/g,'\\\\'); }
