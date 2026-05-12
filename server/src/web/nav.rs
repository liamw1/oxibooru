use crate::app::Context;
use crate::config::Action;

pub struct NavItem {
    pub title: &'static str,
    pub key: String,
    pub url: String,
}

impl NavItem {
    fn new(title: &'static str) -> Self {
        let key = title.to_lowercase().split_whitespace().collect();
        let url = format!("/{key}");
        Self { title, key, url }
    }
}

pub struct Nav {
    pub items: Vec<NavItem>,
}

impl Nav {
    pub fn create(ctx: &Context) -> Self {
        let mut items = vec![NavItem::new("Home")];

        ctx.has_privilege(Action::PostList)
            .then(|| items.push(NavItem::new("Posts")));
        ctx.has_privilege(Action::UploadCreate)
            .then(|| items.push(NavItem::new("Upload")));
        ctx.has_privilege(Action::CommentList)
            .then(|| items.push(NavItem::new("Comments")));
        ctx.has_privilege(Action::TagList)
            .then(|| items.push(NavItem::new("Tags")));
        ctx.has_privilege(Action::PoolList)
            .then(|| items.push(NavItem::new("Pools")));
        ctx.has_privilege(Action::UserList)
            .then(|| items.push(NavItem::new("Users")));

        if ctx.client.id.is_some() {
            items.push(NavItem {
                title: "Account",
                key: "account".into(),
                url: format!("/user/{}", "test_user"),
            });
            items.push(NavItem::new("Logout"));
        } else {
            items.push(NavItem::new("Register"));
            items.push(NavItem::new("Log in"));
        }
        items.push(NavItem::new("Help"));

        Self { items }
    }
}
