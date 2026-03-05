//! Boundary exports for daemon protocol/auth contracts used outside daemon internals.

pub use crate::daemon::protocol;

pub fn read_auth_token() -> Option<String> {
    crate::daemon::auth::read_auth_token()
}
