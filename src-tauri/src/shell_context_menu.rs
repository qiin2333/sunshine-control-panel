#[cfg(target_os = "windows")]
pub fn install_file_transfer_menu() -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::*;

    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let exe = exe.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (verb, _) = hkcu
        .create_subkey(r"Software\Classes\*\shell\Sunshine.SendToClient")
        .map_err(|e| format!("create context menu key failed: {e}"))?;

    verb.set_value("", &"发送到 Sunshine 客户端")
        .map_err(|e| format!("set context menu title failed: {e}"))?;
    verb.set_value("MUIVerb", &"发送到 Sunshine 客户端")
        .map_err(|e| format!("set context menu verb failed: {e}"))?;
    verb.set_value("Icon", &exe)
        .map_err(|e| format!("set context menu icon failed: {e}"))?;
    verb.set_value("MultiSelectModel", &"Single")
        .map_err(|e| format!("set context menu multiselect failed: {e}"))?;

    let (command, _) = verb
        .create_subkey("command")
        .map_err(|e| format!("create context menu command key failed: {e}"))?;
    let command_line = format!("\"{}\" --send-to-client \"%1\"", exe);
    command
        .set_value("", &command_line)
        .map_err(|e| format!("set context menu command failed: {e}"))?;

    Ok(())
}
