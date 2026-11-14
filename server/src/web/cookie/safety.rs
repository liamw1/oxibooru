use crate::model::enums::PostSafety;
use crate::resource::field::Mask;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SafetyFilter {
    enabled_safeties: Mask<PostSafety>,
}

impl SafetyFilter {
    pub fn from_jar(jar: &CookieJar) -> Self {
        jar.get(SAFETY_COOKIE)
            .and_then(|cookie| serde_urlencoded::from_str(cookie.value()).ok())
            .unwrap_or_default()
    }

    pub fn to_cookie(self) -> Cookie<'static> {
        let value = serde_urlencoded::to_string(self).expect("SafetyFilter is urlencodable");
        Cookie::build((SAFETY_COOKIE, value))
            .path("/") // TODO: Adjust to base URL
            .http_only(true)
            .same_site(SameSite::Strict)
            .permanent()
            .build()
    }

    pub fn query_filter(self) -> Option<&'static str> {
        match (self[PostSafety::Safe], self[PostSafety::Sketchy], self[PostSafety::Unsafe]) {
            (false, false, false) => Some("-safety:safe,sketchy,unsafe"),
            (false, false, true) => Some("safety:unsafe"),
            (false, true, false) => Some("safety:sketchy"),
            (false, true, true) => Some("-safety:safe"),
            (true, false, false) => Some("safety:safe"),
            (true, false, true) => Some("-safety:sketchy"),
            (true, true, false) => Some("-safety:unsafe"),
            (true, true, true) => None,
        }
    }
}

impl Default for SafetyFilter {
    fn default() -> Self {
        Self {
            enabled_safeties: Mask::none() | PostSafety::Safe | PostSafety::Sketchy,
        }
    }
}

impl Deref for SafetyFilter {
    type Target = Mask<PostSafety>;
    fn deref(&self) -> &Self::Target {
        &self.enabled_safeties
    }
}

impl DerefMut for SafetyFilter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.enabled_safeties
    }
}

const SAFETY_COOKIE: &str = "safety";
