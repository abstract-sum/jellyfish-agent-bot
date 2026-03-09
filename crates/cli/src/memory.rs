use jellyfish_core::{MemoryKind, Session};

pub fn apply_memory_updates(session: &mut Session, input: &str) -> Vec<String> {
    let trimmed = input.trim();
    let mut updates = Vec::new();

    if let Some(value) = capture_after_prefixes(
        trimmed,
        &["记住：", "记住:", "记住 ", "remember: ", "remember "],
    ) {
        session.remember(MemoryKind::Note, value.to_string());
        updates.push(format!("remembered note: {value}"));
    }

    if let Some(value) = capture_after_prefixes(trimmed, &["我叫", "my name is "]) {
        session.set_display_name(value.to_string());
        session.remember(MemoryKind::Profile, format!("display_name={value}"));
        updates.push(format!("updated display name: {value}"));
    }

    if let Some(value) = capture_after_prefixes(trimmed, &["我的时区是", "my timezone is "]) {
        session.set_timezone(value.to_string());
        session.remember(MemoryKind::Profile, format!("timezone={value}"));
        updates.push(format!("updated timezone: {value}"));
    }

    if let Some((key, value)) = capture_preference(trimmed) {
        session.set_preference(key.to_string(), value.to_string());
        session.remember(MemoryKind::Preference, format!("{key}={value}"));
        updates.push(format!("updated preference: {key}={value}"));
    }

    updates
}

fn capture_after_prefixes<'a>(input: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|prefix| {
        input
            .strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn capture_preference(input: &str) -> Option<(&str, &str)> {
    let candidates = [
        "我的偏好是",
        "我的偏好: ",
        "我的偏好：",
        "preference: ",
        "my preference is ",
    ];

    let remainder = capture_after_prefixes(input, &candidates)?;
    let separator = if remainder.contains('=') {
        '='
    } else if remainder.contains(':') {
        ':'
    } else {
        return None;
    };

    let (key, value) = remainder.split_once(separator)?;
    let key = key.trim();
    let value = value.trim();

    if key.is_empty() || value.is_empty() {
        None
    } else {
        Some((key, value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_session_from_memory_phrases() {
        let mut session = Session::new();
        let updates = apply_memory_updates(&mut session, "记住：我喜欢早上收到简洁摘要");

        assert_eq!(updates.len(), 1);
        assert_eq!(session.memories.len(), 1);
    }

    #[test]
    fn updates_display_name_and_preference() {
        let mut session = Session::new();
        apply_memory_updates(&mut session, "我叫 Yvonne");
        apply_memory_updates(&mut session, "我的偏好是 tone=concise");

        assert_eq!(session.profile.display_name.as_deref(), Some("Yvonne"));
        assert_eq!(session.profile.preferences.len(), 1);
    }
}
