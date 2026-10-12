// Remembers which expanders are open across page loads and HTMX swaps.
// Expanders opt in with a stable key: <details class="expander" data-expander="key">
(() => {
    const STORAGE_KEY = 'expander';

    const loadStates = () => {
        try {
            return JSON.parse(localStorage.getItem(STORAGE_KEY)) || {};
        } catch {
            return {};
        }
    };

    const restore = () => {
        const states = loadStates();
        for (const details of document.querySelectorAll('details.expander[data-expander]')) {
            const key = details.dataset.expander;
            if (!(key in states) || details.open === states[key]) continue;

            // Suppress the chevron transition so the restore isn't animated
            details.classList.add('restoring');
            details.open = states[key];
            requestAnimationFrame(() => requestAnimationFrame(() => details.classList.remove('restoring')));
        }
    };

    // Save on user interaction rather than on 'toggle', which also fires when an
    // initially-open expander is inserted and would overwrite the saved state
    document.addEventListener('click', (e) => {
        const details = e.target.closest?.('details.expander[data-expander] > summary')?.parentElement;
        if (!details) return;
        // The open state flips after the click handlers have run
        setTimeout(() => {
            localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...loadStates(), [details.dataset.expander]: details.open }));
        });
    });

    // Mutation observers run before the next paint, so expanders added by the parser
    // or by an HTMX swap are restored without their default state ever being shown
    new MutationObserver((mutations) => {
        if (mutations.some((mutation) => mutation.addedNodes.length)) restore();
    }).observe(document.documentElement, { childList: true, subtree: true });
})();
