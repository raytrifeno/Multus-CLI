use crate::cli::EncryptArgs;
use crate::commands::common::{default_output_name, ensure_pdf_input};
use crate::types::{PdfToolError, Result};

pub(crate) fn handle_encrypt(args: EncryptArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    ensure_pdf_input(&input_path)?;

    let prompted_password = args.password.is_none();
    let password = if let Some(pass) = args.password.as_ref() {
        pass.trim().to_string()
    } else {
        crate::print_step("PASSWORD");
        crate::prompt_non_empty("Enter password: ")?
    };
    if password.is_empty() {
        return Err(PdfToolError::new("Password cannot be empty."));
    }

    let owner_password = if let Some(owner) = args.owner_password {
        let cleaned = owner.trim().to_string();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    } else if prompted_password {
        crate::print_step("OWNER PASSWORD");
        let value =
            crate::prompt_optional("Enter owner password (empty = same as user password): ")?;
        if value.is_empty() { None } else { Some(value) }
    } else {
        None
    };

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = default_output_name(&input_path, "encrypted", "pdf");
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(&input_path))?;

    crate::run_with_spinner("Encrypting file...", || {
        crate::encrypt_pdf(
            &input_path,
            &output_path,
            &password,
            owner_password.as_deref(),
        )
    })?;
    println!("Encryption complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}
