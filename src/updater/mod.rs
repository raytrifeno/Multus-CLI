mod install;
mod shell;
mod uninstall;
mod version;

pub const UPDATE_REPO_URL: &str = "https://github.com/raytrifeno/Multus-CLI.git";
pub const UPDATE_REPO_REF: &str = "main";

pub(crate) use install::update_multus;
pub(crate) use uninstall::uninstall_multus;
pub(crate) use version::{VersionState, check_version_state};
