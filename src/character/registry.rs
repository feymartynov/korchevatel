use std::sync::{LazyLock, RwLock};
use std::{collections::HashMap, fs::File};

use anyhow::{Context, Result};

use super::Character;

const CONFIG_PATH: &str = "assets/characters.json";
static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| RwLock::new(Registry::default()));

#[derive(Debug, Default)]
pub struct Registry {
    characters: HashMap<Box<str>, Character>,
}

impl Registry {
    pub fn load() -> Result<()> {
        let characters: Vec<Character> = {
            let file = File::open(CONFIG_PATH).context(format!("Open {CONFIG_PATH}"))?;
            serde_json::from_reader(file).context(format!("Parse {CONFIG_PATH}"))?
        };

        let mut registry = REGISTRY.write().map_err(|_| anyhow!("Poisoned rwlock"))?;

        for character in characters {
            registry
                .characters
                .insert(character.name.to_string().into(), character);
        }

        Ok(())
    }

    pub fn with<F, R>(name: &str, f: F) -> R
    where
        F: FnOnce(Option<&Character>) -> R
    {
        let lock = REGISTRY.read().expect("poisoned rwlock");
        f(lock.characters.get(name))
    }
}
