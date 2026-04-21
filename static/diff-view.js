// diff-view.js — 이미지 그룹 내 diff 뷰 모달

const IMAGE_EXTS = new Set(['jpg','jpeg','png','gif','bmp','webp','tiff','heic','avif']);

function isImageFile(path) {
  const ext = (path || '').split('.').pop().toLowerCase();
  return IMAGE_EXTS.has(ext);
}

function openDiffModal(files) {
  const existing = document.getElementById('diff-modal');
  if (existing) existing.remove();

  const imgFiles = files.filter(f => isImageFile(f.path));
  if (imgFiles.length < 2) return;

  const modal = document.createElement('div');
  modal.id = 'diff-modal';
  modal.className = 'modal-overlay';

  const items = imgFiles.map((f, i) => `
    <div class="diff-item">
      <img src="/api/file?path=${encodeURIComponent(f.path)}"
           alt="${escHtml(f.path)}"
           class="diff-img"
           loading="lazy"
           onerror="this.style.display='none'">
      <div class="diff-caption" title="${escHtml(f.path)}">${escHtml(shortPath(f.path))}</div>
      <div class="diff-size">${fmtBytes(f.size)}</div>
      <div class="diff-meta">${f.created ? f.created.replace('T', ' ') : '-'}</div>
    </div>`).join('');

  modal.innerHTML = `
    <div class="modal diff-modal-box">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:12px">
        <h2>이미지 비교 (${imgFiles.length}개)</h2>
        <button class="action-btn" onclick="closeDiffModal()">✕ 닫기</button>
      </div>
      <div class="diff-grid">${items}</div>
    </div>`;

  document.body.appendChild(modal);
  modal.addEventListener('click', e => { if (e.target === modal) closeDiffModal(); });
}

function closeDiffModal() {
  document.getElementById('diff-modal')?.remove();
}

function shortPath(p) {
  const parts = p.replace(/\\/g, '/').split('/');
  return parts.length > 2 ? '…/' + parts.slice(-2).join('/') : p;
}

function fmtBytes(bytes) {
  if (!bytes) return '';
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
  if (bytes >= 1048576)    return (bytes / 1048576).toFixed(1) + ' MB';
  if (bytes >= 1024)       return (bytes / 1024).toFixed(0) + ' KB';
  return bytes + ' B';
}

function escHtml(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
