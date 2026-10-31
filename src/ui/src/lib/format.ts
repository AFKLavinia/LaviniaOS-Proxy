const UNITS = ["B/s", "KB/s", "MB/s", "GB/s"];
const COUNT_FMT = new Intl.NumberFormat("en-US");

export function formatRate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec < 0) {
    return "0 B/s";
  }
  let v = bytesPerSec;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${UNITS[i]}`;
}

export function formatCount(n: number): string {
  if (!Number.isFinite(n)) {
    return "0";
  }
  return COUNT_FMT.format(n);
}
