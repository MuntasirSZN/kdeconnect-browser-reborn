//! D-Bus property reading helpers.

use std::collections::HashMap;

use anyhow::{Context, Result};
use zbus::Connection;
use zbus::OwnedVariant;

use super::DEVICE_IFACE;

/// Deserialize a D-Bus property via the value's JSON representation.
///
/// `zvariant::OwnedVariant` serialises as `{"signature": "…", "value": …}`;
/// we extract the `value` field before deserialising to the concrete Rust
/// type.  This avoids depending on `DynamicDeserialize` bounds.
pub(crate) fn owned_value_to_json<T: serde::de::DeserializeOwned>(v: &OwnedVariant) -> Result<T> {
    let json = serde_json::to_value(v).context("OwnedVariant -> json")?;
    let inner = json.get("value").cloned().unwrap_or(json);
    serde_json::from_value(inner).context("json -> target type")
}

/// Call Properties.Get and return the result as `OwnedVariant`.
pub(crate) async fn get_prop_raw(
    conn: &Connection,
    path: &str,
    prop: &str,
) -> Result<OwnedVariant> {
    let reply = conn
        .call_method(
            Some(super::DEST),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(DEVICE_IFACE, prop),
        )
        .await
        .with_context(|| format!("Properties.Get({prop})"))?;

    let value: OwnedVariant = reply
        .body()
        .deserialize()
        .with_context(|| format!("deserialize property '{prop}'"))?;
    Ok(value)
}

/// Call `Properties.GetAll` and return every property as an `OwnedVariant`.
pub(crate) async fn fetch_all_props(
    conn: &Connection,
    path: &str,
) -> Result<HashMap<String, OwnedVariant>> {
    let reply = conn
        .call_method(
            Some(super::DEST),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &(DEVICE_IFACE,),
        )
        .await
        .with_context(|| format!("Properties.GetAll({path})"))?;

    reply
        .body()
        .deserialize()
        .context("deserialize property map")
}

/// Convert an `OwnedVariant` to the JSON value it carries.
pub(crate) fn variant_to_json(v: &OwnedVariant) -> serde_json::Value {
    let json = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    json.get("value").cloned().unwrap_or(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::wire::{Array, Str, Variant};

    /// `OwnedVariant` is the concrete D-Bus value type; build one from the
    /// `Variant` it owns.
    fn variant(v: Variant<'static>) -> OwnedVariant {
        OwnedVariant::try_from(v).expect("variant")
    }

    #[test]
    fn string_variants_unwrap_to_their_value() {
        let value = variant(Variant::Str(Str::from("Pixel")));
        assert_eq!(
            variant_to_json(&value),
            serde_json::json!("Pixel"),
            "the signature wrapper is stripped"
        );
        assert_eq!(
            owned_value_to_json::<String>(&value).expect("decode string"),
            "Pixel"
        );
    }

    #[test]
    fn array_variants_keep_every_element() {
        let value = variant(Variant::Array(Array::from(vec![
            Str::from("ping"),
            Str::from("share"),
        ])));
        assert_eq!(
            variant_to_json(&value),
            serde_json::json!(["ping", "share"])
        );
        assert_eq!(
            owned_value_to_json::<Vec<String>>(&value).expect("decode array"),
            vec!["ping".to_owned(), "share".to_owned()]
        );
    }

    #[test]
    fn boolean_variants_decode_to_bools() {
        let value = variant(Variant::Bool(true));
        assert_eq!(variant_to_json(&value), serde_json::json!(true));
        assert!(owned_value_to_json::<bool>(&value).expect("decode bool"));
    }

    #[test]
    fn decoding_a_mismatched_type_is_an_error() {
        let value = variant(Variant::Str(Str::from("text")));
        assert!(owned_value_to_json::<bool>(&value).is_err());
    }
}
