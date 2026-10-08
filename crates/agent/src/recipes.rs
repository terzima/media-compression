use anyhow::{bail, Result};
use serde_json::Value;

/// One portable catalog is shipped as a skill resource and embedded in both interfaces.
pub fn catalog(recipe_id: Option<&str>) -> Result<Value> {
    let mut book: Value = serde_json::from_str(include_str!(
        "../../../agent-plugin/skills/media-compression/recipes.json"
    ))?;
    if let Some(id) = recipe_id {
        let recipes = book["recipes"].as_array_mut().expect("Bundled recipe list");
        recipes.retain(|recipe| recipe["id"] == id);
        if recipes.is_empty() {
            bail!("Unknown recipe {id}; use recipes to list available recipes");
        }
    }
    Ok(book)
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_engine::{settings, Properties, Settings};
    use std::collections::HashSet;

    #[test]
    fn catalog_settings_validate_for_their_media_and_channel_variants() {
        let book = catalog(None).unwrap();
        let mut ids = HashSet::new();
        for recipe in book["recipes"].as_array().unwrap() {
            assert!(ids.insert(recipe["id"].as_str().unwrap()));
            for variant in recipe["variants"].as_array().into_iter().flatten() {
                let kind = recipe["kind"].as_str().unwrap();
                let properties = Properties {
                    kind: kind.into(),
                    format: if kind == "image" { "png" } else { "wav" }.into(),
                    width: Some(64),
                    height: Some(64),
                    alpha: true,
                    bit_depth: Some(if kind == "image" { 8 } else { 24 }),
                    sample_rate: Some(48000),
                    channels: Some(variant["conditions"]["channels"].as_u64().unwrap_or(2) as u16),
                    sample_format: Some("s32".into()),
                    ..Default::default()
                };
                for setting in variant["settings"].as_array().unwrap() {
                    let mut setting: Settings = serde_json::from_value(setting.clone()).unwrap();
                    if setting.format == "jpeg" {
                        setting.background = Some("#ffffff".into());
                    }
                    settings::validate(&setting, &properties).unwrap();
                }
            }
        }
        assert!(ids.contains("audio-exact") && ids.contains("image-exact"));
    }

    #[test]
    fn filtered_catalog_keeps_execution_guidance_and_rejects_unknown_recipes() {
        let book = catalog(Some("audio-listening-study")).unwrap();
        assert_eq!(book["recipes"].as_array().unwrap().len(), 1);
        assert!(book["workflows"]["studyCli"].is_array());
        assert!(catalog(Some("video")).is_err());
    }
}
