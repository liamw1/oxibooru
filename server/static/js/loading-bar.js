// Adds a loading bar for slower requests
NProgress.configure({ showSpinner: false, trickleSpeed: 200 });

let pending = 0, timer = null;
const DELAY_MS = 150;

document.addEventListener('htmx:before:request', () => {
    if (pending++ === 0) timer = setTimeout(() => { timer = null; NProgress.start(); }, DELAY_MS);
});
document.addEventListener('htmx:after:request', () => {
    if (--pending > 0) return;
    if (timer) { clearTimeout(timer); timer = null; }
    else NProgress.done();
});
