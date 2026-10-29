const paths = {
  ghost: '<path d="M12 3.5c-4.1 0-7 3.2-7 7.3v9.7l3.5-2 3.5 2 3.5-2 3.5 2v-9.7c0-4.1-2.9-7.3-7-7.3Z"/><path d="M9.5 11h.01M14.5 11h.01M10 14c1.2 1 2.8 1 4 0"/>',
  cat: '<path d="m4 10 1-5 5 3h4l5-3 1 5v5a8 8 0 0 1-16 0Z"/><path d="M9 13h.01M15 13h.01M10 16c1.2 1 2.8 1 4 0"/>',
  bunny: '<path d="M9 10 7 3c-.4-1.6-2-1.4-2 .4L6 12M15 10l2-7c.4-1.6 2-1.4 2 .4L18 12"/><path d="M4 15a8 8 0 0 1 16 0 8 8 0 0 1-16 0Z"/><path d="M9.5 14h.01M14.5 14h.01M10 17c1.2 1 2.8 1 4 0"/>',
  paperclip: '<path d="m21.4 11.1-8.5 8.5a5 5 0 0 1-7.1-7.1l9.2-9.2a3.5 3.5 0 0 1 5 5l-9.2 9.2a2 2 0 0 1-2.8-2.8l8.5-8.5"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>',
  send: '<path d="M7 17 17 7M7 7h10v10"/>',
  close: '<path d="m18 6-12 12M6 6l12 12"/>',
  alert: '<path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v4m0 4h.01"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6m6-14-4 16"/>',
  bolt: '<path d="m13 2-3 9h8L11 22l3-9H6l7-11Z"/>',
  fileText: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M8 13h8m-8 4h8"/>',
  rewind: '<path d="m11 19-9-7 9-7v14Zm11 0-9-7 9-7v14Z"/>',
  eye: '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/>',
  eyeOff: '<path d="m3 3 18 18M10.6 10.6a2 2 0 0 0 2.8 2.8"/><path d="M9.9 5.2A10.7 10.7 0 0 1 12 5c6.4 0 10 7 10 7a14 14 0 0 1-3 3.8M6.2 6.2C3.5 8 2 12 2 12s3.6 7 10 7c1.3 0 2.4-.3 3.4-.8"/>',
  package: '<path d="m12 3 9 5v8l-9 5-9-5V8l9-5Z"/><path d="m3 8 9 5 9-5m-9 5v8m-4-16 9 5"/>',
  globe: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20"/>',
  arrowRight: '<path d="M5 12h14m-7-7 7 7-7 7"/>',
  refresh: '<path d="M20 7v5h-5M4 17v-5h5"/><path d="M5.6 9a7 7 0 0 1 11.6-2L20 12M4 12l2.8 5a7 7 0 0 0 11.6-2"/>',
};

export function icon(name, size = 20, className = '') {
  const pathsMarkup = paths[name];
  if (!pathsMarkup) throw new Error(`Unknown icon: ${name}`);

  return `<svg class="${className}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${pathsMarkup}</svg>`;
}
