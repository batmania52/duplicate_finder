// stats.js — 파일 타입 통계 패널 + SVG 도넛 차트

const TYPE_COLORS = {
  image:   '#4a9eff',
  video:   '#9c27b0',
  archive: '#ff9800',
  regular: '#4caf50',
  other:   '#607d8b',
};
const TYPE_LABELS = {
  image:   '이미지',
  video:   '영상',
  archive: '아카이브',
  regular: '일반',
  other:   '기타',
};

function buildStatsPanel(state) {
  const panel = document.getElementById('stats-panel');
  if (!panel) return;

  const counts = { image: 0, video: 0, archive: 0, regular: 0, other: 0 };
  const sizes  = { image: 0, video: 0, archive: 0, regular: 0, other: 0 };
  let totalCount = 0;
  let totalSize  = 0;

  const tabGroups = [
    { type: 'regular',  groups: state.regular  || [] },
    { type: 'image',    groups: state.image    || [] },
    { type: 'video',    groups: state.video    || [] },
    { type: 'archive',  groups: state.archive  || [] },
  ];

  for (const { type: tabType, groups } of tabGroups) {
    for (const g of groups) {
      for (const f of (g.files || [])) {
        const t = detectType(f.path, tabType);
        counts[t] = (counts[t] || 0) + 1;
        sizes[t]  = (sizes[t]  || 0) + (f.size || 0);
        totalCount++;
        totalSize += (f.size || 0);
      }
    }
  }

  panel.innerHTML = `
    <div class="stats-inner">
      <div class="stats-donut">${buildDonut(counts, totalCount)}</div>
      <div class="stats-table">${buildTable(counts, sizes, totalCount, totalSize)}</div>
    </div>`;
  panel.style.display = '';
}

function detectType(filePath, groupType) {
  if (groupType === 'image')   return 'image';
  if (groupType === 'video')   return 'video';
  if (groupType === 'archive') return 'archive';
  const ext = (filePath || '').split('.').pop().toLowerCase();
  if (['jpg','jpeg','png','gif','bmp','webp','tiff','heic'].includes(ext)) return 'image';
  if (['mp4','mkv','avi','mov','wmv','flv','webm','m4v'].includes(ext))    return 'video';
  if (['zip','7z','rar','tar','gz','bz2','xz'].includes(ext))              return 'archive';
  if (groupType === 'regular') return 'regular';
  return 'other';
}

function buildDonut(counts, total) {
  if (total === 0) return '';

  const R = 40, CX = 50, CY = 50, STROKE = 14;
  const circumference = 2 * Math.PI * R;
  let offset = 0;
  const slices = [];

  for (const [type, color] of Object.entries(TYPE_COLORS)) {
    const n = counts[type] || 0;
    if (n === 0) continue;
    const pct = n / total;
    const dash = pct * circumference;
    slices.push({ type, color, dash, offset });
    offset += dash;
  }

  const paths = slices.map(s => `
    <circle cx="${CX}" cy="${CY}" r="${R}"
      fill="none" stroke="${s.color}" stroke-width="${STROKE}"
      stroke-dasharray="${s.dash} ${circumference - s.dash}"
      stroke-dashoffset="${-s.offset + circumference / 4}"
      transform="rotate(-90,${CX},${CY})"
    />`).join('');

  return `<svg viewBox="0 0 100 100" width="90" height="90">
    <circle cx="${CX}" cy="${CY}" r="${R}" fill="none" stroke="var(--border)" stroke-width="${STROKE}"/>
    ${paths}
    <text x="${CX}" y="${CY + 1}" text-anchor="middle" dominant-baseline="middle"
      font-size="11" fill="var(--text2)">${total}</text>
  </svg>`;
}

function buildTable(counts, sizes, totalCount, totalSize) {
  const rows = Object.entries(TYPE_LABELS).map(([type, label]) => {
    const n = counts[type] || 0;
    if (n === 0) return '';
    const pct = totalCount > 0 ? Math.round(n / totalCount * 100) : 0;
    return `<tr>
      <td><span class="stats-dot" style="background:${TYPE_COLORS[type]}"></span>${label}</td>
      <td>${n.toLocaleString()}</td>
      <td>${pct}%</td>
      <td>${fmtSize(sizes[type] || 0)}</td>
    </tr>`;
  }).join('');

  return `<table class="stats-tbl">
    <thead><tr><th>타입</th><th>파일 수</th><th>비율</th><th>크기</th></tr></thead>
    <tbody>
      ${rows}
      <tr class="stats-total">
        <td>합계</td>
        <td>${totalCount.toLocaleString()}</td>
        <td>100%</td>
        <td>${fmtSize(totalSize)}</td>
      </tr>
    </tbody>
  </table>`;
}

function fmtSize(bytes) {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
  if (bytes >= 1048576)    return (bytes / 1048576).toFixed(1) + ' MB';
  if (bytes >= 1024)       return (bytes / 1024).toFixed(0) + ' KB';
  return bytes + ' B';
}
