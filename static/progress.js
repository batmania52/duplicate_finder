// progress.js — 진행 카운터 덮어쓰기 지원 updateLog + 게이지 업데이트
// \r prefix 메시지는 마지막 .progress div를 교체, 나머지는 새 div 추가

function setGauge(progress) {
  const gaugeEl = document.getElementById('status-gauge');
  if (!gaugeEl) return;
  if (!progress) { gaugeEl.style.display = 'none'; return; }
  gaugeEl.style.display = '';
  document.getElementById('gauge-label').textContent = progress.label;
  document.getElementById('gauge-fill').style.width = (progress.pct * 100).toFixed(1) + '%';
}

function makeLogDiv(text, cls) {
  const div = document.createElement('div');
  if (cls) div.className = cls;
  if (text.includes('[오류]')) div.className = 'err';
  div.textContent = text;
  return div;
}

// "(current / total)" 패턴 파싱 → { pct, label } 또는 null
function parseProgress(text) {
  const m = text.match(/\((\d+)\s*\/\s*(\d+)\)/);
  if (!m) return null;
  const current = parseInt(m[1], 10);
  const total = parseInt(m[2], 10);
  if (total === 0) return null;
  return { pct: current / total, label: text };
}

function updateLog(lines) {
  const el = document.getElementById('log-panel');
  if (!el) return;
  lines.forEach(line => {
    if (line.startsWith('\r')) {
      const text = line.slice(1);
      const last = el.querySelector('.progress');
      if (text === '') {
        if (last) last.className = '';
        setGauge(null);
      } else {
        const div = makeLogDiv(text, 'progress');
        if (last) last.replaceWith(div);
        else el.appendChild(div);
        setGauge(parseProgress(text));
      }
    } else {
      el.appendChild(makeLogDiv(line));
    }
  });
  el.scrollTop = el.scrollHeight;
}
