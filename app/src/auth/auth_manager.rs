#[cfg(any(test, feature = "integration_tests"))]
use warpui::{Entity, SingletonEntity};

pub type LoginGatedFeature = &'static str;

#[cfg(any(test, feature = "integration_tests"))]
pub struct AuthManager;

#[cfg(any(test, feature = "integration_tests"))]
impl AuthManager {
    #[cfg(test)]
    pub fn new_for_test(_: &mut warpui::ModelContext<Self>) -> Self {
        Self
    }
}

#[cfg(any(test, feature = "integration_tests"))]
impl Entity for AuthManager {
    type Event = ();
}

#[cfg(any(test, feature = "integration_tests"))]
impl SingletonEntity for AuthManager {}
