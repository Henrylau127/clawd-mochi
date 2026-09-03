#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundPet {
    pub id: String,
    pub name: String,
}

/// One active pet at a time. Pairing may enroll a device that is not in the roster yet.
pub fn choose_pet<'a>(
    found: &'a [FoundPet],
    active_id: Option<&str>,
    known_ids: &[String],
    pairing: bool,
) -> Option<&'a FoundPet> {
    let is_known = |pet: &FoundPet| known_ids.iter().any(|id| id == &pet.id || id == &pet.name);

    if pairing {
        if let Some(fresh) = found.iter().find(|pet| !is_known(pet)) {
            return Some(fresh);
        }
    }

    if let Some(active_id) = active_id {
        if let Some(hit) = found
            .iter()
            .find(|pet| pet.id == active_id || pet.name == active_id)
        {
            return Some(hit);
        }
    }

    None
}

pub fn display_name(raw: Option<String>) -> String {
    match raw {
        Some(name)
            if !name.is_empty() && name != "DP-????" && !name.eq_ignore_ascii_case("dp-xxxx") =>
        {
            name
        }
        _ => "Desk pet".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_pet, display_name, FoundPet};

    fn pet(id: &str, name: &str) -> FoundPet {
        FoundPet {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn unpaired_without_pairing_does_not_auto_connect() {
        let found = [pet("aaa", "DP-A1B2")];
        assert!(choose_pet(&found, None, &[], false).is_none());
    }

    #[test]
    fn pairing_enrolls_the_first_unknown_pet() {
        let found = [pet("aaa", "DP-A1B2"), pet("bbb", "DP-C3D4")];
        let chosen = choose_pet(&found, None, &[], true).unwrap();
        assert_eq!(chosen.id, "aaa");
    }

    #[test]
    fn pairing_prefers_a_new_pet_over_the_active_one() {
        let found = [pet("aaa", "DP-A1B2"), pet("bbb", "DP-C3D4")];
        let known = vec!["aaa".to_string()];
        let chosen = choose_pet(&found, Some("aaa"), &known, true).unwrap();
        assert_eq!(chosen.id, "bbb");
    }

    #[test]
    fn reconnect_stays_on_the_active_pet() {
        let found = [pet("aaa", "DP-A1B2"), pet("bbb", "DP-C3D4")];
        let known = vec!["aaa".to_string(), "bbb".to_string()];
        let chosen = choose_pet(&found, Some("bbb"), &known, false).unwrap();
        assert_eq!(chosen.id, "bbb");
    }

    #[test]
    fn hides_placeholder_names() {
        assert_eq!(display_name(None), "Desk pet");
        assert_eq!(display_name(Some("DP-????".into())), "Desk pet");
        assert_eq!(display_name(Some("DP-xxxx".into())), "Desk pet");
        assert_eq!(display_name(Some("DP-A1B2".into())), "DP-A1B2");
    }
}
