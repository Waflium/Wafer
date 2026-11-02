use std::path::Path;

use super::dedicated::settings::Properties;

const EULA_LINK: &str = "https://aka.ms/MinecraftEULA";

pub struct Eula {
    agreed: bool,
}

impl Eula {
    pub fn new(file: &Path) -> Eula {
        let properties = std::fs::read(file).ok().and_then(|bytes| Properties::load(&bytes.iter().map(|&b| b as char).collect::<String>()).ok());
        let agreed = match properties {
            Some(properties) => properties.get("eula").unwrap_or("false").eq_ignore_ascii_case("true"),
            None => {
                warn!("Failed to load {}", file.display());
                save_defaults(file);
                false
            }
        };
        Eula { agreed }
    }

    pub fn has_agreed_to_eula(&self) -> bool {
        self.agreed
    }
}

fn save_defaults(file: &Path) {
    let mut properties = Properties::default();
    properties.put("eula", "false".into());
    let comments = format!("By changing the setting below to TRUE you are indicating your agreement to our EULA ({EULA_LINK}).");
    if std::fs::write(file, properties.store(&comments, true)).is_err() {
        warn!("Failed to save {}", file.display());
    }
}
