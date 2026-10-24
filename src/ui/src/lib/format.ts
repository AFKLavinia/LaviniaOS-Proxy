const UNITS = ["B/s", "KB/s", "MB/s", "GB/s"];

export function formatRate(bytesPerSec: number): string {
  let v = bytesPerSec;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${UNITS[i]}`;
}

export function formatCount(n: number): string {
  return new Intl.NumberFormat("en-US").format(n);
}
