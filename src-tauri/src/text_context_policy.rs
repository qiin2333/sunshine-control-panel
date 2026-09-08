//! Conservative UIA capability gate, not a global editing-intent detector.

pub(crate) fn is_confirmed_editor(is_edit_control: bool, value_read_only: Option<bool>) -> bool {
    // Missing/failed ValuePattern reads are unknown. TextEditPattern availability
    // deliberately plays no role: Chromium can expose it on a read-only Document.
    is_edit_control && value_read_only == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_edit_is_accepted() {
        assert!(is_confirmed_editor(true, Some(false)));
    }

    #[test]
    fn readonly_edit_is_rejected() {
        assert!(!is_confirmed_editor(true, Some(true)));
    }

    #[test]
    fn unknown_edit_is_rejected() {
        assert!(!is_confirmed_editor(true, None));
    }

    #[test]
    fn chrome_readonly_document_is_rejected() {
        assert!(!is_confirmed_editor(false, Some(true)));
    }

    #[test]
    fn document_and_custom_controls_are_not_inferred_editable() {
        assert!(!is_confirmed_editor(false, Some(false)));
        assert!(!is_confirmed_editor(false, None));
    }
}
