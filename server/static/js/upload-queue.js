(() => {
    const FORM_SELECTOR = '#upload-form';
    const HELD_INPUT_SELECTOR = 'input[type=file][name^="held["]';

    const heldInput = (key) => document.getElementById(`upload-file-${key}`);

    let keyCounter = 0;
    const newKey = () => `${Date.now().toString(36)}-${(keyCounter++).toString(36)}`;

    const hiddenInput = (name, value) => {
        const input = document.createElement('input');
        input.type = 'hidden';
        input.name = name;
        input.value = value;
        return input;
    };

    document.addEventListener('change', (e) => {
        const dropper = e.target;
        if (!(dropper instanceof HTMLInputElement) || !dropper.matches(`${FORM_SELECTOR} #upload-content`)) return;

        const form = dropper.form;
        const container = form.querySelector('#held-files');
        [...dropper.files].forEach((file, i) => {
            const key = newKey();
            const input = document.createElement('input');
            input.type = 'file';
            input.id = `upload-file-${key}`;
            input.name = `held[${key}]`;
            input.hidden = true;
            input.setAttribute('hx-preserve', '');

            const transfer = new DataTransfer();
            transfer.items.add(file);
            input.files = transfer.files;

            container.append(input, hiddenInput(`new-files[${i}][key]`, key), hiddenInput(`new-files[${i}][name]`, file.name));
        });

        dropper.value = '';
        form.requestSubmit(form.querySelector('button[value=add]'));
    });

    const nextItem = (form) =>
        [...form.querySelectorAll('.uploadable-container')].find((item) => !item.querySelector('input[name$="[message]"]'));

    // Only send the file for the item that's about to be uploaded, and only if the server hasn't already received it
    document.addEventListener('htmx:config:request', (e) => {
        const { form, body } = e.detail.ctx.request;
        if (!form?.matches(FORM_SELECTOR) || !(body instanceof FormData)) return;

        const next = body.get('operation') === 'continue' ? nextItem(form) : null;
        const needsFile = next?.querySelector('input[name$="[content][held]"]');
        for (const input of form.querySelectorAll(HELD_INPUT_SELECTOR)) {
            if (!needsFile || input.closest('.uploadable-container') !== next) body.delete(input.name);
        }
    });

    const previewUrls = new Map();

    const fillPreview = (wrapper) => {
        const key = wrapper.dataset.previewKey;
        const file = heldInput(key)?.files?.[0];
        const isImage = file?.type.startsWith('image/');
        const isVideo = file?.type.startsWith('video/');
        if (!(isImage || isVideo) || wrapper.dataset.previewFilled) return;

        if (!previewUrls.has(key)) previewUrls.set(key, URL.createObjectURL(file));
        const url = previewUrls.get(key);

        const link = document.createElement('a');
        link.href = url;
        if (isImage) {
            const thumbnail = document.createElement('span');
            thumbnail.className = 'thumbnail';
            thumbnail.style.backgroundImage = `url('${url}')`;
            const image = document.createElement('img');
            image.alt = 'thumbnail';
            image.src = url;
            thumbnail.append(image);
            link.append(thumbnail);
            wrapper.replaceChildren(link);
        } else {
            const video = document.createElement('video');
            video.muted = true;
            const source = document.createElement('source');
            source.type = file.type;
            source.src = url;
            video.append(source);
            link.append(video);
            const thumbnail = document.createElement('div');
            thumbnail.className = 'thumbnail';
            thumbnail.append(link);
            wrapper.replaceChildren(thumbnail);
        }
        wrapper.dataset.previewFilled = 'true';
    };

    // Mutation observers run before the next paint, so the empty placeholder is never shown
    new MutationObserver(() => {
        for (const wrapper of document.querySelectorAll('[data-preview-key]:not([data-preview-filled])')) {
            fillPreview(wrapper);
        }
    }).observe(document.documentElement, { childList: true, subtree: true });
})();
