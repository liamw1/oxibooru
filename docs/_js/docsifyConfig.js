window.$docsify = {
    catchPluginErrors: true,
    name: 'Oxibooru Documentation',
    repo: 'https://github.com/liamw1/oxibooru/',
    // causes routing issues
    // routerMode: 'history',
    auto2top: true,
    loadSidebar: true,
    // logo: './_media/logo.png', // gets converted with css for dark mode
    formatUpdated: '{YYYY}-{MM}-{DD} {HH}:{mm}',
        alias: {
        '/.*/_sidebar.md': '/_sidebar.md',
        '.*?/LICENSE': 'https://raw.githubusercontent.com/liamw1/oxibooru/master/LICENSE.md',
            },
    nameLink: '#/',
    toc: {
        tocMaxLevel: 3,
        target: 'h2, h3, h4, h5, h6',
        ignoreHeaders: ['<!-- {docsify-ignore} -->', '<!-- {docsify-ignore-all} -->']
    },
    tabs: {
        persist: true,
        sync: true,
        theme: 'material',
        tabComments: true,
        tabHeadings: true,
    },
    // Search config https://github.com/docsifyjs/docsify/blob/141ac96add9892fcb345551906770c4e9b2265b0/docs/index.html#L105
    search: 'auto',
    progress: {
        position: 'top',
        color: 'var(--accent)',
        height: '2px',
    },
    'flexible-alerts': {
        important: {
            label: 'Important',
            icon: 'icon-important',
            className: 'important'
        },
        caution: {
            label: 'Caution',
            icon: 'icon-attention',
            className: 'attention'
        }
    }
};
