// progress.js — 진행 카운터 덮어쓰기 지원 updateLog
// \r prefix 메시지는 마지막 .progress div를 교체, 나머지는 새 div 추가

function makeLogDiv(text, cls) {
  const div = document.createElement('div');
  if (cls) div.className = cls;
  if (text.includes('[오류]')) div.className = 'err';
  div.textContent = text;
  return div;
}

function updateLog(lines) {
  const el = document.getElementById('log-panel');
  if (!el) return;
  lines.forEach(line => {
    if (line.startsWith('\r')) {
      const text = line.slice(1);
      const last = el.querySelector('.progress');
      const div = makeLogDiv(text, 'progress');
      if (last) last.replaceWith(div);
      else el.appendChild(div);
    } else {
      el.appendChild(makeLogDiv(line));
    }
  });
  el.scrollTop = el.scrollHeight;
}
