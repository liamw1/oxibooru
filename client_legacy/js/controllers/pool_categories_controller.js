"use strict";

const api = require("../api.js");
const pools = require("../pools.js");
const PoolCategoryList = require("../models/pool_category_list.js");
const topNavigation = require("../models/top_navigation.js");
const PoolCategoriesView = require("../views/pool_categories_view.js");
const EmptyView = require("../views/empty_view.js");

class PoolCategoriesController {
    constructor() {
        if (!api.hasPrivilege("pool_category_list")) {
            this._view = new EmptyView();
            this._view.showError(
                "You don't have privileges to view pool categories."
            );
            return;
        }

        topNavigation.activate("pools");
        topNavigation.setTitle("Listing pools");
        PoolCategoryList.get().then(
            (response) => {
                this._poolCategories = response.results;
                this._view = new PoolCategoriesView({
                    poolCategories: this._poolCategories,
                    canEditName: api.hasPrivilege("pool_category_edit_name"),
                    canEditColor: api.hasPrivilege(
                        "pool_category_edit_color"
                    ),
                    canDelete: api.hasPrivilege("pool_category_delete"),
                    canCreate: api.hasPrivilege("pool_category_create"),
                    canSetDefault: api.hasPrivilege(
                        "pool_category_set_default"
                    ),
                });
                this._view.addEventListener("submit", (e) =>
                    this._evtSubmit(e)
                );
            },
            (error) => {
                this._view = new EmptyView();
                this._view.showError(error.message);
            }
        );
    }

    _evtSubmit(e) {
        this._view.clearMessages();
        this._view.disableForm();
        this._poolCategories.save().then(
            () => {
                pools.refreshCategoryColorMap();
                this._view.enableForm();
                this._view.showSuccess("Changes saved.");
            },
            (error) => {
                this._view.enableForm();
                this._view.showError(error.message);
            }
        );
    }
}

module.exports = (router) => {
    router.enter(["pool-categories"], (ctx, next) => {
        ctx.controller = new PoolCategoriesController(ctx, next);
    });
};
