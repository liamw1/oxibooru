// Used for syncing post navigation buttons when toggling edit mode to avoid requerying post neighbors
function syncPostMode(aside, ctx) {
    if (ctx.target.id !== 'sidebar-content') return;

    const edit = /\/post\/[^/]+\/edit(\/|$)/.test(new URL(ctx.request.action, location.origin).pathname);
    const setMode = (a, toEdit) => {
        const url = new URL(a.getAttribute('href'), location.origin);
        const base = url.pathname.replace(/\/edit\/?$/, '');
        url.pathname = toEdit ? `${base}/edit` : base;
        a.setAttribute('href', url.pathname + url.search + url.hash);
    };

    for (const a of aside.querySelectorAll('.previous-post a[href], .next-post a[href]')) setMode(a, edit);

    const toggle = aside.querySelector('#edit-toggle a');
    setMode(toggle, !edit);
    toggle.querySelector('i').className = edit ? 'fa fa-reply' : 'fa fa-pencil';
}
