pub use warp_server_client::auth::{AuthClient, UserAuthenticationError};

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
