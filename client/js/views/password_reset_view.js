'use strict';

const events = require('../events.js');
const BaseView = require('./base_view.js');

class PasswordResetView extends BaseView {
    constructor() {
        super();
        this.template = this.getTemplate('password-reset-template');
    }

    render(ctx) {
        const target = this.contentHolder;
        const source = this.template();

        const form = source.querySelector('form');
        const userNameOrEmailField = source.querySelector('#user-name');

        this.decorateValidator(form);

        form.addEventListener('submit', e => {
            e.preventDefault();
            this.clearMessages();
            this.disableForm(form);
            ctx.proceed(userNameOrEmailField.value)
                .catch(() => { this.enableForm(form); });
        });

        this.showView(target, source);
    }
}

module.exports = PasswordResetView;