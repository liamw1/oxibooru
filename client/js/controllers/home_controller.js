"use strict";

const api = require("../api.js");
const config = require("../config.js");
const topNavigation = require("../models/top_navigation.js");
const HomeView = require("../views/home_view.js");

class HomeController {
    constructor() {
        topNavigation.activate("home");
        topNavigation.setTitle("Home");

        this._homeView = new HomeView({
            name: api.getName(),
            version: config.meta.version,
            buildDate: config.meta.buildDate,
            canListSnapshots: api.hasPrivilege("snapshot_list"),
            canListPosts: api.hasPrivilege("post_list"),
            isDevelopmentMode: config.environment == "development",
        });

        api.fetchConfig().then(() => {
            this._homeView.setStats({
                postCount: api.getPostCount(),
            });
        });
    }

    showSuccess(message) {
        this._homeView.showSuccess(message);
    }

    showError(message) {
        this._homeView.showError(message);
    }
}

module.exports = (router) => {
    router.enter([], (ctx, next) => {
        ctx.controller = new HomeController();
    });
};
