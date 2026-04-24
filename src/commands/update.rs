use crate::cli::UpdateArgs;
use crate::types::Result;
use crate::updater::{
    UPDATE_REPO_REF, UPDATE_REPO_URL, VersionState, check_version_state, update_multus,
};

pub(crate) fn handle_update(args: UpdateArgs) -> Result<i32> {
    let repo = args.repo.unwrap_or_else(|| UPDATE_REPO_URL.to_string());
    let branch = args.branch.unwrap_or_else(|| UPDATE_REPO_REF.to_string());

    match check_version_state(&repo, &branch) {
        VersionState::UpToDate { current } => {
            println!("Already up to date (v{current}).");
            return Ok(0);
        }
        VersionState::UpdateAvailable { current, latest } => {
            println!("Update available: v{current} -> v{latest}");
        }
        VersionState::Unknown { current } => {
            println!("Version current: v{current}");
            println!("Could not verify remote version. Trying direct update...");
        }
    }

    println!("Updating from: {repo} (branch: {branch})");
    let status = crate::run_with_spinner("Updating multus...", || update_multus(&repo, &branch))?;

    println!("{status}");
    println!("Run 'multus --version' to verify current version.");
    Ok(0)
}
